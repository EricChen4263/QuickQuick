//! 热键配置模块：默认值、改键持久化、冲突检测抽象
//!
//! 设计要点：
//! - `HotkeyRegistrar` trait 将真实系统注册与业务逻辑解耦，便于 headless 单测。
//! - `rebind` 先试注册，成功才持久化写入，失败则原样不动，保证配置一致性。
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
}

/// 热键注册器抽象——将系统 API 调用与业务逻辑隔离，使冲突路径可 headless 测试。
///
/// 真实实现在运行时通过 Tauri global shortcut API 完成；
/// 测试侧传入 fake 实现，无需启动 GUI。
pub trait HotkeyRegistrar {
    /// 尝试向系统注册加速键。
    ///
    /// # Errors
    /// - `HotkeyError::AlreadyInUse`：该键已被占用
    fn register(&self, accelerator: &str) -> Result<(), HotkeyError>;
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

    /// 在调用方已经完成运行时注册校验后，更新指定动作的加速键字符串。
    ///
    /// 此函数不做系统注册尝试；命令层运行时改键需要先注册新键、保存配置、
    /// 再注销旧键，不能复用会二次注册的 `rebind`。
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

    /// 尝试将指定动作改绑到新加速键。
    ///
    /// 先通过 `registrar.register` 试注册：
    /// - 成功 → 更新内存配置并返回 `Ok(())`。
    /// - 失败 → 原配置**不变**，将错误原样上抛。
    ///
    /// # Errors
    /// - `HotkeyError::AlreadyInUse`：新键已被占用，配置未改动
    pub fn rebind(
        &mut self,
        action: HotkeyAction,
        accelerator: &str,
        registrar: &dyn HotkeyRegistrar,
    ) -> Result<(), HotkeyError> {
        if self.get_accelerator(action) == accelerator {
            return Ok(());
        }

        // 先试注册；失败则提前返回，保证配置不变
        registrar.register(accelerator)?;

        // 注册成功后才写入内存配置
        self.set_accelerator(action, accelerator);
        Ok(())
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
}
