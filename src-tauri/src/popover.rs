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

/// 触发指定 label 的 popover 窗口：get_or_create → 定位 → 显示 → 聚焦。
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
    if let Err(e) = window.set_focus() {
        eprintln!("[QuickQuick] {label} set_focus 失败: {e}");
    }
}

/// 获取已存在的 popover 窗口，或在首次调用时按规格创建并注册失焦隐藏。
fn get_or_create_popover(
    handle: &tauri::AppHandle,
    spec: &PopoverSpec,
) -> Option<WebviewWindow> {
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
