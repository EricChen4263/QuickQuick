//! 设置 IPC 命令层
//!
//! 模式：每个命令 = 薄的 `#[tauri::command]` 包装 + 可单测的纯函数 impl。
//! 单测只测 impl 函数（传显式路径 + fake registrar），命令层把错误映射为 String。
//!
//! 命令清单（前端通过 invoke 对应命令名调用，命名与 A09/S05 前端对齐）：
//! - `get_hotkeys`              — 读 HotkeyConfig，返回 { history, translate }
//! - `set_hotkey`               — load → rebind（冲突返回 Err）→ save
//! - `get_exclude_list`         — 读 AppSettings.excluded_apps
//! - `set_exclude_list`         — 写 AppSettings.excluded_apps 并 save
//! - `get_translate_providers`  — 返回 registry() 映射的 ProviderDto 列表
//! - `get_selected_provider`    — 读 AppSettings.selected_provider
//! - `set_selected_provider`    — 校验 id 在 registry 内，合法才写入
//! - `get_pause_capture`        — 读 CaptureState.paused AtomicBool
//! - `set_pause_capture`        — 写 AtomicBool + 持久化 pause_capture
//! - `get_skip_sensitive`       — 读 CaptureState.skip_sensitive AtomicBool
//! - `set_skip_sensitive`       — 写 AtomicBool + 持久化 skip_sensitive
//! - `get_stay_in_tray`         — 读 CaptureState.stay_in_tray AtomicBool
//! - `set_stay_in_tray`         — 写 AtomicBool + 持久化 stay_in_tray
//! - `get_auto_update`          — 读 AppSettings.auto_update（纯配置）
//! - `set_auto_update`          — 写 AppSettings.auto_update 并 save
//! - `get_theme`                — 读 AppSettings.theme
//! - `set_theme`                — 校验值∈{auto,light,dark}，合法才写入
//! - `get_launch_on_login`      — 读 autostart.json enabled
//! - `set_launch_on_login`      — 写 autostart.json + 调用 OS autolaunch

use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::RwLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use std::collections::HashMap;

use crate::autostart::AutostartConfig;
use crate::hotkey::{HotkeyAction, HotkeyConfig, HotkeyError, HotkeyRegistrar};
use crate::settings::AppSettings;
use crate::translate::credential::{
    credential_schema, default_cred_store, delete_credentials, load_credentials_for_display,
    save_credentials,
};
use crate::ipc::translate::resolve_provider_or_fallback;
use crate::translate::providers::registry;
use crate::CaptureState;

/// 热键配置 DTO（返回给前端）。
///
/// 字段用 camelCase 序列化，与前端 TypeScript 接口对齐。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyDto {
    pub history: String,
    pub translate: String,
}

/// 翻译 provider 能力 DTO（返回给前端）。
///
/// 字段用 camelCase 序列化，与前端 TypeScript 接口对齐。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDto {
    pub id: String,
    pub name: String,
    pub needs_key: bool,
    /// 是否为非官方/自建接口（序列化为 camelCase `isUnofficial`）。
    /// 前端据此渲染「非官方」标注与失败降级提示（设计文档§三.决策3）。
    pub is_unofficial: bool,
}

/// provider 凭据配置变化事件名。与前端 src/ipc/events.ts 的 PROVIDER_CONFIG_CHANGED_EVENT 必须一致。
/// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
const PROVIDER_CONFIG_CHANGED_EVENT: &str = "provider-config-changed";

/// 默认翻译源切换事件名。与前端 src/ipc/events.ts 的 SELECTED_PROVIDER_CHANGED_EVENT 必须一致。
/// 设置页与翻译页各自缓存当前默认 provider，一方改动后 emit 此事件令另一方刷新；
/// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
const SELECTED_PROVIDER_CHANGED_EVENT: &str = "selected-provider-changed";

/// 将 `action` 字符串解析为 `HotkeyAction`。
///
/// 只接受 "history" / "translate" 两个合法值，其余返回 Err。
/// 集中在此处做边界校验，impl 函数不再重复处理。
fn parse_action(action: &str) -> Result<HotkeyAction, String> {
    match action {
        "history" => Ok(HotkeyAction::History),
        "translate" => Ok(HotkeyAction::Translate),
        other => Err(format!(
            "未知 action：{other}，合法值为 history / translate"
        )),
    }
}

/// `get_hotkeys` 的纯函数实现，可在测试中直接调用。
///
/// 从 `hotkey_path` 加载（文件不存在则用默认值），将两个动作的加速键封装为 DTO 返回。
///
/// # Errors
/// 文件存在但内容不合法（JSON 损坏）时返回错误字符串。
pub fn get_hotkeys_impl(hotkey_path: &Path) -> Result<HotkeyDto, String> {
    let config = if hotkey_path.exists() {
        HotkeyConfig::load(hotkey_path).map_err(|e| e.to_string())?
    } else {
        HotkeyConfig::default()
    };

    Ok(HotkeyDto {
        history: config.get_accelerator(HotkeyAction::History).to_string(),
        translate: config.get_accelerator(HotkeyAction::Translate).to_string(),
    })
}

/// `set_hotkey` 的纯函数实现，可在测试中直接调用。
///
/// 流程：加载（文件不存在则用默认值）→ rebind（含冲突检测）→ save。
/// 冲突时 rebind 返回 AlreadyInUse，映射为包含"已被占用"的错误字符串。
///
/// # Errors
/// - 冲突：错误字符串含"已被占用"
/// - 文件读写失败：错误字符串描述 I/O 问题
pub fn set_hotkey_impl(
    action: HotkeyAction,
    accelerator: &str,
    hotkey_path: &Path,
    registrar: &dyn HotkeyRegistrar,
) -> Result<(), String> {
    let mut config = if hotkey_path.exists() {
        HotkeyConfig::load(hotkey_path).map_err(|e| e.to_string())?
    } else {
        HotkeyConfig::default()
    };

    config
        .rebind(action, accelerator, registrar)
        .map_err(|e| e.to_string())?;
    config.save(hotkey_path).map_err(|e| e.to_string())
}

/// `get_exclude_list` 的纯函数实现，可在测试中直接调用。
///
/// 从 `settings_path` load_or_default，返回排除应用列表。
///
/// # Errors
/// 此函数实际上永不返回 `Err`：`AppSettings::load_or_default` 在文件不存在或
/// 内容损坏时均静默回退默认值，调用方可安全地 `unwrap_or_default`。
/// 签名保留 `Result` 以与命令层 `?` 传播保持一致，便于未来升级为真正可失败路径。
pub fn get_exclude_list_impl(settings_path: &Path) -> Result<Vec<String>, String> {
    let settings = AppSettings::load_or_default(settings_path);
    Ok(settings.excluded_apps)
}

/// `set_exclude_list` 的纯函数实现，可在测试中直接调用。
///
/// 读取现有设置（保留其他字段），仅替换 excluded_apps 后 save。
/// 写文件成功后，若传入 `runtime_list`，同步替换运行时 Arc 中的名单，
/// 使轮询线程在下一次迭代即刻生效，无需重启。
///
/// # Errors
/// 文件写入失败时返回错误字符串（写入失败则不更新运行时，保持一致性）。
pub fn set_exclude_list_impl(
    list: Vec<String>,
    settings_path: &Path,
    runtime_list: Option<&RwLock<crate::privacy::ExcludeList>>,
) -> Result<(), String> {
    let mut settings = AppSettings::load_or_default(settings_path);
    settings.excluded_apps = list.clone();
    settings.save(settings_path).map_err(|e| e.to_string())?;

    if let Some(lock) = runtime_list {
        let new_exclude =
            crate::privacy::ExcludeList::new_with_apps(list.iter().map(String::as_str));
        match lock.write() {
            Ok(mut guard) => *guard = new_exclude,
            Err(e) => eprintln!("[QuickQuick] 排除名单写锁中毒，跳过运行时更新: {e}"),
        }
    }

    Ok(())
}

/// `get_translate_providers` 的纯函数实现，可在测试中直接调用。
///
/// 调用 `registry()` 并映射为 camelCase DTO 列表。
pub fn get_translate_providers_impl() -> Vec<ProviderDto> {
    registry()
        .into_iter()
        .map(|cap| ProviderDto {
            id: cap.id.to_string(),
            name: cap.name.to_string(),
            needs_key: cap.needs_key,
            is_unofficial: cap.is_unofficial,
        })
        .collect()
}

/// `get_selected_provider` 的纯函数实现，可在测试中直接调用。
///
/// 从 settings 文件读取当前选中的 provider id；文件不存在时返回默认值（lingva）。
/// 设置迁移（设计文档§六）：若持久化的 id 不在当前注册表内（含旧值 mymemory、
/// 任意已删除源），回退默认源并在写路径持久化修正，使后续读取稳定。
///
/// # Errors
/// 读取永不失败（`load_or_default` 静默回退默认值）；但写路径持久化修正失败时返回 Err。
pub fn get_selected_provider_impl(settings_path: &Path) -> Result<String, String> {
    let settings = AppSettings::load_or_default(settings_path);
    let resolved = resolve_provider_or_fallback(&settings.selected_provider);

    // 仅当发生回退（存储值非法）时才写回，避免每次读取都触发磁盘写。
    if resolved != settings.selected_provider {
        let mut migrated = settings;
        migrated.selected_provider = resolved.clone();
        migrated.save(settings_path).map_err(|e| e.to_string())?;
    }

    Ok(resolved)
}

/// `set_selected_provider` 的纯函数实现，可在测试中直接调用。
///
/// 校验 id 在 registry 内，合法才写入；非法 id 直接返回 Err，不触发文件写。
///
/// # Errors
/// - id 不在 registry 内：返回错误字符串（非法 provider id）
/// - 文件写入失败：返回错误字符串
pub fn set_selected_provider_impl(id: &str, settings_path: &Path) -> Result<(), String> {
    let is_valid = registry().iter().any(|cap| cap.id == id);
    if !is_valid {
        return Err(format!(
            "非法 provider id：{id}，合法值见 get_translate_providers"
        ));
    }

    let mut settings = AppSettings::load_or_default(settings_path);
    settings.selected_provider = id.to_string();
    settings.save(settings_path).map_err(|e| e.to_string())
}

/// 从 AppHandle 解析配置目录下的指定文件路径。
///
/// 若无法取得配置目录则返回 Err；目录不存在时尝试创建。
/// 此函数是不可单测的胶水层，仅供命令函数调用。
pub(crate) fn resolve_config_path(
    app: &AppHandle,
    filename: &str,
) -> Result<std::path::PathBuf, String> {
    Ok(resolve_config_dir(app)?.join(filename))
}

/// 返回 app_config_dir（必要时创建），用于构造 dev 文件密钥库等需要配置目录的对象。
///
/// 与 `resolve_config_path` 共享同一目录解析逻辑（后者在其基础上 join 文件名）。
/// 此函数是不可单测的胶水层，仅供命令函数调用。
pub(crate) fn resolve_config_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录：{e}"))?;

    let dir = apply_dev_subdir(&base, cfg!(debug_assertions));

    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建配置目录：{e}"))?;

    Ok(dir)
}

/// 按构建类型决定配置目录：debug 在 `base` 下追加 `dev` 子目录，release 用 `base`。
///
/// debug 构建落 dev 子目录，与 release（钥匙串密钥）的数据/密钥彻底隔离，
/// 消除同机 dev↔release 切换时 SQLCipher 密钥不匹配（file is not a database）。
/// release 路径不变（仍落 app_config_dir 根），已发布用户零迁移。
/// 抽成纯函数以可单测；`is_debug` 由调用方传 `cfg!(debug_assertions)`。
pub fn apply_dev_subdir(base: &std::path::Path, is_debug: bool) -> std::path::PathBuf {
    if is_debug {
        base.join("dev")
    } else {
        base.to_path_buf()
    }
}

/// 系统热键注册器：通过 Tauri global shortcut API 向 OS 注册热键。
///
/// 生产运行时使用；测试侧注入 fake 实现，无需启动 GUI。
struct SystemHotkeyRegistrar<'a> {
    app: &'a AppHandle,
}

impl HotkeyRegistrar for SystemHotkeyRegistrar<'_> {
    fn register(&self, accelerator: &str) -> Result<(), HotkeyError> {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        // is_registered 返回 bool；已注册则视为冲突，拒绝改绑。
        // 仅做冲突检测，不实际绑定回调（回调在 lib.rs setup 阶段统一绑定）。
        if self.app.global_shortcut().is_registered(accelerator) {
            Err(HotkeyError::AlreadyInUse)
        } else {
            Ok(())
        }
    }
}

/// Tauri 命令：读取热键配置，返回 { history, translate }。
#[tauri::command]
pub fn get_hotkeys(app: AppHandle) -> Result<HotkeyDto, String> {
    let path = resolve_config_path(&app, "hotkey.json")?;
    get_hotkeys_impl(&path)
}

/// Tauri 命令：将指定动作改绑到新加速键（含冲突检测 + 运行时即时应用）。
///
/// 流程：读出旧键（用于注销）→ 持久化新键 → 注销旧键 → 注册新键并绑回调。
/// 持久化失败时直接返回 Err，运行时状态不改动，保证一致性。
/// 运行时注册失败时，持久化已完成，重启后新键仍会生效，但会把错误反馈给前端。
#[tauri::command]
pub fn set_hotkey(app: AppHandle, action: String, accelerator: String) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let hotkey_action = parse_action(&action)?;
    let path = resolve_config_path(&app, "hotkey.json")?;

    // 读出旧加速键，供后续注销使用
    let old_accelerator = get_hotkeys_impl(&path).ok().map(|dto| match hotkey_action {
        HotkeyAction::History => dto.history,
        HotkeyAction::Translate => dto.translate,
    });

    // 持久化新键（含冲突检测），失败直接返回，运行时不动
    let registrar = SystemHotkeyRegistrar { app: &app };
    set_hotkey_impl(hotkey_action, &accelerator, &path, &registrar)?;

    // 注销旧键；「未注册」不视为错误（静默忽略），其他错误也仅记录
    if let Some(old) = old_accelerator {
        if old != accelerator {
            let _ = app.global_shortcut().unregister(old.as_str());
        }
    }

    // 注册新键并绑 popover 回调；失败映射为 String 返回前端
    crate::register_action_shortcut(&app, hotkey_action, &accelerator)
        .map_err(|e| format!("热键运行时注册失败（持久化已完成，重启后生效）: {e}"))
}

/// Tauri 命令：读取排除名单。
#[tauri::command]
pub fn get_exclude_list(app: AppHandle) -> Result<Vec<String>, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_exclude_list_impl(&path)
}

/// Tauri 命令：写入排除名单（持久化 + 运行时即时生效）。
#[tauri::command]
pub fn set_exclude_list(
    app: AppHandle,
    state: State<'_, CaptureState>,
    list: Vec<String>,
) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_exclude_list_impl(list, &path, Some(&state.exclude_list))
}

/// Tauri 命令：返回所有可用翻译 provider 列表。
#[tauri::command]
pub fn get_translate_providers() -> Vec<ProviderDto> {
    get_translate_providers_impl()
}

/// Tauri 命令：读取当前选中的翻译 provider id。
#[tauri::command]
pub fn get_selected_provider(app: AppHandle) -> Result<String, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_selected_provider_impl(&path)
}

/// Tauri 命令：设置翻译 provider（校验 id 合法性）。
///
/// 写入成功后 emit `selected-provider-changed`，让设置页与翻译页双向同步当前默认源
/// （任一页改动，另一页据事件重读刷新）。emit 失败仅记日志，不影响命令返回值。
#[tauri::command]
pub fn set_selected_provider(app: AppHandle, id: String) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_selected_provider_impl(&id, &path)?;
    if let Err(e) = app.emit(SELECTED_PROVIDER_CHANGED_EVENT, ()) {
        eprintln!("[QuickQuick] selected-provider-changed emit 失败: {e}");
    }
    Ok(())
}

/// `get_pause_capture` 的纯函数实现：从 AtomicBool 读取当前暂停状态。
pub fn get_pause_capture_impl(state: &CaptureState) -> bool {
    state.paused.load(Ordering::Relaxed)
}

/// `set_pause_capture` 的纯函数实现：写 AtomicBool 并持久化到 settings.json。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_pause_capture_impl(
    value: bool,
    state: &CaptureState,
    settings_path: &Path,
) -> Result<(), String> {
    state.paused.store(value, Ordering::Relaxed);
    let mut settings = AppSettings::load_or_default(settings_path);
    settings.pause_capture = value;
    settings.save(settings_path).map_err(|e| e.to_string())
}

/// `get_skip_sensitive` 的纯函数实现：从 AtomicBool 读取当前敏感跳过状态。
pub fn get_skip_sensitive_impl(state: &CaptureState) -> bool {
    state.skip_sensitive.load(Ordering::Relaxed)
}

/// `set_skip_sensitive` 的纯函数实现：写 AtomicBool 并持久化到 settings.json。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_skip_sensitive_impl(
    value: bool,
    state: &CaptureState,
    settings_path: &Path,
) -> Result<(), String> {
    state.skip_sensitive.store(value, Ordering::Relaxed);
    let mut settings = AppSettings::load_or_default(settings_path);
    settings.skip_sensitive = value;
    settings.save(settings_path).map_err(|e| e.to_string())
}

/// `get_stay_in_tray` 的纯函数实现：从 AtomicBool 读取当前托盘驻留状态。
pub fn get_stay_in_tray_impl(state: &CaptureState) -> bool {
    state.stay_in_tray.load(Ordering::Relaxed)
}

/// `set_stay_in_tray` 的纯函数实现：写 AtomicBool 并持久化到 settings.json。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_stay_in_tray_impl(
    value: bool,
    state: &CaptureState,
    settings_path: &Path,
) -> Result<(), String> {
    state.stay_in_tray.store(value, Ordering::Relaxed);
    let mut settings = AppSettings::load_or_default(settings_path);
    settings.stay_in_tray = value;
    settings.save(settings_path).map_err(|e| e.to_string())
}

/// `get_auto_update` 的纯函数实现：从 settings.json 读取 auto_update 值。
///
/// # Errors
/// 此函数实际上永不返回 Err（load_or_default 回退默认值）。
/// 签名保留 Result 与命令层保持一致。
pub fn get_auto_update_impl(settings_path: &Path) -> Result<bool, String> {
    let settings = AppSettings::load_or_default(settings_path);
    Ok(settings.auto_update)
}

/// `set_auto_update` 的纯函数实现：写 settings.json auto_update 字段。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_auto_update_impl(value: bool, settings_path: &Path) -> Result<(), String> {
    let mut settings = AppSettings::load_or_default(settings_path);
    settings.auto_update = value;
    settings.save(settings_path).map_err(|e| e.to_string())
}

/// `get_theme` 的纯函数实现：从 settings.json 读取 theme 值。
///
/// # Errors
/// 此函数实际上永不返回 Err（load_or_default 回退默认值）。
pub fn get_theme_impl(settings_path: &Path) -> Result<String, String> {
    let settings = AppSettings::load_or_default(settings_path);
    Ok(settings.theme)
}

/// `set_theme` 的纯函数实现：校验值合法后写入 settings.json。
///
/// 合法值：`auto` / `light` / `dark`；其余返回 Err，不触发文件写。
///
/// # Errors
/// - 非法 theme 值：返回包含合法值说明的错误字符串
/// - 文件写入失败：返回错误字符串
pub fn set_theme_impl(theme: &str, settings_path: &Path) -> Result<(), String> {
    if !matches!(theme, "auto" | "light" | "dark") {
        return Err(format!(
            "非法 theme 值：{theme}，合法值为 auto / light / dark"
        ));
    }
    let mut settings = AppSettings::load_or_default(settings_path);
    settings.theme = theme.to_string();
    settings.save(settings_path).map_err(|e| e.to_string())
}

/// 单张图片原图阈值合法区间下限（1 MiB）。
const MIN_IMAGE_THRESHOLD: u64 = 1024 * 1024;
/// 单张图片原图阈值合法区间上限（500 MiB）。
const MAX_IMAGE_THRESHOLD: u64 = 500 * 1024 * 1024;

/// `get_image_threshold` 的纯函数实现：从 settings.json 读取 max_image_bytes。
///
/// # Errors
/// 此函数实际上永不返回 `Err`：`AppSettings::load_or_default` 在文件不存在或
/// 内容损坏时均静默回退默认值，调用方可安全地 `unwrap_or_default`。
/// 签名保留 `Result` 以与命令层 `?` 传播保持一致，便于未来升级为真正可失败路径。
pub fn get_image_threshold_impl(settings_path: &Path) -> Result<u64, String> {
    let settings = AppSettings::load_or_default(settings_path);
    Ok(settings.max_image_bytes)
}

/// `set_image_threshold` 的纯函数实现：校验范围后写入 settings.json。
///
/// 合法区间：`1MiB ..= 500MiB`；越界返回中文 Err，不触发文件写。
///
/// # Errors
/// - 越界值：返回说明合法范围的错误字符串
/// - 文件写入失败：返回错误字符串
pub fn set_image_threshold_impl(bytes: u64, settings_path: &Path) -> Result<(), String> {
    if !(MIN_IMAGE_THRESHOLD..=MAX_IMAGE_THRESHOLD).contains(&bytes) {
        return Err(format!(
            "图片阈值超出范围：{bytes} 字节，合法范围为 {MIN_IMAGE_THRESHOLD}（1MiB）到 {MAX_IMAGE_THRESHOLD}（500MiB）字节"
        ));
    }
    let mut settings = AppSettings::load_or_default(settings_path);
    settings.max_image_bytes = bytes;
    settings.save(settings_path).map_err(|e| e.to_string())
}

/// `get_launch_on_login` 的纯函数实现：从 autostart.json 读取 enabled 值。
///
/// # Errors
/// 此函数实际上永不返回 Err（load_or_default 回退默认值）。
pub fn get_launch_on_login_impl(autostart_path: &Path) -> Result<bool, String> {
    let config = AutostartConfig::load_or_default(autostart_path);
    Ok(config.enabled)
}

/// `set_launch_on_login` 的纯函数实现（持久化部分）：写 autostart.json。
///
/// OS autolaunch 的 enable/disable 由命令层在写入后调用 `autostart::apply_to_os`
/// 完成，纯函数只负责持久化，保持可单测。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_launch_on_login_impl(value: bool, autostart_path: &Path) -> Result<(), String> {
    let mut config = AutostartConfig::load_or_default(autostart_path);
    config.enabled = value;
    config.save(autostart_path).map_err(|e| e.to_string())
}

/// Tauri 命令：读取当前暂停捕获状态。
#[tauri::command]
pub fn get_pause_capture(state: State<'_, CaptureState>) -> bool {
    get_pause_capture_impl(&state)
}

/// Tauri 命令：设置暂停捕获状态（运行时生效 + 持久化）。
#[tauri::command]
pub fn set_pause_capture(
    app: AppHandle,
    state: State<'_, CaptureState>,
    value: bool,
) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_pause_capture_impl(value, &state, &path)
}

/// Tauri 命令：读取当前敏感内容跳过状态。
#[tauri::command]
pub fn get_skip_sensitive(state: State<'_, CaptureState>) -> bool {
    get_skip_sensitive_impl(&state)
}

/// Tauri 命令：设置敏感内容跳过状态（运行时生效 + 持久化）。
#[tauri::command]
pub fn set_skip_sensitive(
    app: AppHandle,
    state: State<'_, CaptureState>,
    value: bool,
) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_skip_sensitive_impl(value, &state, &path)
}

/// Tauri 命令：读取当前托盘驻留状态。
#[tauri::command]
pub fn get_stay_in_tray(state: State<'_, CaptureState>) -> bool {
    get_stay_in_tray_impl(&state)
}

/// Tauri 命令：设置托盘驻留状态（运行时生效 + 持久化）。
#[tauri::command]
pub fn set_stay_in_tray(
    app: AppHandle,
    state: State<'_, CaptureState>,
    value: bool,
) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_stay_in_tray_impl(value, &state, &path)
}

/// Tauri 命令：读取自动更新开关。
#[tauri::command]
pub fn get_auto_update(app: AppHandle) -> Result<bool, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_auto_update_impl(&path)
}

/// Tauri 命令：设置自动更新开关。
#[tauri::command]
pub fn set_auto_update(app: AppHandle, value: bool) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_auto_update_impl(value, &path)
}

/// Tauri 命令：读取当前 UI 主题。
#[tauri::command]
pub fn get_theme(app: AppHandle) -> Result<String, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_theme_impl(&path)
}

/// Tauri 命令：设置 UI 主题（校验值合法性：auto / light / dark）。
#[tauri::command]
pub fn set_theme(app: AppHandle, theme: String) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_theme_impl(&theme, &path)
}

/// Tauri 命令：读取单张图片原图阈值（字节）。
#[tauri::command]
pub fn get_image_threshold(app: AppHandle) -> Result<u64, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_image_threshold_impl(&path)
}

/// Tauri 命令：设置单张图片原图阈值（字节，合法范围 1MiB–500MiB）。
#[tauri::command]
pub fn set_image_threshold(app: AppHandle, bytes: u64) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_image_threshold_impl(bytes, &path)
}

/// Tauri 命令：读取开机自启状态。
#[tauri::command]
pub fn get_launch_on_login(app: AppHandle) -> Result<bool, String> {
    let path = resolve_config_path(&app, "autostart.json")?;
    get_launch_on_login_impl(&path)
}

/// Tauri 命令：设置开机自启（持久化 autostart.json + 应用到 OS）。
#[tauri::command]
pub fn set_launch_on_login(app: AppHandle, value: bool) -> Result<(), String> {
    let path = resolve_config_path(&app, "autostart.json")?;
    set_launch_on_login_impl(value, &path)?;
    crate::autostart::apply_to_os(&app, value);
    Ok(())
}

/// provider 凭据字段 DTO（返回给前端，camelCase）。
///
/// 描述该字段是否为 secret、是否必填，供前端动态渲染凭据表单。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialFieldDto {
    /// 字段标识符（存取路由键）
    pub key: String,
    /// UI 显示标签
    pub label: String,
    /// 是否为 secret（true = 写 keychain，输入框用密码形式）
    pub is_secret: bool,
    /// 是否必填
    pub required: bool,
}

/// provider 凭据值 DTO（返回给前端，camelCase）。
///
/// 安全约定：secret 字段（is_secret=true）的 value 永远为 None，仅用 is_set 表示是否已存；
/// 非密字段（is_secret=false）的 value 为 Some(已存值) 或 None（未存）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialValueDto {
    /// 字段标识符
    pub key: String,
    /// 字段当前值：secret 字段永远为 None；非密字段为 Some(值) 或 None
    pub value: Option<String>,
    /// 是否已保存过（secret 字段靠此布尔判断；非密字段 value.is_some() 与此等价）
    pub is_set: bool,
}

/// `get_provider_credential_schema` 的纯函数实现，可在测试中直接调用。
///
/// 映射 `credential_schema` 返回的 `CredentialField` 列表为 DTO。
/// 未知 provider_id 返回空 Vec（不 Err），与 credential_schema 语义一致。
pub fn get_provider_credential_schema_impl(provider_id: &str) -> Vec<CredentialFieldDto> {
    credential_schema(provider_id)
        .into_iter()
        .map(|f| CredentialFieldDto {
            key: f.key.to_string(),
            label: f.label.to_string(),
            is_secret: f.is_secret,
            required: f.required,
        })
        .collect()
}

/// `get_provider_credentials` 的纯函数实现，可在测试中直接调用。
///
/// 走只读 DB 展示路径（`load_credentials_for_display`），绝不访问 keychain，
/// 遍历 schema 中每个字段组装 DTO 列表：
/// - secret 字段：value 永远为 None，is_set 表示 secret_presence 标记表中是否有该标记
/// - 非密字段：value 为 Some(已存值) 或 None（读加密 DB），is_set 与 value.is_some() 等价
///
/// # 安全
/// secret 字段的实际值永不出现在返回结果中；展示路径不读 keychain，故不触发钥匙串弹窗。
///
/// # Errors
/// DB 读取失败时返回错误字符串。
pub fn get_provider_credentials_impl(
    provider_id: &str,
    conn: &rusqlite::Connection,
) -> Result<Vec<CredentialValueDto>, String> {
    // 走只读 DB 展示路径：secret 字段靠 secret_presence 标记判断是否已配置，
    // 绝不读 keychain（否则设置页一打开就触发反复弹密码）。
    let loaded = load_credentials_for_display(provider_id, conn).map_err(|e| e.to_string())?;

    let result = credential_schema(provider_id)
        .into_iter()
        .map(|field| {
            let saved_value = loaded
                .iter()
                .find(|(k, _)| k == field.key)
                .map(|(_, v)| v.clone());

            if field.is_secret {
                CredentialValueDto {
                    key: field.key.to_string(),
                    value: None,
                    is_set: saved_value.is_some(),
                }
            } else {
                let is_set = saved_value.is_some();
                CredentialValueDto {
                    key: field.key.to_string(),
                    value: saved_value,
                    is_set,
                }
            }
        })
        .collect();

    Ok(result)
}

/// `set_provider_credentials` 的纯函数实现，可在测试中直接调用。
///
/// 将前端传入的 `HashMap<String, String>` 转为 save_credentials 所需的 `&[(&str, &str)]`，
/// 再调 save_credentials 按 schema 路由（secret→store，非密→DB）。
///
/// # 安全
/// - 函数/日志/错误消息绝不打印 secret 字段的值
/// - 未知 field_key 由 save_credentials 拒绝（返回 CredError::UnknownField）
///
/// # Errors
/// - 未知 provider_id：返回错误字符串
/// - 未知 field_key：返回错误字符串（不含字段值）
/// - store 写入失败 / DB 操作失败：返回错误字符串
pub fn set_provider_credentials_impl(
    provider_id: &str,
    values: HashMap<String, String>,
    store: &dyn crate::translate::credential::CredStore,
    conn: &rusqlite::Connection,
) -> Result<(), String> {
    let pairs: Vec<(&str, &str)> = values
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    save_credentials(provider_id, &pairs, store, conn).map_err(|e| e.to_string())
}

/// Tauri 命令：获取指定 provider 的凭据字段 schema。
#[tauri::command]
pub fn get_provider_credential_schema(provider_id: String) -> Vec<CredentialFieldDto> {
    get_provider_credential_schema_impl(&provider_id)
}

/// Tauri 命令：获取指定 provider 的已保存凭据（secret 字段只返回 is_set，不回明文）。
#[tauri::command]
pub fn get_provider_credentials(
    state: State<'_, super::AppDb>,
    provider_id: String,
) -> Result<Vec<CredentialValueDto>, String> {
    // 展示路径只读 DB（secret_presence 标记），不构造 store、不碰 keychain
    super::with_db(&state, |conn| {
        get_provider_credentials_impl(&provider_id, conn)
    })
}

/// Tauri 命令：保存指定 provider 的凭据（secret→keychain，非密→加密 DB）。
///
/// 保存成功后向前端 emit `provider-config-changed` 事件，
/// 使翻译页实时刷新 configuredIds、解禁已配置的 keyed 源选项。
/// emit 失败仅记录日志，不影响命令返回值。
#[tauri::command]
pub fn set_provider_credentials(
    app: AppHandle,
    state: State<'_, super::AppDb>,
    provider_id: String,
    values: HashMap<String, String>,
) -> Result<(), String> {
    let config_dir = resolve_config_dir(&app)?;
    let store = default_cred_store(&config_dir);
    super::with_db(&state, |conn| {
        set_provider_credentials_impl(&provider_id, values, &store, conn)
    })?;
    if let Err(e) = app.emit(PROVIDER_CONFIG_CHANGED_EVENT, ()) {
        eprintln!("[QuickQuick] provider-config-changed emit 失败: {e}");
    }
    Ok(())
}

/// `delete_provider_credentials` 的纯函数实现，可在测试中直接调用。
///
/// 按 schema 遍历 provider 所有字段：secret→store.delete_secret，非密→DELETE FROM DB。
/// 幂等——字段不存在也不报错。
///
/// # Errors
/// - 未知 provider_id：返回错误字符串
/// - store 删除失败 / DB 操作失败：返回错误字符串
pub fn delete_provider_credentials_impl(
    provider_id: &str,
    store: &dyn crate::translate::credential::CredStore,
    conn: &rusqlite::Connection,
) -> Result<(), String> {
    delete_credentials(provider_id, store, conn).map_err(|e| e.to_string())
}

/// Tauri 命令：清除指定 provider 的所有已保存凭据（keychain + DB 均清）。
///
/// 删除成功后向前端 emit `provider-config-changed` 事件，
/// 使翻译页徽标从「已配置」变回「待配置」。
/// emit 失败仅记录日志，不影响命令返回值。
#[tauri::command]
pub fn delete_provider_credentials(
    app: AppHandle,
    state: State<'_, super::AppDb>,
    provider_id: String,
) -> Result<(), String> {
    let config_dir = resolve_config_dir(&app)?;
    let store = default_cred_store(&config_dir);
    super::with_db(&state, |conn| {
        delete_provider_credentials_impl(&provider_id, &store, conn)
    })?;
    if let Err(e) = app.emit(PROVIDER_CONFIG_CHANGED_EVENT, ()) {
        eprintln!("[QuickQuick] provider-config-changed emit 失败（delete）: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, Arc};
    use tempfile::NamedTempFile;

    fn make_state(paused: bool, skip_sensitive: bool, stay_in_tray: bool) -> CaptureState {
        use crate::privacy::ExcludeList;
        use std::sync::RwLock;
        CaptureState {
            paused: Arc::new(AtomicBool::new(paused)),
            skip_sensitive: Arc::new(AtomicBool::new(skip_sensitive)),
            stay_in_tray: Arc::new(AtomicBool::new(stay_in_tray)),
            exclude_list: Arc::new(RwLock::new(ExcludeList::default())),
        }
    }

    #[test]
    fn get_pause_capture_reads_atomic() {
        let state = make_state(true, true, true);
        assert!(get_pause_capture_impl(&state));
        state.paused.store(false, Ordering::Relaxed);
        assert!(!get_pause_capture_impl(&state));
    }

    #[test]
    fn set_pause_capture_writes_atomic_and_persists() {
        let state = make_state(false, true, true);
        let file = NamedTempFile::new().unwrap();
        set_pause_capture_impl(true, &state, file.path()).unwrap();
        assert!(state.paused.load(Ordering::Relaxed));
        let settings = AppSettings::load_or_default(file.path());
        assert!(settings.pause_capture);
    }

    #[test]
    fn get_skip_sensitive_reads_atomic() {
        let state = make_state(false, false, true);
        assert!(!get_skip_sensitive_impl(&state));
    }

    #[test]
    fn set_skip_sensitive_writes_atomic_and_persists() {
        let state = make_state(false, true, true);
        let file = NamedTempFile::new().unwrap();
        set_skip_sensitive_impl(false, &state, file.path()).unwrap();
        assert!(!state.skip_sensitive.load(Ordering::Relaxed));
        let settings = AppSettings::load_or_default(file.path());
        assert!(!settings.skip_sensitive);
    }

    #[test]
    fn get_stay_in_tray_reads_atomic() {
        let state = make_state(false, true, false);
        assert!(!get_stay_in_tray_impl(&state));
    }

    #[test]
    fn set_stay_in_tray_writes_atomic_and_persists() {
        let state = make_state(false, true, true);
        let file = NamedTempFile::new().unwrap();
        set_stay_in_tray_impl(false, &state, file.path()).unwrap();
        assert!(!state.stay_in_tray.load(Ordering::Relaxed));
        let settings = AppSettings::load_or_default(file.path());
        assert!(!settings.stay_in_tray);
    }

    #[test]
    fn get_auto_update_returns_default_when_no_file() {
        let file = NamedTempFile::new().unwrap();
        let nonexistent = file.path().with_extension("nonexistent_settings.json");
        let result = get_auto_update_impl(&nonexistent).unwrap();
        assert!(result, "auto_update 默认应为 true");
    }

    #[test]
    fn set_auto_update_persists_value() {
        let file = NamedTempFile::new().unwrap();
        set_auto_update_impl(false, file.path()).unwrap();
        assert!(!get_auto_update_impl(file.path()).unwrap());
    }

    #[test]
    fn get_theme_returns_default_when_no_file() {
        let file = NamedTempFile::new().unwrap();
        let nonexistent = file.path().with_extension("nonexistent_settings.json");
        assert_eq!(get_theme_impl(&nonexistent).unwrap(), "auto");
    }

    #[test]
    fn set_theme_rejects_invalid_value() {
        let file = NamedTempFile::new().unwrap();
        let err = set_theme_impl("purple", file.path()).unwrap_err();
        assert!(err.contains("非法 theme 值"), "错误信息应说明非法值：{err}");
    }

    #[test]
    fn set_theme_accepts_valid_values() {
        let file = NamedTempFile::new().unwrap();
        for theme in ["auto", "light", "dark"] {
            set_theme_impl(theme, file.path()).unwrap();
            assert_eq!(get_theme_impl(file.path()).unwrap(), theme);
        }
    }

    #[test]
    fn get_launch_on_login_returns_default_when_no_file() {
        let file = NamedTempFile::new().unwrap();
        let nonexistent = file.path().with_extension("nonexistent_autostart.json");
        let result = get_launch_on_login_impl(&nonexistent).unwrap();
        assert!(result, "launch_on_login 默认应为 true（设计§二）");
    }

    #[test]
    fn set_launch_on_login_persists_value() {
        let file = NamedTempFile::new().unwrap();
        set_launch_on_login_impl(false, file.path()).unwrap();
        assert!(!get_launch_on_login_impl(file.path()).unwrap());
        set_launch_on_login_impl(true, file.path()).unwrap();
        assert!(get_launch_on_login_impl(file.path()).unwrap());
    }

    #[test]
    fn set_pause_capture_preserves_existing_fields() {
        let state = make_state(false, true, true);
        let file = NamedTempFile::new().unwrap();
        let initial = AppSettings {
            excluded_apps: vec!["com.test.app".to_string()],
            selected_provider: "deepl".to_string(),
            ..AppSettings::default()
        };
        initial.save(file.path()).unwrap();
        set_pause_capture_impl(true, &state, file.path()).unwrap();
        let restored = AppSettings::load_or_default(file.path());
        assert_eq!(restored.excluded_apps, vec!["com.test.app"]);
        assert_eq!(restored.selected_provider, "deepl");
        assert!(restored.pause_capture);
    }

    #[test]
    fn set_exclude_list_persists_to_file() {
        let file = NamedTempFile::new().unwrap();
        let apps = vec![
            "com.1password.1password".to_string(),
            "com.foo.bar".to_string(),
        ];
        set_exclude_list_impl(apps.clone(), file.path(), None).unwrap();
        let settings = AppSettings::load_or_default(file.path());
        assert_eq!(settings.excluded_apps, apps);
    }

    #[test]
    fn get_image_threshold_returns_default_when_no_file() {
        let file = NamedTempFile::new().unwrap();
        let nonexistent = file.path().with_extension("nonexistent_settings.json");
        let result = get_image_threshold_impl(&nonexistent).unwrap();
        assert_eq!(result, 20 * 1024 * 1024, "默认阈值应为 20MiB");
    }

    #[test]
    fn set_image_threshold_persists_and_get_reads_back() {
        let file = NamedTempFile::new().unwrap();
        let threshold = 50 * 1024 * 1024u64;
        set_image_threshold_impl(threshold, file.path()).unwrap();
        let got = get_image_threshold_impl(file.path()).unwrap();
        assert_eq!(got, threshold, "set 后 get 应返回相同值");
    }

    #[test]
    fn set_image_threshold_rejects_too_small() {
        let file = NamedTempFile::new().unwrap();
        let err = set_image_threshold_impl(512 * 1024, file.path()).unwrap_err();
        assert!(err.contains("范围"), "错误信息应说明范围限制：{err}");
    }

    #[test]
    fn set_image_threshold_rejects_too_large() {
        let file = NamedTempFile::new().unwrap();
        let too_large = 600 * 1024 * 1024u64;
        let err = set_image_threshold_impl(too_large, file.path()).unwrap_err();
        assert!(err.contains("范围"), "错误信息应说明范围限制：{err}");
    }

    #[test]
    fn set_image_threshold_out_of_range_does_not_modify_file() {
        let file = NamedTempFile::new().unwrap();
        set_image_threshold_impl(30 * 1024 * 1024, file.path()).unwrap();
        let _ = set_image_threshold_impl(0, file.path()).unwrap_err();
        let got = get_image_threshold_impl(file.path()).unwrap();
        assert_eq!(got, 30 * 1024 * 1024, "越界 set 后文件不应被改动");
    }

    #[test]
    fn set_exclude_list_updates_runtime_immediately() {
        use crate::privacy::ExcludeList;
        use std::sync::RwLock;

        let lock = RwLock::new(ExcludeList::default());
        let apps = vec!["com.1password.1password".to_string()];
        let file = NamedTempFile::new().unwrap();

        // 写入前名单为空
        assert!(!lock.read().unwrap().contains("com.1password.1password"));

        set_exclude_list_impl(apps, file.path(), Some(&lock)).unwrap();

        // 写入后运行时名单立即包含新 app，无需重启
        assert!(
            lock.read().unwrap().contains("com.1password.1password"),
            "运行时排除名单应在 set_exclude_list 后立即生效"
        );
    }

    fn make_cred_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS provider_config (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
                value        TEXT NOT NULL,
                PRIMARY KEY (provider_id, field_key)
            );
            CREATE TABLE IF NOT EXISTS secret_presence (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
                PRIMARY KEY (provider_id, field_key)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn get_provider_credential_schema_impl_baidu_returns_two_fields() {
        let fields = get_provider_credential_schema_impl("baidu");
        assert_eq!(fields.len(), 2, "百度 schema 应有 2 个字段");
        let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
        assert!(keys.contains(&"app_id"), "应含 app_id 字段");
        assert!(keys.contains(&"secret_key"), "应含 secret_key 字段");
    }

    #[test]
    fn get_provider_credential_schema_impl_unknown_returns_empty() {
        let fields = get_provider_credential_schema_impl("unknown_provider");
        assert!(fields.is_empty(), "未知 provider 应返回空 schema");
    }

    #[test]
    fn get_provider_credentials_impl_unset_fields_are_not_set() {
        let conn = make_cred_db();

        let results = get_provider_credentials_impl("baidu", &conn).unwrap();
        assert_eq!(results.len(), 2, "百度凭据应返回 2 个字段");
        for item in &results {
            assert!(!item.is_set, "未保存时所有字段 is_set 应为 false");
            assert!(item.value.is_none(), "未保存时 value 应为 None");
        }
    }

    #[test]
    fn get_provider_credentials_impl_secret_field_value_is_always_none() {
        use crate::translate::credential::MockCredStore;
        use std::collections::HashMap;
        let store = MockCredStore::new();
        let conn = make_cred_db();

        let mut values = HashMap::new();
        values.insert("app_id".to_string(), "my_app_id".to_string());
        values.insert("secret_key".to_string(), "super_secret".to_string());
        set_provider_credentials_impl("baidu", values, &store, &conn).unwrap();

        let results = get_provider_credentials_impl("baidu", &conn).unwrap();
        let secret_field = results.iter().find(|f| f.key == "secret_key").unwrap();
        assert!(
            secret_field.value.is_none(),
            "secret 字段的 value 永远应为 None（不回明文）"
        );
        assert!(secret_field.is_set, "已保存的 secret 字段 is_set 应为 true");

        let non_secret = results.iter().find(|f| f.key == "app_id").unwrap();
        assert_eq!(
            non_secret.value,
            Some("my_app_id".to_string()),
            "非密字段已保存后应返回 Some(值)"
        );
        assert!(non_secret.is_set, "已保存的非密字段 is_set 应为 true");
    }

    #[test]
    fn set_provider_credentials_impl_persists_and_loadable() {
        use crate::translate::credential::MockCredStore;
        use std::collections::HashMap;
        let store = MockCredStore::new();
        let conn = make_cred_db();

        let mut values = HashMap::new();
        values.insert("app_id".to_string(), "test_id".to_string());
        values.insert("secret_key".to_string(), "test_secret".to_string());

        set_provider_credentials_impl("baidu", values, &store, &conn).unwrap();

        let results = get_provider_credentials_impl("baidu", &conn).unwrap();
        let app_id = results.iter().find(|f| f.key == "app_id").unwrap();
        assert_eq!(app_id.value, Some("test_id".to_string()));
        let secret_key = results.iter().find(|f| f.key == "secret_key").unwrap();
        assert!(secret_key.is_set);
        assert!(secret_key.value.is_none(), "secret 字段 value 永远 None");
    }

    #[test]
    fn set_provider_credentials_impl_unknown_field_returns_err() {
        use crate::translate::credential::MockCredStore;
        use std::collections::HashMap;
        let store = MockCredStore::new();
        let conn = make_cred_db();

        let mut values = HashMap::new();
        values.insert("nonexistent_field".to_string(), "val".to_string());

        let result = set_provider_credentials_impl("baidu", values, &store, &conn);
        assert!(result.is_err(), "未知 field_key 应返回 Err");
    }

    #[test]
    fn delete_provider_credentials_impl_clears_all_fields() {
        use crate::translate::credential::MockCredStore;
        use std::collections::HashMap;
        let store = MockCredStore::new();
        let conn = make_cred_db();

        let mut values = HashMap::new();
        values.insert("app_id".to_string(), "test_id".to_string());
        values.insert("secret_key".to_string(), "test_secret".to_string());
        set_provider_credentials_impl("baidu", values, &store, &conn).unwrap();

        delete_provider_credentials_impl("baidu", &store, &conn).unwrap();

        let results = get_provider_credentials_impl("baidu", &conn).unwrap();
        for item in &results {
            assert!(
                !item.is_set,
                "删除后所有字段 is_set 应为 false，实际：{item:?}"
            );
        }
    }

    #[test]
    fn delete_provider_credentials_impl_unknown_provider_returns_err() {
        use crate::translate::credential::MockCredStore;
        let store = MockCredStore::new();
        let conn = make_cred_db();

        let result = delete_provider_credentials_impl("unknown_xyz", &store, &conn);
        assert!(result.is_err(), "未知 provider 应返回 Err");
    }

    #[test]
    fn delete_provider_credentials_impl_idempotent() {
        use crate::translate::credential::MockCredStore;
        let store = MockCredStore::new();
        let conn = make_cred_db();

        let result = delete_provider_credentials_impl("baidu", &store, &conn);
        assert!(result.is_ok(), "空 provider 也应成功（幂等）: {result:?}");
    }

    // 对齐 acceptance TV1-F4-A01：DTO 透出 is_unofficial（前端据此渲染「非官方」标注）。
    // 非官方免 key 源 true、官方 keyed 源 false，与 capability 一致。
    #[test]
    fn get_translate_providers_impl_exposes_is_unofficial() {
        let dtos = get_translate_providers_impl();
        let find = |id: &str| {
            dtos.iter()
                .find(|d| d.id == id)
                .unwrap_or_else(|| panic!("DTO 列表应含 {id}"))
        };
        assert!(find("lingva").is_unofficial, "lingva 应标注非官方");
        assert!(find("bing").is_unofficial, "bing 应标注非官方");
        assert!(!find("baidu").is_unofficial, "baidu 为官方源");
        assert!(!find("google").is_unofficial, "google 官方源");
    }
}
