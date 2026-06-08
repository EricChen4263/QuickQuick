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
pub mod frontmost;
pub mod hotkey;
pub mod image;
pub mod ipc;
pub mod keyprovider;
pub mod macos_paste;
pub mod onboarding;
pub mod paste;
pub mod pipeline;
mod popover;
pub mod portable;
pub mod privacy;
pub mod settings;
pub mod translate;
mod tray;
mod window_pos;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};

use tauri::{Emitter, Manager, Runtime, WindowEvent};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_updater::UpdaterExt;

/// 剪贴板变化事件名。与前端 src/ipc/events.ts 的 CLIPBOARD_CHANGED_EVENT 必须一致。
/// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
const CLIPBOARD_CHANGED_EVENT: &str = "clipboard-changed";

/// 后台更新检查的首检延迟（秒）。
///
/// 启动后不立即查更新：先让开库、剪贴板轮询、托盘/窗口等启动 I/O 沉淀，
/// 避免与首屏抢网络/CPU；8s 足够覆盖冷启动且用户几乎无感。
pub const UPDATE_FIRST_CHECK_DELAY_SECS: u64 = 8;

/// 后台更新检查的轮询间隔（秒）= 6 小时。
///
/// 桌面端发版频率低，6h 轮询既能在合理时间内发现新版，又把网络/服务端压力降到极低；
/// 用户也可随时通过设置页手动检查，无需高频后台轮询。
pub const UPDATE_POLL_INTERVAL_SECS: u64 = 21600;

/// 判定本轮入库结果是否需要通知前端刷新列表。
///
/// Inserted（新条目）与 Bumped（已存在条目被提到最前）都改变了列表的内容或顺序，
/// 故二者均需通知；空结果（无剪贴板变化）不通知，避免无谓 IPC。
fn should_notify_clip_change(outcomes: &[db::IngestOutcome]) -> bool {
    !outcomes.is_empty()
}

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
///
/// 注意：`tauri-plugin-single-instance` **不在此列**。它依赖真实 runtime
/// （进程间锁），且官方要求必须最先注册；放进本泛型函数会污染 MockRuntime 测试路径，
/// 故单独在 `run()` 内以 desktop-only 守卫、最先注册（详见 `run()`）。
pub fn register_plugins<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // RT1-F2-S03：富文本预览链接点击走系统默认浏览器，防 webview 自身导航把 app 顶掉。
        .plugin(tauri_plugin_opener::init())
}

/// 启动 Tauri 应用。
///
/// 单实例语义：通过 `tauri-plugin-single-instance` 保证全机仅一个进程存活。
/// 第二个实例启动时立即退出，已运行实例在回调里显示并聚焦 main 窗口
/// （行为与托盘「显示 QuickQuick」一致），消除「开机自启 + 用户手动再开」导致的
/// 双进程（双剪贴板轮询线程、热键重复抢注册）。该插件必须最先注册（官方硬性要求）。
///
/// 插件注册委托给 `register_plugins`；setup 阶段：
/// - 打开加密数据库并通过 `app.manage(AppDb(...))` 注册为 Tauri 状态
/// - 启动剪贴板轮询后台线程（500ms 间隔）
/// - 读取并应用自启动偏好（`apply_autostart_preference`）
/// - 注册全局热键（history / translate / main），从持久化文件读取，失败时仅记录不 panic
/// - 构建系统托盘菜单
/// - 监听主窗口失焦事件 → 自动隐藏
///
/// # Panics
/// 若 Tauri builder 初始化失败则 panic（属于不可恢复的启动错误）。
pub fn run() {
    let builder = tauri::Builder::default();

    // single-instance 必须在所有其它插件之前注册（官方硬性要求）。
    // 第二个实例会立即退出，回调在已运行实例中触发——复用托盘的显示逻辑，
    // 把 main 窗口显示并聚焦。仅桌面端有意义，故用 cfg(desktop) 守卫。
    // 回调签名为 Fn(&AppHandle, Vec<String>, String)，依次是 app/argv/cwd，此处仅需 app。
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
        tray::show_and_focus_window(app);
    }));

    register_plugins(builder)
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
            ipc::settings::get_provider_credential_schema,
            ipc::settings::get_provider_credentials,
            ipc::settings::set_provider_credentials,
            ipc::settings::delete_provider_credentials,
            ipc::system::get_storage_stats,
            ipc::system::cleanup_history,
            ipc::system::open_accessibility_settings,
            ipc::system::paste_to_front,
            ipc::system::copy_clip_to_clipboard,
            ipc::system::hide_and_return_focus,
            ipc::update::check_for_updates,
            ipc::update::download_and_install_update,
            ipc::update::restart_app,
        ])
        .setup(|app| {
            setup_app_db(app);
            setup_ecdict_db(app);
            let capture_state = init_capture_state(app);
            // 提前克隆 Arc，让 capture_state 的所有权移交 manage 之前完成
            let paused = Arc::clone(&capture_state.paused);
            let skip_sensitive = Arc::clone(&capture_state.skip_sensitive);
            let stay_in_tray = Arc::clone(&capture_state.stay_in_tray);
            let exclude_list = Arc::clone(&capture_state.exclude_list);
            app.manage(capture_state);
            setup_frontmost_tracking(app);
            start_clipboard_poll(app.handle().clone(), paused, skip_sensitive, exclude_list);
            apply_autostart_preference(app);
            register_hotkeys(app.handle());
            tray::setup_tray(app)?;
            setup_main_window_behavior(app, stay_in_tray)?;
            // 启动即设 Accessory：QuickQuick 常驻后台、靠托盘+全局热键唤起，
            // 不在 Dock 显示图标（对标 Maccy/Paste）。配套 tray.rs 在显示窗口时
            // 先 activate 进程，确保 Accessory 下键盘焦点仍可达。
            #[cfg(target_os = "macos")]
            app.set_activation_policy(macos_startup_activation_policy());
            spawn_update_watcher(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

/// 打开加密数据库并注册为 Tauri 状态。
///
/// 三平台统一用 `LocalKeyProvider`（机器绑定本地密钥库，去 Keychain、永不弹密码）取得
/// SQLCipher 密钥，数据库文件放在 app_config_dir()。
/// 无论开库成功与否，都调用 `app.manage(AppDb(...))`：成功放 `Some(conn)`，失败放 `None`。
/// 这保证 Tauri 状态始终已注册，避免前端 invoke 时因状态缺失在 dispatch 层 panic。
/// 开库失败时仅 eprintln 记录原因，不 panic——IPC 命令将通过 `with_db` 返回 Err 而非崩溃。
fn setup_app_db(app: &mut tauri::App) {
    let conn_opt = match ipc::settings::resolve_config_dir(app.handle()) {
        Ok(dir) => open_db_with_reset(&dir),
        Err(e) => {
            eprintln!("[QuickQuick] 无法获取/创建配置目录，数据库将不可用: {e}");
            None
        }
    };

    // 无论 conn_opt 是 Some 还是 None，都注册状态，防止 dispatch 层 panic
    app.manage(ipc::AppDb(Mutex::new(conn_opt)));
}

/// 构造本地 ECDICT 词典 DAO 并注册为 Tauri 状态。
///
/// 库文件随应用打包于 `resources/ecdict.db`（见 tauri.conf.json 的 bundle.resources）。
/// 经 `resource_dir()` 解析其绝对路径；解析失败仅 eprintln 记录、用空路径占位注册，
/// 不 panic——`EcdictDb::lookup` 在库缺失时返回错误，前端按「源不可用」处理。
fn setup_ecdict_db(app: &mut tauri::App) {
    let db_path = match app.path().resource_dir() {
        Ok(dir) => dir.join("resources/ecdict.db"),
        Err(e) => {
            eprintln!("[QuickQuick] 无法解析资源目录，ECDICT 本地词典将不可用: {e}");
            std::path::PathBuf::new()
        }
    };
    app.manage(ipc::EcdictDbState(Arc::new(
        translate::ecdict_db::EcdictDb::new(db_path),
    )));
}

/// 用 `LocalKeyProvider` 开库；密钥/库迁移失败时做一次性重置（备份旧库 + 旧 master.key 后重建）。
///
/// 触发重置的两类失败（设计 §四#4）：
/// - 密钥解密失败：旧 master.key 来自异机或损坏，或老 release 密钥曾在 Keychain、新库无对应文件。
/// - `file is not a database`：旧库由不同密钥加密（如老 dev 密钥在 `dev/` 子目录），新密钥解不开。
///
/// 重置只发生一次：备份后用全新随机主密钥重建空库。预发布、单用户，剪贴板历史重置可接受
/// （翻译 secret 需用户到设置页重填一次，见 release note）。
fn open_db_with_reset(dir: &std::path::Path) -> Option<rusqlite::Connection> {
    let db_path = dir.join("quickquick.db");
    let provider = keyprovider::LocalKeyProvider::new(dir);

    match pipeline::open_app_db(&provider, &db_path) {
        Ok(conn) => Some(conn),
        Err(e) if is_resettable_open_error(&e) => {
            eprintln!("[QuickQuick] 开库失败，执行一次性重置（备份旧库与旧密钥后重建）: {e}");
            reset_and_reopen(dir, &db_path)
        }
        Err(e) => {
            eprintln!("[QuickQuick] 数据库打开失败，IPC 命令将返回错误而非崩溃: {e}");
            None
        }
    }
}

/// 判断开库错误是否属于「需一次性重置」（密钥解密失败 / 库非数据库格式）。
fn is_resettable_open_error(err: &str) -> bool {
    err.contains("密钥解密失败")
        || err.contains("file is not a database")
        || err.contains("not a database")
}

/// 执行一次性重置：备份旧库 + 旧 master.key，用新 LocalKeyProvider 重建空库。
fn reset_and_reopen(
    dir: &std::path::Path,
    db_path: &std::path::Path,
) -> Option<rusqlite::Connection> {
    if db_path.exists() {
        if let Err(e) = db::backup_corrupt_file(db_path) {
            eprintln!("[QuickQuick] 旧库备份失败，放弃重置: {e}");
            return None;
        }
    }
    let key_path = dir.join("master.key");
    if key_path.exists() {
        if let Err(e) = db::backup_corrupt_file(&key_path) {
            eprintln!("[QuickQuick] 旧 master.key 备份失败，放弃重置: {e}");
            return None;
        }
    }

    let provider = keyprovider::LocalKeyProvider::new(dir);
    match pipeline::open_app_db(&provider, db_path) {
        Ok(conn) => Some(conn),
        Err(e) => {
            eprintln!("[QuickQuick] 重置后重建数据库仍失败，IPC 命令将返回错误: {e}");
            None
        }
    }
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
                exclude: &exclude_guard,
            };

            // 每轮（约 500ms）读一次 settings.json 取 max_image_bytes。
            // settings.json < 1 KB，桌面端有 OS 页缓存，开销可接受；
            // 好处是用户在 UI 调整阈值后下一轮即生效，无需重启。
            // 读取或路径解析失败时回退默认值（20 MiB），不中断轮询。
            let max_image_bytes = ipc::settings::resolve_config_dir(&handle)
                .ok()
                .map(|dir| {
                    settings::AppSettings::load_or_default(&dir.join("settings.json"))
                        .max_image_bytes
                })
                .unwrap_or_else(|| settings::AppSettings::default().max_image_bytes);

            match pipeline::capture_and_ingest(
                &backend,
                &mut last_seen,
                conn,
                &policy,
                max_image_bytes,
            ) {
                Ok(outcomes) => {
                    // 有新条目或条目被置顶时通知前端刷新列表
                    if should_notify_clip_change(&outcomes) {
                        if let Err(e) = handle.emit(CLIPBOARD_CHANGED_EVENT, ()) {
                            eprintln!("[QuickQuick] 发送 {CLIPBOARD_CHANGED_EVENT} 事件失败: {e}");
                        }
                    }
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
    let config_path = match ipc::settings::resolve_config_dir(app.handle()) {
        Ok(dir) => dir.join("autostart.json"),
        Err(e) => {
            eprintln!("[QuickQuick] 无法获取/创建配置目录，跳过自启动偏好应用: {e}");
            return;
        }
    };

    let pref = autostart::AutostartConfig::load_or_default(&config_path);
    autostart::apply_to_os(&app.handle().clone(), pref.enabled);
}

/// 启动时该用的 macOS 激活策略：恒为 `Accessory`（后台不占 Dock 图标）。
///
/// 抽成具名小函数是为了给回归测试一个可断言的决策锚：真实的 Dock 隐藏行为
/// 是纯 GUI 状态、headless 无法验证，但「决策值是 Accessory 而非 Regular」可单测。
/// 若有人误改回 Regular，回归测试失败——见 docs/design/dock-icon-accessory.md（O3）。
#[cfg(target_os = "macos")]
pub fn macos_startup_activation_policy() -> tauri::ActivationPolicy {
    tauri::ActivationPolicy::Accessory
}

/// 启动后台更新 watcher：首检延迟后周期性检查更新。
///
/// 用 `async_runtime::spawn`（Tauri 的 tokio 运行时）而非系统线程，便于 `await`
/// updater 的异步检查。`already_ready` 用 `Arc<AtomicBool>` 跨循环保持去重状态：
/// 一旦某一轮发现可用更新即置位，后续轮次 `should_check` 返回 false，不再重复检查。
///
/// 判定为应检查时调用 `run_one_update_check`：经 S02 已实现真实下载安装，
/// 并在就绪后 emit `update://ready` 通知前端（首检延迟 + 长间隔轮询）。
fn spawn_update_watcher(app: tauri::AppHandle) {
    use std::sync::atomic::{AtomicBool, Ordering};

    let already_ready = Arc::new(AtomicBool::new(false));
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(
            UPDATE_FIRST_CHECK_DELAY_SECS,
        ))
        .await;
        loop {
            let enabled = read_auto_update_enabled(&app);
            if ipc::update::should_check(enabled, already_ready.load(Ordering::Relaxed)) {
                run_one_update_check(&app, &already_ready).await;
            }
            tokio::time::sleep(std::time::Duration::from_secs(UPDATE_POLL_INTERVAL_SECS)).await;
        }
    });
}

/// 读取 auto_update 开关；读失败时按"关闭"保守处理，并记录原因。
///
/// 复用 settings 域既有读法（resolve_config_path + get_auto_update_impl），不另造解析。
/// 读失败（如配置目录暂不可用）时返回 false，避免在状态未知时贸然发起网络检查。
fn read_auto_update_enabled(app: &tauri::AppHandle) -> bool {
    match ipc::settings::resolve_config_path(app, "settings.json")
        .and_then(|p| ipc::settings::get_auto_update_impl(&p))
    {
        Ok(enabled) => enabled,
        Err(e) => {
            eprintln!("update_watcher: 读取 auto_update 开关失败，本轮跳过：{e}");
            false
        }
    }
}

/// 执行一轮更新检查；发现可用更新则静默下载安装、广播就绪事件并置位去重。
///
/// updater 初始化或检查出错时仅 `eprintln!` 记录、不 panic，等下一轮重试。
/// 下载安装与 `update://ready` 事件复用 `ipc::update` 的薄封装（与手动命令共享）。
async fn run_one_update_check(
    app: &tauri::AppHandle,
    already_ready: &Arc<std::sync::atomic::AtomicBool>,
) {
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(e) => {
            eprintln!("update_watcher: updater 初始化失败：{e}");
            return;
        }
    };
    match updater.check().await {
        Ok(Some(update)) => {
            eprintln!(
                "update_watcher: 发现可用更新 {}，开始静默下载",
                update.version
            );
            // 复用 update 域的下载安装薄封装；成功后内部 emit update://ready 并置位去重。
            ipc::update::download_install_for_watcher(app, update, already_ready).await;
        }
        Ok(None) => {}
        Err(e) => eprintln!("update_watcher: 检查更新失败：{e}"),
    }
}

/// 注册"最近外部前台 app"追踪并把共享状态交给 Tauri 托管。
///
/// 托管 `Arc<LastExternalApp>`：观察者回调（主线程）与 paste_to_front 命令（命令线程）
/// 共用同一实例——前者写 pid、后者读 pid。Arc 让两侧持有同一 `Mutex<Option<i32>>`。
///
/// macOS：装 NSWorkspace 应用激活通知观察者（见 `register_frontmost_observer`）。
/// 非 macOS：仅托管空状态（观察者为 no-op），保证 `State<Arc<LastExternalApp>>` 始终可取。
fn setup_frontmost_tracking(app: &tauri::App) {
    let shared = Arc::new(frontmost::LastExternalApp::new());
    register_frontmost_observer(Arc::clone(&shared));
    app.manage(shared);
}

/// macOS：注册 NSWorkspace 应用激活通知观察者，记录最近一个非自身前台 app 的 pid。
///
/// 线程模型：观察者注册与回调均在主线程（setup 在主线程；通知投递到 mainQueue）。
/// 因此 set pid 发生在主线程，paste_to_front 命令线程随后读取。
///
/// 用 block 形式（`addObserverForName:object:queue:usingBlock:` + block2::RcBlock）
/// 而非 `define_class!` 自定义 NSObject：无需声明新类/管理 selector，最简且能编译。
///
/// 观察者 token 故意 `std::mem::forget`：观察者需存活至进程退出（与 app 同生命周期），
/// 不在任何时点反注册；泄漏一个常驻 token 是此场景的正确选择，非疏漏。
#[cfg(target_os = "macos")]
fn register_frontmost_observer(shared: Arc<frontmost::LastExternalApp>) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSWorkspace, NSWorkspaceDidActivateApplicationNotification};
    use objc2_foundation::NSNotification;

    // self_pid 在闭包外取一次：用于过滤掉 QuickQuick 自身激活通知。
    let self_pid = std::process::id() as i32;

    let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        // 通知对象由 AppKit 投递，引用在回调期间有效；转成安全引用读取 userInfo。
        let notification: &NSNotification = unsafe { notification.as_ref() };
        let Some(pid) = extract_activated_pid(notification) else {
            return;
        };
        if frontmost::should_record_pid(pid, self_pid) {
            shared.set(pid);
        }
    });

    // sharedWorkspace / notificationCenter 是安全 API；addObserver 标 unsafe（block 须 sendable，
    // 本 block 仅捕获 Arc 与 i32，满足要求），observer 对象唯一持有人是我们 forget 的 token。
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let observer: Retained<_> = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            None,
            &block,
        )
    };

    // 观察者须存活至进程退出；forget 防止 token 析构反注册。
    std::mem::forget(observer);
}

/// macOS：从激活通知的 userInfo 取出被激活 app 的 pid（processIdentifier）。
///
/// 路径：notification.userInfo → objectForKey(NSWorkspaceApplicationKey)
/// → downcast 为 NSRunningApplication → processIdentifier。
/// 任一环节缺失（无 userInfo / 无该 key / 类型不符）返回 None，调用方据此跳过记录。
#[cfg(target_os = "macos")]
fn extract_activated_pid(notification: &objc2_foundation::NSNotification) -> Option<i32> {
    use objc2::rc::Retained;
    use objc2_app_kit::{NSRunningApplication, NSWorkspaceApplicationKey};

    let user_info = notification.userInfo()?;
    // userInfo 是无类型 NSDictionary；用 NSWorkspaceApplicationKey 取关联的 NSRunningApplication。
    let value = user_info.objectForKey(unsafe { NSWorkspaceApplicationKey });
    let running_app: Retained<NSRunningApplication> = value?.downcast().ok()?;
    Some(running_app.processIdentifier())
}

/// 非 macOS：观察者为 no-op（无 NSWorkspace），仅保留签名让 setup 通用。
#[cfg(not(target_os = "macos"))]
fn register_frontmost_observer(_shared: Arc<frontmost::LastExternalApp>) {}

/// 将 `HotkeyAction` 映射为对应 popover 窗口的标签字符串。
///
/// 此映射是纯函数，与运行时解耦，可在单测中直接验证。
/// 标签值必须与前端 popover 窗口的 `label` 字段严格一致。
pub fn popover_label_for_action(action: hotkey::HotkeyAction) -> Option<&'static str> {
    match action {
        hotkey::HotkeyAction::History => Some("clip-popover"),
        hotkey::HotkeyAction::Translate => Some("trans-popover"),
        hotkey::HotkeyAction::Main => None,
    }
}

/// 注册单个动作的全局快捷键并绑定对应的 popover 回调。
///
/// 供启动期 `register_hotkeys` 和运行时改键 `set_hotkey` 命令共用，消除回调逻辑重复。
/// 失败时返回 `tauri_plugin_global_shortcut::Error`，由调用方决定处理策略
/// （启动期仅 eprintln 降级；改键时映射为 String 返回前端）。
pub fn register_action_shortcut(
    handle: &tauri::AppHandle,
    action: hotkey::HotkeyAction,
    accelerator: &str,
) -> Result<(), tauri_plugin_global_shortcut::Error> {
    let handle_cb = handle.clone();
    match popover_label_for_action(action) {
        Some(label) => {
            handle
                .global_shortcut()
                .on_shortcut(accelerator, move |_app, _shortcut, _event| {
                    popover::trigger_popover(&handle_cb, label);
                })
        }
        None => {
            handle
                .global_shortcut()
                .on_shortcut(accelerator, move |_app, _shortcut, _event| {
                    tray::show_and_focus_window(&handle_cb);
                })
        }
    }
}

/// 注册全局热键（history / translate / main）。
///
/// 热键值优先从 `app_config_dir()/hotkey.json` 读取持久化配置，
/// 文件不存在或读取失败时回退 `HotkeyConfig::default()`，
/// 使用户通过 set_hotkey 改键后重启仍可生效。
/// 注册失败时仅记录警告，不阻断启动——错误在函数内部消化，永不向上传播。
fn register_hotkeys(handle: &tauri::AppHandle) {
    let config = ipc::settings::resolve_config_dir(handle)
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

    for action in [
        hotkey::HotkeyAction::History,
        hotkey::HotkeyAction::Translate,
        hotkey::HotkeyAction::Main,
    ] {
        let key = config.get_accelerator(action).to_string();
        if let Err(e) = register_action_shortcut(handle, action, &key) {
            eprintln!(
                "[QuickQuick] {:?} 热键注册失败（可能已被占用）: {e}",
                action
            );
        }
    }
}

/// 从 `AppSettings` 初始化运行时 `CaptureState`。
///
/// 读取持久化的 settings.json（不存在则用默认值），将三个布尔字段
/// 转换为 `Arc<AtomicBool>`，供轮询线程与 IPC 命令层共享。
fn init_capture_state(app: &tauri::App) -> CaptureState {
    let settings = ipc::settings::resolve_config_dir(app.handle())
        .ok()
        .map(|dir| dir.join("settings.json"))
        .map(|path| settings::AppSettings::load_or_default(&path))
        .unwrap_or_default();

    let exclude_list =
        privacy::ExcludeList::new_with_apps(settings.excluded_apps.iter().map(String::as_str));

    CaptureState {
        paused: Arc::new(AtomicBool::new(settings.pause_capture)),
        skip_sensitive: Arc::new(AtomicBool::new(settings.skip_sensitive)),
        stay_in_tray: Arc::new(AtomicBool::new(settings.stay_in_tray)),
        exclude_list: Arc::new(RwLock::new(exclude_list)),
    }
}

/// 主窗口红绿灯（交通灯）按钮的逻辑坐标 (x, y)。
///
/// 自绘标题栏 `.qq-titlebar` 高 44px，macOS 红绿灯按钮约 14px。
/// - x=18：接近标题栏左缩进，不与「QuickQuick」文字重叠。
/// - y=15：几何居中为 (44-14)/2=15，按钮中线 ≈ 15+7=22 正对栏中线 22，与标题文字中线对齐。
///   （config 字段在隐藏窗口上常失效，故由 NSWindow 重定位兜底，不依赖 config。）
///
/// 抽成纯函数便于单测，定位副作用由 [`reposition_traffic_lights`] 在 macOS 守卫下执行。
pub fn traffic_light_logical_position() -> (f64, f64) {
    (18.0, 15.0)
}

/// macOS：把 NSWindow 的三颗红绿灯按钮整体移到 [`traffic_light_logical_position`] 给定位置。
///
/// 为何不用 tauri 的 `trafficLightPosition` config：该值在 `visible:false` 初始隐藏窗口上
/// 常被忽略（实测按钮偏高未居中）。这里直接拿底层 NSWindow 重设按钮 frame，可靠生效。
///
/// 坐标系：标题栏 superview 用左下原点（未翻转），(x, y) 以「距左、距顶」语义给出，
/// 故按钮 origin.y = 容器高 - y - 按钮高。三颗按钮保持原间距，整体平移、不改尺寸。
/// 任一句柄缺失（无 NSWindow / 无按钮 / 无 superview）则跳过、不 panic。
#[cfg(target_os = "macos")]
fn reposition_traffic_lights(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowButton};
    use objc2_foundation::NSPoint;

    let Ok(ns_window_ptr) = window.ns_window() else {
        eprintln!("重定位红绿灯：取 NSWindow 失败");
        return;
    };
    // ns_window() 返回的指针在窗口存活期间有效；转成 NSWindow 引用读取标准按钮。
    let ns_window: &NSWindow = unsafe { &*(ns_window_ptr as *const NSWindow) };

    let buttons = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ];
    let Some(close) = ns_window.standardWindowButton(buttons[0]) else {
        return;
    };
    let Some(container) = (unsafe { close.superview() }) else {
        return;
    };

    let (x, y) = traffic_light_logical_position();
    let container_height = container.frame().size.height;
    // 以最左的关闭按钮 origin.x 为基准，保持三颗按钮的相对间距整体平移。
    let base_x = close.frame().origin.x;
    for button_id in buttons {
        let Some(button) = ns_window.standardWindowButton(button_id) else {
            continue;
        };
        let frame = button.frame();
        let new_x = x + (frame.origin.x - base_x);
        let new_y = container_height - y - frame.size.height;
        button.setFrameOrigin(NSPoint::new(new_x, new_y));
    }
}

/// 非 macOS：无红绿灯按钮，no-op；保留签名让 setup 通用、cfg 对称。
#[cfg(not(target_os = "macos"))]
fn reposition_traffic_lights(_window: &tauri::WebviewWindow) {}

/// 监听主窗口失焦与关闭请求事件：
/// - 失焦（`Focused(false)`）：`stay_in_tray == true` 时隐藏窗口，否则退出应用。
/// - 关闭按钮（`CloseRequested`）：`stay_in_tray == true` 时拦截并隐藏到后台，
///   否则放行默认行为（退出），保持与失焦分支一致的语义。
///
/// 托盘「退出」菜单直接调用 `app_handle().exit(0)`，不经过此事件，两者解耦。
///
/// `stay_in_tray` 通过 `Arc<AtomicBool>` 传入，运行时 IPC 改值即刻生效。
fn setup_main_window_behavior(
    app: &mut tauri::App,
    stay_in_tray: Arc<AtomicBool>,
) -> Result<(), tauri::Error> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    // 红绿灯定位：config 的 trafficLightPosition 在 visible:false 初始隐藏窗口上常被忽略，
    // 故直接操作底层 NSWindow 显式重定位。失败不致命，仅记录后继续。
    #[cfg(target_os = "macos")]
    reposition_traffic_lights(&window);

    let win = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            if stay_in_tray.load(Ordering::Relaxed) {
                let _ = win.hide();
            } else {
                win.app_handle().exit(0);
            }
        }
        WindowEvent::CloseRequested { api, .. } if stay_in_tray.load(Ordering::Relaxed) => {
            api.prevent_close();
            let _ = win.hide();
            // 关主窗也应把前台焦点显式还给上一个外部 app（方案 C）；仅 hide 窗口会把焦点
            // 留在 QuickQuick 进程。app.hide() + 按 LastExternalApp 记录的 pid 激活，保留
            // stay_in_tray 语义（进程驻留托盘不退出），拿不到 pid 时降级隐式还焦、不 panic。
            ipc::system::return_focus_after_main_hide(win.app_handle());
        }
        _ => {}
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// History 动作应映射到 clip-popover 标签
    #[test]
    fn popover_label_for_history_action_returns_clip_popover() {
        // Arrange & Act
        let label = popover_label_for_action(hotkey::HotkeyAction::History);

        // Assert
        assert_eq!(label, Some("clip-popover"));
    }

    /// Translate 动作应映射到 trans-popover 标签
    #[test]
    fn popover_label_for_translate_action_returns_trans_popover() {
        // Arrange & Act
        let label = popover_label_for_action(hotkey::HotkeyAction::Translate);

        // Assert
        assert_eq!(label, Some("trans-popover"));
    }

    /// Main 动作不映射到 popover，应走主窗口显示逻辑
    #[test]
    fn popover_label_for_main_action_returns_none() {
        // Arrange & Act
        let label = popover_label_for_action(hotkey::HotkeyAction::Main);

        // Assert
        assert_eq!(label, None);
    }

    /// 冒烟测试：验证 HotkeyConfig 默认值非空且与设计文档约定一致（A04 后端侧）
    #[test]
    fn lib_default_hotkey_config_sane() {
        // Arrange
        let config = hotkey::HotkeyConfig::default();

        // Act
        let history = config.get_accelerator(hotkey::HotkeyAction::History);
        let translate = config.get_accelerator(hotkey::HotkeyAction::Translate);
        let main = config.get_accelerator(hotkey::HotkeyAction::Main);

        // Assert: 三字段非空且等于设计文档规定值
        assert!(!history.is_empty(), "history 热键不应为空");
        assert!(!translate.is_empty(), "translate 热键不应为空");
        assert!(!main.is_empty(), "main 热键不应为空");
        assert_eq!(
            history, "CmdOrCtrl+Shift+C",
            "history 热键应为 CmdOrCtrl+Shift+C"
        );
        assert_eq!(
            translate, "CmdOrCtrl+Shift+T",
            "translate 热键应为 CmdOrCtrl+Shift+T"
        );
        assert_eq!(main, "CmdOrCtrl+Shift+M", "main 热键应为 CmdOrCtrl+Shift+M");
    }

    /// 验证热键默认值符合设计文档要求
    #[test]
    fn hotkey_defaults_match_spec() {
        let config = hotkey::HotkeyConfig::default();
        assert_eq!(
            config.get_accelerator(hotkey::HotkeyAction::History),
            "CmdOrCtrl+Shift+C",
            "history 热键默认值应为 CmdOrCtrl+Shift+C"
        );
        assert_eq!(
            config.get_accelerator(hotkey::HotkeyAction::Translate),
            "CmdOrCtrl+Shift+T",
            "translate 热键默认值应为 CmdOrCtrl+Shift+T"
        );
        assert_eq!(
            config.get_accelerator(hotkey::HotkeyAction::Main),
            "CmdOrCtrl+Shift+M",
            "main 热键默认值应为 CmdOrCtrl+Shift+M"
        );
    }

    /// 红绿灯按钮应与 44px 自绘标题栏中线对齐
    #[test]
    fn traffic_light_position_matches_titlebar_center() {
        // Arrange & Act
        let position = traffic_light_logical_position();

        // Assert
        assert_eq!(position, (18.0, 15.0));
    }

    /// 空切片（无剪贴板变化）不应触发前端刷新
    #[test]
    fn should_notify_clip_change_empty_returns_false() {
        // Arrange
        let outcomes: &[db::IngestOutcome] = &[];

        // Act
        let result = should_notify_clip_change(outcomes);

        // Assert
        assert!(!result, "空切片不应触发通知");
    }

    /// 含有 Inserted 结果时应触发前端刷新
    #[test]
    fn should_notify_clip_change_with_inserted_returns_true() {
        // Arrange
        let outcomes = &[db::IngestOutcome::Inserted("abc".to_string())];

        // Act
        let result = should_notify_clip_change(outcomes);

        // Assert
        assert!(result, "含 Inserted 时应触发通知");
    }

    /// 含有 Bumped 结果时应触发前端刷新（已有条目被提到最前也改变了顺序）
    #[test]
    fn should_notify_clip_change_with_bumped_returns_true() {
        // Arrange
        let outcomes = &[db::IngestOutcome::Bumped("xyz".to_string())];

        // Act
        let result = should_notify_clip_change(outcomes);

        // Assert
        assert!(result, "含 Bumped 时应触发通知");
    }

    /// 混合多个结果时只要有任意一个非空就应触发前端刷新
    #[test]
    fn should_notify_clip_change_mixed_outcomes_returns_true() {
        // Arrange
        let outcomes = &[
            db::IngestOutcome::Inserted("id-1".to_string()),
            db::IngestOutcome::Bumped("id-2".to_string()),
        ];

        // Act
        let result = should_notify_clip_change(outcomes);

        // Assert
        assert!(result, "混合 Inserted+Bumped 时应触发通知");
    }
}
