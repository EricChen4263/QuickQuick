//! Popover 浮层窗口管理模块
//!
//! 负责两个独立 popover 窗口的懒建、触发、失焦隐藏：
//! - `clip-popover`（720×480）：剪贴板快速选取
//! - `trans-popover`（320×200）：迷你翻译
//!
//! 窗口不写进 tauri.conf.json，首次触发时按需创建，复用时直接定位并显示。
//! 所有可失败操作均用 eprintln 记录并优雅降级，不 panic、不向上抛错。

use tauri::{Manager, WebviewWindow, WindowEvent};

use crate::window_pos::compute_window_position_for_width;

/// clip-popover 窗口宽度（物理像素）
const CLIP_WIDTH: i32 = 720;
/// clip-popover 窗口高度（物理像素）
const CLIP_HEIGHT: f64 = 480.0;
/// trans-popover 窗口宽度（物理像素）
const TRANS_WIDTH: i32 = 320;
/// trans-popover 窗口高度（物理像素）
const TRANS_HEIGHT: f64 = 200.0;

/// Popover 窗口规格：统一描述一个 popover 所需的全部参数，避免重复硬编码。
struct PopoverSpec {
    label: &'static str,
    url: &'static str,
    width: i32,
    height: f64,
}

/// 两个 popover 的规格表，顺序与 label 一一对应。
const POPOVER_SPECS: &[PopoverSpec] = &[
    PopoverSpec {
        label: "clip-popover",
        url: "src/clip-popover/index.html",
        width: CLIP_WIDTH,
        height: CLIP_HEIGHT,
    },
    PopoverSpec {
        label: "trans-popover",
        url: "src/trans-popover/index.html",
        width: TRANS_WIDTH,
        height: TRANS_HEIGHT,
    },
];

/// 触发指定 label 的 popover 窗口：get_or_create → 定位 → 显示 → 激活 app → 聚焦。
///
/// `label` 必须是 `"clip-popover"` 或 `"trans-popover"`；
/// 未知 label 会记录错误并立即返回。
pub fn trigger_popover(handle: &tauri::AppHandle, label: &str) {
    let Some(spec) = POPOVER_SPECS.iter().find(|s| s.label == label) else {
        eprintln!("[QuickQuick] 未知 popover label: {label}");
        return;
    };

    let Some(window) = get_or_create_popover(handle, spec) else {
        return;
    };

    let position = compute_window_position_for_width(&window, spec.width);

    if let Err(e) = window.set_position(position) {
        eprintln!("[QuickQuick] {label} set_position 失败: {e}");
    }
    if let Err(e) = window.show() {
        eprintln!("[QuickQuick] {label} show 失败: {e}");
        return;
    }
    // 先激活 app，再聚焦窗口。
    // 顺序说明：macOS 要求进程本身处于"活跃"(active)状态后，
    // 窗口的 makeKeyAndOrderFront 才能拿到 key 状态。
    // 若反序（先 set_focus 后 activate），窗口已前置但 app 未激活，
    // 依然收不到键盘事件——全局热键回调场景必须显式激活。
    activate_app_macos();
    if let Err(e) = window.set_focus() {
        eprintln!("[QuickQuick] {label} set_focus 失败: {e}");
    }
}

/// 显式激活当前 app，使 QuickQuick 成为 macOS 活跃应用。
///
/// 为何需要此调用：tauri/tao 的 `window.set_focus()` 底层用的是已废弃的
/// `activateIgnoringOtherApps:YES`，在 macOS 14+（含 macOS 26.5）上该方法
/// 已成 no-op。从全局热键触发时前台是其他 app，QuickQuick 进程未被激活，
/// `makeKeyAndOrderFront` 无法让窗口真正拿到 key——键盘事件永远不会进入 webview。
///
/// 此 helper 调用 `NSApplication.activate()`（macOS 14+ 正式接口），
/// 替代已废弃的 `activateIgnoringOtherApps:`，确保进程被激活。
///
/// 拿不到 MainThreadMarker（理论上不应发生：热键回调在主线程）时优雅跳过，
/// 不 panic，与本文件其他错误处理风格（eprintln 降级）一致。
/// `pub(crate)`：tray 在 Accessory 策略下显示窗口时复用同一激活实现，
/// 避免在 tray.rs 另造一份 NSApplication FFI（DRY，见设计文档改造点 #2）。
#[cfg(target_os = "macos")]
pub(crate) fn activate_app_macos() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    // MainThreadMarker::new() 是安全函数：内部读 pthread_main_np() 判断线程，
    // 全局热键回调在主线程，实践中此处总返回 Some；拿不到时优雅跳过。
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("[QuickQuick] activate_app_macos: 非主线程，跳过激活");
        return;
    };

    // sharedApplication 与 activate 均为 objc2-app-kit 0.3 安全 API，
    // MainThreadMarker 已保证主线程约束，无需额外 unsafe。
    let app = NSApplication::sharedApplication(mtm);
    app.activate();
}

/// 非 macOS 平台：空实现，编译时零开销消除。
#[cfg(not(target_os = "macos"))]
pub(crate) fn activate_app_macos() {}

/// 获取已存在的 popover 窗口，或在首次调用时按规格创建并注册失焦隐藏。
fn get_or_create_popover(handle: &tauri::AppHandle, spec: &PopoverSpec) -> Option<WebviewWindow> {
    // 已存在则直接复用，无需重建
    if let Some(existing) = handle.get_webview_window(spec.label) {
        return Some(existing);
    }

    let window = tauri::WebviewWindowBuilder::new(
        handle,
        spec.label,
        tauri::WebviewUrl::App(spec.url.into()),
    )
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .resizable(false)
    .inner_size(spec.width as f64, spec.height)
    .build();

    match window {
        Ok(w) => {
            register_focus_lost_hide(&w);
            Some(w)
        }
        Err(e) => {
            eprintln!("[QuickQuick] {} 创建失败: {e}", spec.label);
            None
        }
    }
}

/// 在窗口上注册失焦隐藏事件：失去焦点时自动 hide。
///
/// 用 clone 将窗口句柄移入闭包，避免生命周期问题。
fn register_focus_lost_hide(window: &WebviewWindow) {
    let window_clone = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            if let Err(e) = window_clone.hide() {
                eprintln!("[QuickQuick] popover hide 失败: {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    // activate_app_macos 是纯 FFI 包装，无法在无 GUI 的 headless 环境中真正调用
    // NSApplication（sharedApplication 在无 NSApp 时行为未定义）。
    // 此处验证的是：函数签名存在、可被引用、cfg 门控正确——即"编译即通过"的结构测试。
    // 运行时激活行为需通过手动集成测试（全局热键触发 + 键盘输入验证）验证。

    #[cfg(target_os = "macos")]
    #[test]
    fn activate_app_macos_is_callable_as_fn_pointer() {
        // 将私有函数赋值给函数指针，确认签名为 fn()。
        // 此测试在 headless CI 中不调用 NSApplication，只验证函数存在且类型正确。
        let _f: fn() = super::activate_app_macos;
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn activate_app_macos_noop_on_non_macos() {
        // 非 macOS 平台的空实现应可被安全调用，无副作用。
        super::activate_app_macos();
    }
}
