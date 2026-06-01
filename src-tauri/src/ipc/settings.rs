//! 设置 IPC 命令层
//!
//! 模式：每个命令 = 薄的 `#[tauri::command]` 包装 + 可单测的纯函数 impl。
//! 单测只测 impl 函数（传显式路径 + fake registrar），命令层把错误映射为 String。
//!
//! 命令清单（前端通过 invoke 对应命令名调用，命名与 A09/S05 前端对齐）：
//! - `get_hotkeys`              — 读 HotkeyConfig，返回 { history, translate }
//! - `set_hotkey`              — load → rebind（冲突返回 Err）→ save
//! - `get_exclude_list`        — 读 AppSettings.excluded_apps
//! - `set_exclude_list`        — 写 AppSettings.excluded_apps 并 save
//! - `get_translate_providers` — 返回 registry() 映射的 ProviderDto 列表
//! - `get_selected_provider`   — 读 AppSettings.selected_provider
//! - `set_selected_provider`   — 校验 id 在 registry 内，合法才写入

use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::hotkey::{HotkeyAction, HotkeyConfig, HotkeyError, HotkeyRegistrar};
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
/// 读取现有设置（保留 selected_provider），仅替换 excluded_apps 后 save。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_exclude_list_impl(list: Vec<String>, settings_path: &Path) -> Result<(), String> {
    let mut settings = AppSettings::load_or_default(settings_path);
    settings.excluded_apps = list;
    settings.save(settings_path).map_err(|e| e.to_string())
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

/// Tauri 命令：写入排除名单。
#[tauri::command]
pub fn set_exclude_list(app: AppHandle, list: Vec<String>) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_exclude_list_impl(list, &path)
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
