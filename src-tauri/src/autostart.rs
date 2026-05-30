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
