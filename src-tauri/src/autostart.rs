//! 自启动偏好配置模块
//!
//! 设计要点：
//! - `AutostartConfig` 仅管理偏好数据（enabled 字段），不持有也不调用插件句柄。
//! - 与真实 OS LaunchAgent 注册的解耦通过调用层（lib.rs setup）完成，使本模块 headless 可测。
//! - 持久化风格与 hotkey.rs 完全对齐：serde_json、thiserror 枚举错误、不 unwrap/panic。

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// 自启动相关错误
#[derive(Debug, Error)]
pub enum AutostartError {
    /// JSON 序列化/反序列化失败
    #[error("自启动配置序列化失败：{0}")]
    SerdeError(#[from] serde_json::Error),
    /// 文件 I/O 失败
    #[error("自启动配置文件读写失败：{0}")]
    IoError(#[from] std::io::Error),
    /// 配置文件存在但无法解析为 JSON 对象：**拒绝写入**（已先备份留痕）。
    ///
    /// 文案直接用 `settings` 模块那条（含文件名、修复指引与备份路径），对用户最有用。
    #[error("{0}")]
    Unparsable(String),
    /// OS 登录项注册/注销失败（配置已回滚或被尽力回滚）。
    #[error("{0}")]
    ApplyFailed(String),
}

impl From<crate::settings::SettingsError> for AutostartError {
    fn from(err: crate::settings::SettingsError) -> Self {
        use crate::settings::SettingsError;
        match err {
            SettingsError::SerdeError(inner) => Self::SerdeError(inner),
            SettingsError::IoError(inner) => Self::IoError(inner),
            SettingsError::Unparsable { .. } => Self::Unparsable(err.to_string()),
        }
    }
}

/// 自启动偏好配置：是否在登录时自动启动 QuickQuick
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutostartConfig {
    /// 是否启用开机自启，默认开（与设计文档§二"自启动：默认开"严格对齐）
    pub enabled: bool,
}

impl Default for AutostartConfig {
    fn default() -> Self {
        // 默认开——设计文档§二明确要求 onboarding 时默认开
        Self { enabled: true }
    }
}

impl AutostartConfig {
    /// 将当前偏好序列化为 JSON 并写入指定路径。
    ///
    /// # Errors
    /// - `AutostartError::SerdeError`：序列化失败
    /// - `AutostartError::IoError`：写文件失败
    pub fn save(&self, path: &Path) -> Result<(), AutostartError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// 逐键写入 `enabled`（对齐 settings.json 的策略，见 [`crate::settings::update_json_object`]）。
    ///
    /// 与 [`AutostartConfig::save`] 的差别：
    /// - 只改 `enabled` 一个键，**未来版本 / 别的工具写进来的未知键原样保留**；
    /// - 文件语法坏 / 根不是对象时先备份为 `autostart.json.<ms>.bak`，然后**拒写**并返回 Err——
    ///   绝不把用户原文件覆盖成只剩一个键的新文件。
    ///
    /// # Errors
    /// - `AutostartError::Unparsable`：文件无法解析（已备份，原文件未改动）
    /// - `AutostartError::IoError` / `SerdeError`：读写或序列化失败
    pub fn save_enabled(path: &Path, enabled: bool) -> Result<(), AutostartError> {
        crate::settings::update_json_object(path, |map| {
            map.insert("enabled".to_string(), serde_json::Value::Bool(enabled));
        })?;
        Ok(())
    }

    /// 从指定路径读取 JSON 并反序列化为 `AutostartConfig`。
    ///
    /// 文件不存在时返回 `AutostartError::IoError`；调用方应在该情形下
    /// 回退到 `AutostartConfig::default()`（见 lib.rs setup 侧实现）。
    ///
    /// # Errors
    /// - `AutostartError::IoError`：读文件失败
    /// - `AutostartError::SerdeError`：反序列化失败
    pub fn load(path: &Path) -> Result<Self, AutostartError> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 从路径加载偏好，文件不存在时静默回退到 `default()`。
    ///
    /// 用于 lib.rs setup 阶段——首次启动时配置文件尚不存在，应使用默认值（默认开）
    /// 而不是报错中断启动。
    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }
}

/// 将 `enabled` 偏好应用到 OS LaunchAgent（enable/disable autolaunch）。
///
/// 封装为独立函数供 `lib.rs` setup 阶段与 `ipc/settings.rs` 的 `set_launch_on_login`
/// 命令共享，避免重复维护相同的 enable/disable 分支逻辑。
///
/// 平台登录项操作（可注入：生产用 tauri 插件实现，测试用假实现）。
///
/// 抽成 trait 是为了让「条目已不存在」「删除失败」这类平台差异可测——
/// Windows 上删除一个不存在的注册表项会报错，Mac 上则是幂等的。
pub trait AutostartLauncher {
    /// 当前系统里是否已注册登录项。
    fn is_enabled(&self) -> Result<bool, String>;
    /// 注册登录项。
    fn enable(&self) -> Result<(), String>;
    /// 注销登录项。
    fn disable(&self) -> Result<(), String>;
}

/// [`AutostartLauncher`] 的真实实现：`tauri-plugin-autostart`。
struct TauriLauncher<'a> {
    handle: &'a tauri::AppHandle,
}

impl AutostartLauncher for TauriLauncher<'_> {
    fn is_enabled(&self) -> Result<bool, String> {
        use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
        self.handle
            .autolaunch()
            .is_enabled()
            .map_err(|e| e.to_string())
    }

    fn enable(&self) -> Result<(), String> {
        use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
        self.handle.autolaunch().enable().map_err(|e| e.to_string())
    }

    fn disable(&self) -> Result<(), String> {
        use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
        self.handle
            .autolaunch()
            .disable()
            .map_err(|e| e.to_string())
    }
}

/// 按目标状态**幂等**应用登录项设置。
///
/// 关闭时先查当前状态：条目已不存在说明目标状态已达成，直接算成功——
/// 否则 Windows 上会出现「条目本来就没有，删除报错 → 永久失败」的死路
/// （依据：`auto-launch` 0.5.0 删除不存在的注册表项会返回错误，属源码阅读结论，未在 Windows 实测）。
/// 只有「条目仍在、删除真的失败」才返回 Err；读不到状态时保守地仍尝试 delete，
/// 并把读取失败一并写进错误，便于定位。
///
/// # Errors
/// 注册失败、或条目仍在却注销失败时返回错误字符串。
pub fn apply_with(launcher: &dyn AutostartLauncher, enabled: bool) -> Result<(), String> {
    if enabled {
        return launcher
            .enable()
            .map_err(|e| format!("启用自启动失败：{e}"));
    }
    match launcher.is_enabled() {
        Ok(false) => Ok(()),
        Ok(true) => launcher
            .disable()
            .map_err(|e| format!("关闭自启动失败：{e}")),
        Err(read_err) => launcher
            .disable()
            .map_err(|e| format!("关闭自启动失败：{e}（读取当前状态失败：{read_err}）")),
    }
}

/// 把登录项设置应用到当前系统。
///
/// 失败时返回错误字符串（由调用方决定怎么处理）：
/// - 启动期（`lib.rs` setup）只 eprintln，注册失败不该中断启动；
/// - 用户主动切换（IPC 命令）必须回滚配置并返回 Err——否则界面显示「已开启」而系统没注册。
///
/// # Errors
/// 同 [`apply_with`]。
pub fn apply_to_os(handle: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    apply_with(&TauriLauncher { handle }, enabled)
}

/// 用户主动切换开机自启：写配置 → 应用 OS 登录项；**应用失败则回滚配置并返回 Err**。
///
/// `apply` 由调用方注入（生产传 [`apply_to_os`] 的闭包，测试传假实现），
/// 这样「OS 注册失败」这条分支不依赖真机 LaunchAgent 也能测。
///
/// 回滚语义：写入前文件存在就写回原字节对应的值；写入前文件不存在就把它删掉，
/// 避免为了回滚凭空生出一份配置。
///
/// # Errors
/// - 写配置失败：直接返回该错误（未改动 OS 状态）
/// - OS 应用失败：回滚配置后返回「系统登录项设置失败：…，已恢复为原设置」
pub fn set_enabled_with_apply(
    path: &Path,
    value: bool,
    apply: impl FnOnce(bool) -> Result<(), String>,
) -> Result<(), AutostartError> {
    let existed_before = path.exists();
    let previous = AutostartConfig::load_or_default(path).enabled;

    AutostartConfig::save_enabled(path, value)?;

    if let Err(err) = apply(value) {
        let rollback = if existed_before {
            AutostartConfig::save_enabled(path, previous).map_err(|e| e.to_string())
        } else {
            std::fs::remove_file(path).map_err(|e| e.to_string())
        };
        return match rollback {
            Ok(()) => Err(AutostartError::ApplyFailed(format!(
                "系统登录项设置失败：{err}，已恢复为原设置"
            ))),
            Err(rollback_err) => Err(AutostartError::ApplyFailed(format!(
                "系统登录项设置失败：{err}；恢复原设置也失败，配置文件当前为新值，将于下次启动生效，请检查文件权限后重试（{rollback_err}）"
            ))),
        };
    }
    Ok(())
}
