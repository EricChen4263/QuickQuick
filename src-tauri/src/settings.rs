//! 应用全局设置持久化模块
//!
//! 设计要点：
//! - 镜像 autostart.rs 的文件持久化模式：serde_json、thiserror 枚举错误、不 unwrap/panic。
//! - `AppSettings` 管理排除名单与翻译 provider 选择，不持有也不调用任何 Tauri 句柄。
//! - 路径解析（app_config_dir）属于命令层胶水，不在本模块处理。
//! - 所有新字段均加 `#[serde(default)]`，保证旧 settings.json 缺字段时反序列化不失败，
//!   向前兼容：既有 `excluded_apps` / `selected_provider` 绝不丢失。

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

fn default_selected_provider() -> String {
    // 默认免 key 源（设计文档§三.决策1：移除 MyMemory 后默认切 Lingva）。
    "google_free".to_string()
}

fn default_skip_sensitive() -> bool {
    true
}

fn default_stay_in_tray() -> bool {
    true
}

fn default_auto_update() -> bool {
    true
}

fn default_theme() -> String {
    "auto".to_string()
}

fn default_max_image_bytes() -> u64 {
    20 * 1024 * 1024
}

/// 应用全局设置：捕获行为、隐私开关、UI 偏好与翻译配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// 剪贴板捕获排除的应用标识列表（如 macOS bundle ID）
    #[serde(default)]
    pub excluded_apps: Vec<String>,
    /// 当前选中的翻译 provider id（与 providers::registry() 的 id 对应）
    #[serde(default = "default_selected_provider")]
    pub selected_provider: String,
    /// 是否暂停剪贴板捕获（默认 false）
    #[serde(default)]
    pub pause_capture: bool,
    /// 是否跳过平台 concealed/transient 敏感标记内容（默认 true）
    #[serde(default = "default_skip_sensitive")]
    pub skip_sensitive: bool,
    /// 失焦时是否隐藏到托盘而非退出（默认 true）
    #[serde(default = "default_stay_in_tray")]
    pub stay_in_tray: bool,
    /// 是否启用自动更新检查（默认 true）
    #[serde(default = "default_auto_update")]
    pub auto_update: bool,
    /// UI 主题：auto / light / dark（默认 "auto"）
    #[serde(default = "default_theme")]
    pub theme: String,
    /// 单张图片原图大小上限（字节）；超出则只存缩略图（original_present=0）。默认 20MiB。
    #[serde(default = "default_max_image_bytes")]
    pub max_image_bytes: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            excluded_apps: Vec::new(),
            selected_provider: default_selected_provider(),
            pause_capture: false,
            skip_sensitive: default_skip_sensitive(),
            stay_in_tray: default_stay_in_tray(),
            auto_update: default_auto_update(),
            theme: default_theme(),
            max_image_bytes: default_max_image_bytes(),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 旧 JSON（只含 excluded_apps + selected_provider）能正确反序列化，
    /// 新字段取各自默认值，不能因缺字段而 panic/Err。
    #[test]
    fn legacy_json_missing_new_fields_uses_defaults() {
        let json = r#"{"excluded_apps":["com.foo.app"],"selected_provider":"deepl"}"#;
        let settings: AppSettings = serde_json::from_str(json).expect("旧 JSON 反序列化应成功");

        assert_eq!(settings.excluded_apps, vec!["com.foo.app"]);
        assert_eq!(settings.selected_provider, "deepl");
        assert!(!settings.pause_capture, "pause_capture 默认应为 false");
        assert!(settings.skip_sensitive, "skip_sensitive 默认应为 true");
        assert!(settings.stay_in_tray, "stay_in_tray 默认应为 true");
        assert!(settings.auto_update, "auto_update 默认应为 true");
        assert_eq!(settings.theme, "auto", "theme 默认应为 auto");
    }

    /// 完全空的 JSON 对象也能反序列化，一切字段取默认值。
    #[test]
    fn empty_json_object_uses_all_defaults() {
        let settings: AppSettings = serde_json::from_str("{}").expect("空对象反序列化应成功");

        assert!(settings.excluded_apps.is_empty());
        assert_eq!(settings.selected_provider, "google_free");
        assert!(!settings.pause_capture);
        assert!(settings.skip_sensitive);
        assert!(settings.stay_in_tray);
        assert!(settings.auto_update);
        assert_eq!(settings.theme, "auto");
    }

    /// Default impl 与 serde default 保持一致。
    #[test]
    fn default_impl_matches_serde_defaults() {
        let d = AppSettings::default();
        assert_eq!(d.selected_provider, "google_free");
        assert!(!d.pause_capture);
        assert!(d.skip_sensitive);
        assert!(d.stay_in_tray);
        assert!(d.auto_update);
        assert_eq!(d.theme, "auto");
    }

    /// 序列化后再反序列化，值不变（round-trip）。
    #[test]
    fn round_trip_preserves_all_fields() {
        let original = AppSettings {
            excluded_apps: vec!["com.bar.app".to_string()],
            selected_provider: "openai".to_string(),
            pause_capture: true,
            skip_sensitive: false,
            stay_in_tray: false,
            auto_update: false,
            theme: "dark".to_string(),
            max_image_bytes: 5 * 1024 * 1024,
        };
        let json = serde_json::to_string(&original).expect("序列化应成功");
        let restored: AppSettings = serde_json::from_str(&json).expect("反序列化应成功");

        assert_eq!(restored.excluded_apps, original.excluded_apps);
        assert_eq!(restored.selected_provider, original.selected_provider);
        assert_eq!(restored.pause_capture, original.pause_capture);
        assert_eq!(restored.skip_sensitive, original.skip_sensitive);
        assert_eq!(restored.stay_in_tray, original.stay_in_tray);
        assert_eq!(restored.auto_update, original.auto_update);
        assert_eq!(restored.theme, original.theme);
        assert_eq!(restored.max_image_bytes, 5 * 1024 * 1024);
    }

    /// 旧 JSON（无 max_image_bytes 字段）反序列化后该字段取默认值 20MiB。
    #[test]
    fn legacy_json_missing_max_image_bytes_uses_default() {
        let json = r#"{"excluded_apps":[],"selected_provider":"mymemory"}"#;
        let settings: AppSettings = serde_json::from_str(json).expect("旧 JSON 反序列化应成功");
        assert_eq!(
            settings.max_image_bytes,
            20 * 1024 * 1024,
            "max_image_bytes 默认应为 20MiB"
        );
    }

    /// Default impl 的 max_image_bytes 与 serde default fn 一致（20MiB）。
    #[test]
    fn default_max_image_bytes_is_20mib() {
        let d = AppSettings::default();
        assert_eq!(d.max_image_bytes, 20 * 1024 * 1024);
    }
}
