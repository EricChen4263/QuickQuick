//! QuickQuick Tauri 应用核心库
//!
//! 负责初始化 Tauri builder、注册插件，以及挂载 IPC 命令。
//! 二进制入口（main.rs）仅调用本模块的 `run()`，保持入口文件极简。
//!
//! 子模块：
//! - `hotkey`：全局热键配置、改键持久化与冲突检测
//! - `ipc`：所有 Tauri 命令（clipboard / translate / settings）
//! - `pipeline`：启动数据管道（open_app_db / capture_and_ingest / ArboardBackend）
//! - `popover`：popover 窗口的显示/隐藏与定位逻辑（clip-popover / trans-popover）
//! - `settings`：应用配置（排除名单、provider 选择）
//! - `tray`：系统托盘菜单构建（setup 阶段调用）
//! - `window_pos`：预热窗口定位逻辑（光标所在显示器水平居中、靠上 15%）

pub mod autostart;
pub mod clipboard;
pub mod db;
pub mod hotkey;
pub mod image;
pub mod ipc;
pub mod keyprovider;
pub mod onboarding;
pub mod paste;
pub mod pipeline;
pub mod portable;
pub mod privacy;
pub mod settings;
pub mod translate;
mod popover;
mod tray;
mod window_pos;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};

use tauri::{Manager, Runtime, WindowEvent};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// 运行时捕获状态：通过原子布尔在主线程与轮询线程间共享，无锁读写。
///
/// 由 setup 阶段从 `AppSettings` 初始化，IPC 命令层通过 `app.state::<CaptureState>()`
/// 读写，轮询线程通过克隆的 `Arc` 读取。
pub struct CaptureState {
    /// 是否暂停剪贴板捕获
    pub paused: Arc<AtomicBool>,
    /// 是否跳过 concealed/transient 敏感内容
    pub skip_sensitive: Arc<AtomicBool>,
    /// 失焦时是否隐藏到托盘（false = 退出应用）
    pub stay_in_tray: Arc<AtomicBool>,
    /// App 排除名单：IPC set_exclude_list 写入后即时生效，无需重启。
    ///
    /// 用 RwLock 而非原子布尔：名单是 HashSet<String>，不可原子操作。
    /// 读多写少（500ms 读一次，用户主动设置时才写），RwLock 读锁无竞争开销可忽略。
    pub exclude_list: Arc<RwLock<privacy::ExcludeList>>,
}

/// 将所有插件注册到给定的 builder 并返回。
///
/// 注册的插件：
/// - `tauri-plugin-autostart`：开机自启（行为由运行期 `apply_autostart_preference` 控制）
/// - `tauri-plugin-updater`：应用自动更新（endpoints 在 tauri.conf.json 配置）
/// - `tauri-plugin-global-shortcut`：全局热键注册
///
/// 函数签名对 Runtime 泛型化，允许生产代码（`Wry`）和测试（`MockRuntime`）共用同一套
/// 插件注册逻辑，避免测试与生产漂移。
pub fn register_plugins<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
}

/// 启动 Tauri 应用。
///
/// 插件注册委托给 `register_plugins`；setup 阶段：
/// - 打开加密数据库并通过 `app.manage(AppDb(...))` 注册为 Tauri 状态
/// - 启动剪贴板轮询后台线程（500ms 间隔）
/// - 读取并应用自启动偏好（`apply_autostart_preference`）
/// - 注册全局热键（history / translate），从持久化文件读取，失败时仅记录不 panic
/// - 构建系统托盘菜单
/// - 监听主窗口失焦事件 → 自动隐藏
///
/// # Panics
/// 若 Tauri builder 初始化失败则 panic（属于不可恢复的启动错误）。
pub fn run() {
    register_plugins(tauri::Builder::default())
        .invoke_handler(tauri::generate_handler![
            ipc::clipboard::list_clip_items,
            ipc::clipboard::delete_clip_item,
            ipc::clipboard::toggle_favorite_clip,
            ipc::clipboard::get_clip_image_original,
            ipc::translate::translate_text,
            ipc::translate::list_translate_history,
            ipc::settings::get_hotkeys,
            ipc::settings::set_hotkey,
            ipc::settings::get_exclude_list,
            ipc::settings::set_exclude_list,
            ipc::settings::get_translate_providers,
            ipc::settings::get_selected_provider,
            ipc::settings::set_selected_provider,
            ipc::settings::get_pause_capture,
            ipc::settings::set_pause_capture,
            ipc::settings::get_skip_sensitive,
            ipc::settings::set_skip_sensitive,
            ipc::settings::get_stay_in_tray,
            ipc::settings::set_stay_in_tray,
            ipc::settings::get_auto_update,
            ipc::settings::set_auto_update,
            ipc::settings::get_theme,
            ipc::settings::set_theme,
            ipc::settings::get_image_threshold,
            ipc::settings::set_image_threshold,
            ipc::settings::get_launch_on_login,
            ipc::settings::set_launch_on_login,
            ipc::system::get_storage_stats,
            ipc::system::cleanup_history,
            ipc::system::open_accessibility_settings,
            ipc::system::paste_to_front,
        ])
        .setup(|app| {
            setup_app_db(app);
            let capture_state = init_capture_state(app);
            // 提前克隆 Arc，让 capture_state 的所有权移交 manage 之前完成
            let paused = Arc::clone(&capture_state.paused);
            let skip_sensitive = Arc::clone(&capture_state.skip_sensitive);
            let stay_in_tray = Arc::clone(&capture_state.stay_in_tray);
            let exclude_list = Arc::clone(&capture_state.exclude_list);
            app.manage(capture_state);
            start_clipboard_poll(app.handle().clone(), paused, skip_sensitive, exclude_list);
            apply_autostart_preference(app);
            register_hotkeys(app.handle());
            tray::setup_tray(app)?;
            setup_window_focus_hide(app, stay_in_tray)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

/// 打开加密数据库并注册为 Tauri 状态。
///
/// 使用 KeychainKeyProvider 取得 SQLCipher 密钥，数据库文件放在 app_config_dir()。
/// 无论开库成功与否，都调用 `app.manage(AppDb(...))`：成功放 `Some(conn)`，失败放 `None`。
/// 这保证 Tauri 状态始终已注册，避免前端 invoke 时因状态缺失在 dispatch 层 panic。
/// 开库失败时仅 eprintln 记录原因，不 panic——IPC 命令将通过 `with_db` 返回 Err 而非崩溃。
fn setup_app_db(app: &mut tauri::App) {
    let conn_opt = match app.path().app_config_dir() {
        Ok(dir) => {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("[QuickQuick] 无法创建配置目录，数据库将不可用: {e}");
                None
            } else {
                let db_path = dir.join("quickquick.db");
                let provider = keyprovider::KeychainKeyProvider::new();
                match pipeline::open_app_db(&provider, &db_path) {
                    Ok(conn) => Some(conn),
                    Err(e) => {
                        eprintln!("[QuickQuick] 数据库打开失败，IPC 命令将返回错误而非崩溃: {e}");
                        None
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("[QuickQuick] 无法获取配置目录，数据库将不可用: {e}");
            None
        }
    };

    // 无论 conn_opt 是 Some 还是 None，都注册状态，防止 dispatch 层 panic
    app.manage(ipc::AppDb(Mutex::new(conn_opt)));
}

/// 启动剪贴板轮询后台线程（500ms 间隔）。
///
/// 线程持有 AppHandle，每次迭代通过 state::<AppDb>() 取得数据库锁后写入。
/// ArboardBackend 在 headless 环境下初始化可能失败，此时仅记录警告不启动线程。
/// `paused` 和 `skip_sensitive` 在每次迭代通过 `Relaxed` load 读取，运行时生效。
///
/// # 为什么不在线程内 panic
/// 后台线程崩溃不会影响主线程，但会导致轮询静默停止；
/// 捕获错误并 eprintln 保证可见性。
fn start_clipboard_poll(
    handle: tauri::AppHandle,
    paused: Arc<AtomicBool>,
    skip_sensitive: Arc<AtomicBool>,
    exclude_list: Arc<RwLock<privacy::ExcludeList>>,
) {
    let backend = match pipeline::ArboardBackend::new() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[QuickQuick] 剪贴板后端初始化失败，轮询不启动: {e}");
            return;
        }
    };

    std::thread::spawn(move || {
        let mut last_seen: u64 = 0;

        loop {
            std::thread::sleep(std::time::Duration::from_millis(
                clipboard::POLL_INTERVAL_MS,
            ));

            let Ok(state) = handle.try_state::<ipc::AppDb>().ok_or(()) else {
                continue;
            };
            let Ok(guard) = state.0.lock() else {
                continue;
            };
            // 数据库不可用时静默跳过本轮，不 panic
            let Some(conn) = guard.as_ref() else {
                continue;
            };

            // 每轮取读锁获取当前排除名单；lock poison 时跳过本轮不 panic
            let Ok(exclude_guard) = exclude_list.read() else {
                continue;
            };

            let policy = privacy::CapturePolicy {
                paused: paused.load(Ordering::Relaxed),
                skip_sensitive: skip_sensitive.load(Ordering::Relaxed),
                exclude: &*exclude_guard,
            };

            // 每轮从 settings.json 读取当前图片阈值；ingest 只在真正捕获到新图时发生，
            // 读文件开销可接受；失败时回退到默认值（20MiB），不中断轮询。
            let max_image_bytes = handle
                .path()
                .app_config_dir()
                .ok()
                .map(|dir| {
                    settings::AppSettings::load_or_default(&dir.join("settings.json"))
                        .max_image_bytes
                })
                .unwrap_or_else(|| settings::AppSettings::default().max_image_bytes);

            match pipeline::capture_and_ingest(&backend, &mut last_seen, conn, &policy, max_image_bytes) {
                Ok(outcomes) => {
                    let _ = outcomes;
                }
                Err(e) => {
                    eprintln!("[QuickQuick] 剪贴板写库失败（非致命）: {e}");
                }
            }
        }
    });
}

/// 读取自启动偏好配置并应用到 OS LaunchAgent。
///
/// 配置路径优先用 `app_config_dir()`；若拿不到则跳过（仅 eprintln 不 panic）。
/// 实际 enable/disable 逻辑委托给 `autostart::apply_to_os`，与 IPC 命令层共享。
fn apply_autostart_preference(app: &mut tauri::App) {
    let config_path = match app.path().app_config_dir() {
        Ok(dir) => {
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
    autostart::apply_to_os(&app.handle().clone(), pref.enabled);
}

/// 注册全局热键（history / translate）。
///
/// 热键值优先从 `app_config_dir()/hotkey.json` 读取持久化配置，
/// 文件不存在或读取失败时回退 `HotkeyConfig::default()`，
/// 使用户通过 set_hotkey 改键后重启仍可生效。
/// 注册失败时仅记录警告，不阻断启动——错误在函数内部消化，永不向上传播。
fn register_hotkeys(handle: &tauri::AppHandle) {
    let config = handle
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("hotkey.json"))
        .and_then(|path| {
            if path.exists() {
                hotkey::HotkeyConfig::load(&path).ok()
            } else {
                None
            }
        })
        .unwrap_or_default();
    let history_key = config
        .get_accelerator(hotkey::HotkeyAction::History)
        .to_string();
    let translate_key = config
        .get_accelerator(hotkey::HotkeyAction::Translate)
        .to_string();

    let handle_history = handle.clone();
    let handle_translate = handle.clone();

    // 注册 history 热键；失败则记录并优雅降级
    if let Err(e) = handle.global_shortcut().on_shortcut(
        history_key.as_str(),
        move |_app, _shortcut, _event| {
            popover::trigger_popover(&handle_history, "clip-popover");
        },
    ) {
        eprintln!("[QuickQuick] history 热键注册失败（可能已被占用）: {e}");
    }

    // 注册 translate 热键；失败则记录并优雅降级
    if let Err(e) = handle.global_shortcut().on_shortcut(
        translate_key.as_str(),
        move |_app, _shortcut, _event| {
            popover::trigger_popover(&handle_translate, "trans-popover");
        },
    ) {
        eprintln!("[QuickQuick] translate 热键注册失败（可能已被占用）: {e}");
    }
}


/// 从 `AppSettings` 初始化运行时 `CaptureState`。
///
/// 读取持久化的 settings.json（不存在则用默认值），将三个布尔字段
/// 转换为 `Arc<AtomicBool>`，供轮询线程与 IPC 命令层共享。
fn init_capture_state(app: &tauri::App) -> CaptureState {
    let settings = app
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| {
            let _ = std::fs::create_dir_all(&dir);
            dir.join("settings.json")
        })
        .map(|path| settings::AppSettings::load_or_default(&path))
        .unwrap_or_default();

    let exclude_list = privacy::ExcludeList::new_with_apps(
        settings.excluded_apps.iter().map(String::as_str),
    );

    CaptureState {
        paused: Arc::new(AtomicBool::new(settings.pause_capture)),
        skip_sensitive: Arc::new(AtomicBool::new(settings.skip_sensitive)),
        stay_in_tray: Arc::new(AtomicBool::new(settings.stay_in_tray)),
        exclude_list: Arc::new(RwLock::new(exclude_list)),
    }
}

/// 监听主窗口失焦事件：`stay_in_tray == true` 时隐藏窗口（默认行为），
/// `stay_in_tray == false` 时退出应用。
///
/// `stay_in_tray` 通过 `Arc<AtomicBool>` 传入，运行时 IPC 改值即刻生效。
fn setup_window_focus_hide(
    app: &mut tauri::App,
    stay_in_tray: Arc<AtomicBool>,
) -> Result<(), tauri::Error> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    let win = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            if stay_in_tray.load(Ordering::Relaxed) {
                let _ = win.hide();
            } else {
                win.app_handle().exit(0);
            }
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
        assert_eq!(
            history, "CmdOrCtrl+Shift+V",
            "history 热键应为 CmdOrCtrl+Shift+V"
        );
        assert_eq!(
            translate, "CmdOrCtrl+Shift+T",
            "translate 热键应为 CmdOrCtrl+Shift+T"
        );
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
