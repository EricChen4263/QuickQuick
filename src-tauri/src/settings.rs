//! 应用全局设置持久化模块
//!
//! 设计要点：
//! - 镜像 autostart.rs 的文件持久化模式：serde_json、thiserror 枚举错误、不 unwrap/panic。
//! - `AppSettings` 管理排除名单与翻译 provider 选择，不持有也不调用任何 Tauri 句柄。
//! - 路径解析（app_config_dir）属于命令层胶水，不在本模块处理。

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// 应用设置相关错误
#[derive(Debug, Error)]
pub enum SettingsError {
    /// JSON 序列化/反序列化失败
    #[error("应用设置序列化失败：{0}")]
    SerdeError(#[from] serde_json::Error),
    /// 文件 I/O 失败
    #[error("应用设置文件读写失败：{0}")]
    IoError(#[from] std::io::Error),
}

/// 应用全局设置：排除名单与翻译 provider 选择
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// 剪贴板捕获排除的应用标识列表（如 macOS bundle ID）
    pub excluded_apps: Vec<String>,
    /// 当前选中的翻译 provider id（与 providers::registry() 的 id 对应）
    pub selected_provider: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            excluded_apps: Vec::new(),
            // 默认使用 mymemory（免费、无需 Key，设计文档§三默认源）
            selected_provider: "mymemory".to_string(),
        }
    }
}

impl AppSettings {
    /// 将当前设置序列化为 JSON 并写入指定路径。
    ///
    /// # Errors
    /// - `SettingsError::SerdeError`：序列化失败
    /// - `SettingsError::IoError`：写文件失败
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// 从指定路径读取 JSON 并反序列化为 `AppSettings`。
    ///
    /// 文件不存在时返回 `SettingsError::IoError`；调用方应在该情形下
    /// 回退到 `AppSettings::default()`（见 load_or_default）。
    ///
    /// # Errors
    /// - `SettingsError::IoError`：读文件失败
    /// - `SettingsError::SerdeError`：反序列化失败
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        let content = std::fs::read_to_string(path)?;
        let settings = serde_json::from_str(&content)?;
        Ok(settings)
    }

    /// 从路径加载设置，文件不存在时静默回退到 `default()`。
    ///
    /// 用于命令层——首次启动时配置文件尚不存在，应使用默认值
    /// 而不是报错。
    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }
}
