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
use tauri::{AppHandle, Manager, State};

use crate::autostart::AutostartConfig;
use crate::hotkey::{HotkeyAction, HotkeyConfig, HotkeyError, HotkeyRegistrar};
use crate::CaptureState;
use crate::settings::AppSettings;
use crate::translate::providers::registry;

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
}

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
        let new_exclude = crate::privacy::ExcludeList::new_with_apps(
            list.iter().map(String::as_str),
        );
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
        })
        .collect()
}

/// `get_selected_provider` 的纯函数实现，可在测试中直接调用。
///
/// 从 settings 文件读取当前选中的 provider id；文件不存在时返回默认值 "mymemory"。
///
/// # Errors
/// 此函数实际上永不返回 `Err`：`AppSettings::load_or_default` 在文件不存在或
/// 内容损坏时均静默回退默认值，调用方可安全地 `unwrap_or_default`。
/// 签名保留 `Result` 以与命令层 `?` 传播保持一致，便于未来升级为真正可失败路径。
pub fn get_selected_provider_impl(settings_path: &Path) -> Result<String, String> {
    let settings = AppSettings::load_or_default(settings_path);
    Ok(settings.selected_provider)
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
fn resolve_config_path(app: &AppHandle, filename: &str) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录：{e}"))?;

    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建配置目录：{e}"))?;

    Ok(dir.join(filename))
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

/// Tauri 命令：将指定动作改绑到新加速键（含冲突检测）。
#[tauri::command]
pub fn set_hotkey(app: AppHandle, action: String, accelerator: String) -> Result<(), String> {
    let hotkey_action = parse_action(&action)?;
    let path = resolve_config_path(&app, "hotkey.json")?;
    let registrar = SystemHotkeyRegistrar { app: &app };
    set_hotkey_impl(hotkey_action, &accelerator, &path, &registrar)
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
#[tauri::command]
pub fn set_selected_provider(app: AppHandle, id: String) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_selected_provider_impl(&id, &path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, Arc};
    use tempfile::NamedTempFile;

    fn make_state(paused: bool, skip_sensitive: bool, stay_in_tray: bool) -> CaptureState {
        use std::sync::RwLock;
        use crate::privacy::ExcludeList;
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
        let apps = vec!["com.1password.1password".to_string(), "com.foo.bar".to_string()];
        set_exclude_list_impl(apps.clone(), file.path(), None).unwrap();
        let settings = AppSettings::load_or_default(file.path());
        assert_eq!(settings.excluded_apps, apps);
    }

    #[test]
    fn set_exclude_list_updates_runtime_immediately() {
        use std::sync::RwLock;
        use crate::privacy::ExcludeList;

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
}
