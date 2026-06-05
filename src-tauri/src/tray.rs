//! 系统托盘模块：构建托盘菜单并绑定事件处理
//!
//! 策略：托盘由本模块 setup_tray() 唯一构建（带右键菜单+事件回调）；
//! tauri.conf.json 不声明 app.trayIcon，避免"配置自动建 + 代码建"双图标。
//! 图标使用专用单色模板图 tray.png，通过 include_bytes! 编译期嵌入二进制，
//! dev/prod 均从内存读取，不依赖 resource_dir 路径。以 icon_as_template(true)
//! 告知 macOS 此为 template image，系统自动按菜单栏明/暗模式反色渲染。
//!
//! 菜单项：
//! - "显示 QuickQuick" → show + set_focus
//! - "退出" → app.exit(0)
//!
//! 左键点击托盘图标 → show + set_focus（与"显示"菜单项等效）

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

/// 在 setup 阶段构建系统托盘图标与菜单。
///
/// # Errors
/// 若菜单或托盘构建失败则返回错误（由 Tauri 统一处理，不会 panic）。
pub fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show_item = MenuItemBuilder::new("显示 QuickQuick")
        .id("show")
        .build(app)?;
    let quit_item = MenuItemBuilder::new("退出").id("quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&show_item, &quit_item])
        .build()?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("QuickQuick")
        .on_menu_event(|app_handle, event| {
            handle_menu_event(app_handle, event.id.as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            handle_tray_icon_event(tray.app_handle(), &event);
        });

    // tray.png 在编译期嵌入二进制，dev/prod 均从内存读取，不依赖 resource_dir。
    // icon_as_template(true) 告知 macOS 这是 template image，
    // 系统会自动按当前菜单栏明/暗模式将黑色像素反色为白色（或保持黑色），
    // 无需应用层手动处理深色模式切换。
    match Image::from_bytes(include_bytes!("../icons/tray.png")) {
        Ok(icon) => {
            builder = builder.icon(icon).icon_as_template(true);
        }
        Err(e) => {
            // 极少发生（编译期已嵌入有效 PNG），回退到应用图标但不套 template，
            // 避免彩色图标经 template 渲染后变成实心白块。
            eprintln!("[QuickQuick] 托盘：加载 tray.png 失败，回退到应用图标: {e}");
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
        }
    }

    // 构建托盘；Tauri 将其生命周期与 app 绑定，build 后无需显式持有
    let _tray = builder.build(app)?;

    Ok(())
}

/// 处理托盘菜单点击事件。
fn handle_menu_event(app: &tauri::AppHandle, item_id: &str) {
    match item_id {
        "show" => show_and_focus_window(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// 处理托盘图标自身的事件（左键单击 → 显示窗口）。
fn handle_tray_icon_event(app: &tauri::AppHandle, event: &TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        show_and_focus_window(app);
    }
}

/// 显示 main 窗口并设置焦点；失败时记录日志不 panic。
///
/// 可见性为 `pub(crate)` 供 single-instance 插件回调复用：第二个实例启动时，
/// 已运行实例需执行与托盘「显示 QuickQuick」一致的显示行为，复用此函数避免重复实现。
pub(crate) fn show_and_focus_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        eprintln!("[QuickQuick] 托盘：找不到 main 窗口");
        return;
    };
    if let Err(e) = window.show() {
        eprintln!("[QuickQuick] 托盘：显示窗口失败: {e}");
        return;
    }
    // 顺序关键：必须先 activate 进程，再 set_focus。Accessory 策略下进程不在常规
    // 激活序列里，仅靠 set_focus（底层是 macOS 14+ 已废弃的 activateIgnoringOtherApps:）
    // 无法让窗口拿到 key 状态，键盘事件进不了 webview（搜索框打不了字）。
    // 复用 popover 已验证的激活实现（见 popover.rs:83-89 与设计文档 4.2）。
    #[cfg(target_os = "macos")]
    crate::popover::activate_app_macos();
    if let Err(e) = window.set_focus() {
        eprintln!("[QuickQuick] 托盘：设置焦点失败: {e}");
    }
}

#[cfg(test)]
mod tests {
    // 与 popover 模块的 FFI 测试边界一致：activate_app_macos 是纯 NSApplication 包装，
    // headless 下不真正调用（sharedApplication 无 NSApp 时行为未定义）。此处只做
    // 编译期/类型守卫——确认 tray 能把 popover 的激活函数当作 `fn()` 引用，
    // 即 pub(crate) 提升成立、跨模块复用单一实现（DRY）。真实激活+键盘焦点行为
    // 归真机 manual_confirm（见设计文档 M2）。

    /// tray 可将 popover 的 activate_app_macos 作为 fn() 引用：守卫 pub(crate) 可见性，
    /// 确保 show_and_focus_window 复用 popover 的单一激活实现而非另造 FFI。
    #[cfg(target_os = "macos")]
    #[test]
    fn popover_activate_fn_is_referenceable_by_tray() {
        let _f: fn() = crate::popover::activate_app_macos;
    }
}
