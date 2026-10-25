//! 热键配置模块：默认值、改键持久化
//!
//! 设计要点：
//! - 本模块只负责配置读写，**不承担系统注册职责**：运行时注册/注销由命令层
//!   `ipc::settings::set_hotkey_runtime_impl` 编排（注册新键 → 落盘 → 注销旧键）。
//! - 使用 thiserror 枚举化错误，Display 文本中文，符合项目规范。

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// 支持的热键动作
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotkeyAction {
    /// 呼出剪贴板历史面板
    History,
    /// 呼出翻译面板
    Translate,
    /// 打开并聚焦应用主界面
    Main,
}

/// 热键相关错误
#[derive(Debug, Error)]
pub enum HotkeyError {
    /// 热键已被系统或其他应用占用，拒绝绑定
    #[error("热键已被占用，无法绑定")]
    AlreadyInUse,
    /// JSON 序列化/反序列化失败
    #[error("热键配置序列化失败：{0}")]
    SerdeError(#[from] serde_json::Error),
    /// 文件 I/O 失败
    #[error("热键配置文件读写失败：{0}")]
    IoError(#[from] std::io::Error),
    /// 配置文件存在但无法解析为 JSON 对象：**拒绝写入**（已先备份留痕）。
    #[error("{0}")]
    Unparsable(String),
}

impl From<crate::settings::SettingsError> for HotkeyError {
    fn from(err: crate::settings::SettingsError) -> Self {
        use crate::settings::SettingsError;
        match err {
            SettingsError::SerdeError(inner) => Self::SerdeError(inner),
            SettingsError::IoError(inner) => Self::IoError(inner),
            SettingsError::Unparsable { .. } => Self::Unparsable(err.to_string()),
        }
    }
}

fn default_history_accelerator() -> String {
    "CmdOrCtrl+Shift+C".to_string()
}

fn default_translate_accelerator() -> String {
    "CmdOrCtrl+Shift+T".to_string()
}

fn default_main_accelerator() -> String {
    "CmdOrCtrl+Shift+M".to_string()
}

/// 热键配置：保存各动作各自的加速键字符串
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// 历史面板热键
    #[serde(default = "default_history_accelerator")]
    history_accelerator: String,
    /// 翻译面板热键
    #[serde(default = "default_translate_accelerator")]
    translate_accelerator: String,
    /// 应用主界面热键
    #[serde(default = "default_main_accelerator")]
    main_accelerator: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            // 默认值来自设计文档§一，与验收项 V0-F2-A01 严格对齐
            history_accelerator: default_history_accelerator(),
            translate_accelerator: default_translate_accelerator(),
            main_accelerator: default_main_accelerator(),
        }
    }
}

impl HotkeyConfig {
    /// 获取指定动作当前绑定的加速键字符串
    pub fn get_accelerator(&self, action: HotkeyAction) -> &str {
        match action {
            HotkeyAction::History => &self.history_accelerator,
            HotkeyAction::Translate => &self.translate_accelerator,
            HotkeyAction::Main => &self.main_accelerator,
        }
    }

    /// 更新指定动作的加速键字符串。
    ///
    /// 此函数只改内存配置，不做系统注册；命令层的运行时改键必须先注册新键、
    /// 保存配置、再注销旧键，注册与注销都由调用方负责编排。
    pub(crate) fn set_accelerator(&mut self, action: HotkeyAction, accelerator: &str) {
        match action {
            HotkeyAction::History => {
                self.history_accelerator = accelerator.to_string();
            }
            HotkeyAction::Translate => {
                self.translate_accelerator = accelerator.to_string();
            }
            HotkeyAction::Main => {
                self.main_accelerator = accelerator.to_string();
            }
        }
    }

    /// 逐键写入某个动作的加速键（对齐 settings.json 的策略，见 [`crate::settings::update_json_object`]）。
    ///
    /// 与 [`HotkeyConfig::save`] 的差别：只改目标动作的那一个键，另两个动作与任何未知键原样保留；
    /// 文件语法坏 / 根不是对象时先备份为 `hotkey.json.<ms>.bak`，然后**拒写**并返回 Err。
    ///
    /// # Errors
    /// - `HotkeyError::Unparsable`：文件无法解析（已备份，原文件未改动）
    /// - `HotkeyError::IoError` / `SerdeError`：读写或序列化失败
    pub fn save_accelerator(
        path: &Path,
        action: HotkeyAction,
        accelerator: &str,
    ) -> Result<(), HotkeyError> {
        let key = Self::config_key(action);
        crate::settings::update_json_object(path, |map| {
            map.insert(
                key.to_string(),
                serde_json::Value::String(accelerator.to_string()),
            );
        })?;
        Ok(())
    }

    /// 动作对应的 JSON 键名（与 serde 字段名保持一致）。
    fn config_key(action: HotkeyAction) -> &'static str {
        match action {
            HotkeyAction::History => "history_accelerator",
            HotkeyAction::Translate => "translate_accelerator",
            HotkeyAction::Main => "main_accelerator",
        }
    }

    /// 将当前配置序列化为 JSON 并写入指定路径。
    ///
    /// # Errors
    /// - `HotkeyError::SerdeError`：序列化失败
    /// - `HotkeyError::IoError`：写文件失败
    pub fn save(&self, path: &Path) -> Result<(), HotkeyError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// 从指定路径读取 JSON 并反序列化为 `HotkeyConfig`。
    ///
    /// # Errors
    /// - `HotkeyError::IoError`：读文件失败
    /// - `HotkeyError::SerdeError`：反序列化失败
    pub fn load(path: &Path) -> Result<Self, HotkeyError> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 容错读取：文件不存在 / 读失败 / 内容损坏时一律回退 [`HotkeyConfig::default`]。
    ///
    /// 语义与 [`crate::autostart::AutostartConfig::load_or_default`] 对齐。
    /// 写路径（[`Self::save_accelerator`]）另有「备份 + 拒写」保护，所以这里静默回退
    /// 不会把坏文件覆盖掉——它只是让本次比较/展示有个可用的基准值。
    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }
}
