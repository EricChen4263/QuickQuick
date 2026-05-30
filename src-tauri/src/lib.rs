//! QuickQuick Tauri 应用核心库
//!
//! 负责初始化 Tauri builder、注册插件，以及挂载 IPC 命令。
//! 二进制入口（main.rs）仅调用本模块的 `run()`，保持入口文件极简。
//!
//! 子模块：
//! - `hotkey`：全局热键配置、改键持久化与冲突检测
//! - `tray`：系统托盘菜单构建（setup 阶段调用）
//! - `window_pos`：预热窗口定位逻辑（光标所在显示器水平居中、靠上 15%）

pub mod autostart;
pub mod hotkey;
pub mod keyprovider;
mod tray;
mod window_pos;

use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// 启动 Tauri 应用。
///
/// 注册插件：
/// - `tauri-plugin-autostart`：开机自启（默认开，行为由 OS 侧控制）
/// - `tauri-plugin-updater`：应用自动更新（endpoints 在 tauri.conf.json 配置）
/// - `tauri-plugin-global-shortcut`：全局热键注册
///
/// setup 阶段：
/// - 注册两个全局热键（history/translate），失败时记录但不 panic
/// - 构建系统托盘菜单（显示/退出）
/// - 监听失焦事件 → 隐藏窗口
///
/// # Panics
/// 若 Tauri builder 初始化失败则 panic（属于不可恢复的启动错误）。
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            apply_autostart_preference(app);
            register_hotkeys(app.handle());
            tray::setup_tray(app)?;
            setup_window_focus_hide(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

/// 读取自启动偏好配置并应用到 OS LaunchAgent。
///
/// 配置路径优先用 `app_config_dir()`；若拿不到则跳过（仅 eprintln 不 panic）。
/// 调用 enable/disable 失败时同样仅 eprintln，不阻断启动。
fn apply_autostart_preference(app: &mut tauri::App) {
    // 计算配置文件路径；拿不到目录时优雅降级
    let config_path = match app.path().app_config_dir() {
        Ok(dir) => {
            // 目录可能尚未创建（首次启动），尝试创建；失败则跳过
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("[QuickQuick] 无法创建配置目录，跳过自启动偏好应用: {e}");
                return;
            }
            dir.join("autostart.json")
        }
        Err(e) => {
            eprintln!("[QuickQuick] 无法获取配置目录，跳过自启动偏好应用: {e}");
            return;
        }
    };

    let pref = autostart::AutostartConfig::load_or_default(&config_path);
    let mgr = app.autolaunch();

    if pref.enabled {
        if let Err(e) = mgr.enable() {
            eprintln!("[QuickQuick] 自启动 enable 失败（不影响启动）: {e}");
        }
    } else if let Err(e) = mgr.disable() {
        eprintln!("[QuickQuick] 自启动 disable 失败（不影响启动）: {e}");
    }
}

/// 注册全局热键（history / translate）。
///
/// 热键值取自 `HotkeyConfig::default()`。注册失败时仅记录警告，不阻断启动——
/// 错误在函数内部消化，永不向上传播。
fn register_hotkeys(handle: &tauri::AppHandle) {
    let config = hotkey::HotkeyConfig::default();
    let history_key = config.get_accelerator(hotkey::HotkeyAction::History).to_string();
    let translate_key = config.get_accelerator(hotkey::HotkeyAction::Translate).to_string();

    let handle_history = handle.clone();
    let handle_translate = handle.clone();

    // 注册 history 热键；失败则记录并优雅降级
    if let Err(e) = handle.global_shortcut().on_shortcut(history_key.as_str(), move |_app, _shortcut, _event| {
        trigger_window(&handle_history, "history");
    }) {
        eprintln!("[QuickQuick] history 热键注册失败（可能已被占用）: {e}");
    }

    // 注册 translate 热键；失败则记录并优雅降级
    if let Err(e) = handle.global_shortcut().on_shortcut(translate_key.as_str(), move |_app, _shortcut, _event| {
        trigger_window(&handle_translate, "translate");
    }) {
        eprintln!("[QuickQuick] translate 热键注册失败（可能已被占用）: {e}");
    }
}

/// 热键触发时：定位窗口 → 显示 → 聚焦 → 向前端发送路由事件。
fn trigger_window(handle: &tauri::AppHandle, route: &'static str) {
    let Some(window) = handle.get_webview_window("main") else {
        eprintln!("[QuickQuick] 找不到 main 窗口");
        return;
    };

    // 计算目标位置（光标所在显示器水平居中、靠上约 15%）
    let position = window_pos::compute_window_position(&window);

    if let Err(e) = window.set_position(position) {
        eprintln!("[QuickQuick] 设置窗口位置失败: {e}");
    }
    if let Err(e) = window.show() {
        eprintln!("[QuickQuick] 显示窗口失败: {e}");
        return;
    }
    if let Err(e) = window.set_focus() {
        eprintln!("[QuickQuick] 设置窗口焦点失败: {e}");
    }
    if let Err(e) = handle.emit("route", route) {
        eprintln!("[QuickQuick] emit route 事件失败: {e}");
    }
}

/// 监听主窗口失焦事件，失焦后自动隐藏。
fn setup_window_focus_hide(app: &mut tauri::App) -> Result<(), tauri::Error> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    // 克隆一份供闭包持有，避免与外层借用冲突
    let win = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            // 失去焦点时隐藏窗口
            let _ = win.hide();
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 冒烟测试：验证 HotkeyConfig 默认值非空且与设计文档约定一致（A04 后端侧）
    #[test]
    fn lib_default_hotkey_config_sane() {
        // Arrange
        let config = hotkey::HotkeyConfig::default();

        // Act
        let history = config.get_accelerator(hotkey::HotkeyAction::History);
        let translate = config.get_accelerator(hotkey::HotkeyAction::Translate);

        // Assert: 两字段非空且等于设计文档规定值
        assert!(!history.is_empty(), "history 热键不应为空");
        assert!(!translate.is_empty(), "translate 热键不应为空");
        assert_eq!(history, "CmdOrCtrl+Shift+V", "history 热键应为 CmdOrCtrl+Shift+V");
        assert_eq!(translate, "CmdOrCtrl+Shift+T", "translate 热键应为 CmdOrCtrl+Shift+T");
    }

    /// 验证热键默认值符合设计文档要求
    #[test]
    fn hotkey_defaults_match_spec() {
        let config = hotkey::HotkeyConfig::default();
        assert_eq!(
            config.get_accelerator(hotkey::HotkeyAction::History),
            "CmdOrCtrl+Shift+V",
            "history 热键默认值应为 CmdOrCtrl+Shift+V"
        );
        assert_eq!(
            config.get_accelerator(hotkey::HotkeyAction::Translate),
            "CmdOrCtrl+Shift+T",
            "translate 热键默认值应为 CmdOrCtrl+Shift+T"
        );
    }
}
