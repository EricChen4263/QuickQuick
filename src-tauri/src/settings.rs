//! 应用全局设置持久化模块
//!
//! 设计要点：
//! - 镜像 autostart.rs 的文件持久化模式：serde_json、thiserror 枚举错误、不 unwrap/panic。
//! - `AppSettings` 管理排除名单、忽略规则与翻译 provider 选择，不持有也不调用任何 Tauri 句柄。
//! - 路径解析（app_config_dir）属于命令层胶水，不在本模块处理。
//! - 所有新字段均加 `#[serde(default)]`，保证旧 settings.json 缺字段时反序列化不失败，
//!   向前兼容：既有 `excluded_apps` / `selected_provider` 绝不丢失。

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

use crate::privacy::ExcludeMode;

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

/// 列表排序方式（只影响展示顺序，不改变去重与淘汰语义）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// 最近复制优先（默认）：按 `last_modified_utc` 降序，最近一次复制在最前。
    ///
    /// 这正是列表查询本来的顺序，所以默认值不改动任何既有行为。
    #[default]
    Recent,
    /// 首次复制优先：按 `created_utc` 降序，同一内容再次复制不会让它跳到最前。
    FirstCopied,
}

/// 搜索命中高亮样式。
///
/// 三档都是纯视觉：命中区间不变、<mark> 结构不变，只换 CSS 呈现。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightStyle {
    /// 默认：背景染色 + 加粗（与既有视觉一致）
    #[default]
    Default,
    /// 下划线：去掉背景，只在命中片段下划一条 accent 线
    Underline,
    /// 纯加粗：去掉背景，只加字重
    Bold,
}

/// 弹出面板行内图片缩略图高度的合法区间下限（px）。
///
/// 比 16px 更矮就只剩一条缝，既看不出图也点不准行。
pub const MIN_IMAGE_HEIGHT_PX: u32 = 16;

/// 弹出面板行内图片缩略图高度的合法区间上限（px）。
///
/// 上限压在 48px：一行缩略图再高就会把列表挤成一屏只放得下几条，
/// 弹窗本体高度固定，超出部分只能靠滚动补。
pub const MAX_IMAGE_HEIGHT_PX: u32 = 48;

/// 弹出面板行内图片缩略图高度的默认值（px）。
///
/// 与改动前 `.popover-row-thumb` 的硬编码高度一致，升级后默认视觉不变。
pub const DEFAULT_IMAGE_HEIGHT_PX: u32 = 24;

/// 首次预览延迟的合法区间下限（毫秒）。
///
/// 与「后续切换 200ms」同一档：首个预览比后续切换还快就没有「先看列表」的意义。
pub const MIN_PREVIEW_FIRST_DELAY_MS: u32 = 200;

/// 首次预览延迟的合法区间上限（毫秒）。
///
/// 5 秒是「等太久就等于没预览」的上界；再长用户会以为预览坏了。
pub const MAX_PREVIEW_FIRST_DELAY_MS: u32 = 5000;

/// 首次预览延迟的默认值（毫秒）。
///
/// 与改动前 `PREVIEW_FIRST_DELAY_MS` 的硬编码一致，升级后默认手感不变。
pub const DEFAULT_PREVIEW_FIRST_DELAY_MS: u32 = 1500;

fn default_image_height_px() -> u32 {
    DEFAULT_IMAGE_HEIGHT_PX
}

fn default_preview_first_delay_ms() -> u32 {
    DEFAULT_PREVIEW_FIRST_DELAY_MS
}

/// 历史条目上限的合法区间下限。
///
/// 1 是「只留最新一条」的极限，不允许 0：那等于每复制一次就把历史清空，
/// 与「保留历史」的产品前提相反。
pub const MIN_HISTORY_ITEMS: usize = 1;

/// 历史条目上限的合法区间上限（对齐 Maccy 的 1–999 语义）。
pub const MAX_HISTORY_ITEMS: usize = 999;

/// 历史条目上限的默认值。
///
/// 沿用「手动清理」此前硬编码的 500 条，使存量用户升级后行为不变；
/// 与 Maccy 的默认 200 不同（见 docs/design/v04-work-items.md P4-4）。
pub const DEFAULT_MAX_HISTORY_ITEMS: usize = 500;

fn default_max_history_items() -> usize {
    DEFAULT_MAX_HISTORY_ITEMS
}

/// 应用全局设置：捕获行为、隐私开关、UI 偏好与翻译配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// 剪贴板捕获排除的应用标识列表（如 macOS bundle ID）
    ///
    /// 黑白名单共用这一份条目：`exclude_mode` 决定方向，切换模式时不动已填的条目。
    #[serde(default)]
    pub excluded_apps: Vec<String>,
    /// 应用名单模式：blacklist（名单内不记录，默认）/ whitelist（只记录名单内）
    ///
    /// 旧 settings.json 没有该字段时按黑名单处理，与历史行为一致。
    #[serde(default)]
    pub exclude_mode: ExcludeMode,
    /// 内容忽略正则（多条，任一命中即不入库）
    ///
    /// 只匹配文本内容；图片条目没有文本可比，不参与正则匹配。
    /// 非法正则在编译时丢弃该条并留痕，其余规则照常生效（不会因写错一条把剪贴板全吞）。
    #[serde(default)]
    pub ignore_regexes: Vec<String>,
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
    /// 历史条目上限（1..=999，默认 500）：超过后自动淘汰最旧的非收藏条目
    ///
    /// 约束的**只是非收藏条目**——收藏项既不计数也不被淘汰（对齐 Maccy 的 pin 无限豁免）。
    /// 消费侧请读 [`AppSettings::effective_max_history_items`] 而不是本字段：settings.json 可能
    /// 被手工改坏（0 / 10000），直接用会让历史被清空或淘汰彻底失效。
    #[serde(default = "default_max_history_items")]
    pub max_history_items: usize,
    /// 列表排序方式（默认 `Recent`：最近复制优先）
    ///
    /// 只改变列表展示顺序，不参与去重、收藏与淘汰判定。
    #[serde(default)]
    pub sort_order: SortOrder,
    /// 弹出面板行内图片缩略图高度（px，16..=48，默认 24）
    ///
    /// 消费侧请读 [`AppSettings::effective_image_height_px`]：settings.json 是明文文件，
    /// 手工写出的 1 或 999 会让缩略图小到看不见 / 大到挤爆一行。
    #[serde(default = "default_image_height_px")]
    pub image_height_px: u32,
    /// 弹出面板首个行内预览的等待时长（毫秒，200..=5000，默认 1500）
    ///
    /// 只作用于「面板本次打开后的首个预览」；后续切换固定 200ms 不开放配置，
    /// 因为那一档的取值只影响翻页时中间项闪不闪，没有第二种合理值。
    #[serde(default = "default_preview_first_delay_ms")]
    pub preview_first_delay_ms: u32,
    /// 搜索命中高亮样式（默认 `Default`：背景 + 加粗）
    #[serde(default)]
    pub highlight_style: HighlightStyle,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            excluded_apps: Vec::new(),
            exclude_mode: ExcludeMode::default(),
            ignore_regexes: Vec::new(),
            selected_provider: default_selected_provider(),
            pause_capture: false,
            skip_sensitive: default_skip_sensitive(),
            stay_in_tray: default_stay_in_tray(),
            auto_update: default_auto_update(),
            theme: default_theme(),
            max_image_bytes: default_max_image_bytes(),
            max_history_items: default_max_history_items(),
            sort_order: SortOrder::default(),
            image_height_px: default_image_height_px(),
            preview_first_delay_ms: default_preview_first_delay_ms(),
            highlight_style: HighlightStyle::default(),
        }
    }
}

impl AppSettings {
    /// 历史条目上限（已收敛到 [`MIN_HISTORY_ITEMS`]..=[`MAX_HISTORY_ITEMS`]）。
    ///
    /// 写路径（`set_max_history_items`）已经拒收越界值，这里再收敛一次是给**读路径**兜底：
    /// settings.json 是明文文件，用户或旧版本脚本可以写出 0 或 10000。收敛而不报错，是因为
    /// 报错会让整份设置（含排除名单、翻译源）退化成默认值，代价远大于把一个数字夹回边界。
    ///
    /// 非法输入的具体处理：
    /// - `0` → `MIN_HISTORY_ITEMS`（保留最新一条，而不是清空历史）
    /// - 大于上限 → `MAX_HISTORY_ITEMS`（等于回到「界面上能选到的最大上限」）
    pub fn effective_max_history_items(&self) -> usize {
        self.max_history_items
            .clamp(MIN_HISTORY_ITEMS, MAX_HISTORY_ITEMS)
    }

    /// 生效的缩略图高度（已收敛到 [`MIN_IMAGE_HEIGHT_PX`]..=[`MAX_IMAGE_HEIGHT_PX`]）。
    ///
    /// 与 [`AppSettings::effective_max_history_items`] 同理：写路径已经拒收越界值，
    /// 这里兜的是**读路径**——手工改坏的 1 / 999 不该让弹窗缩略图小到看不见或撑爆行。
    pub fn effective_image_height_px(&self) -> u32 {
        self.image_height_px
            .clamp(MIN_IMAGE_HEIGHT_PX, MAX_IMAGE_HEIGHT_PX)
    }

    /// 生效的首次预览延迟（已收敛到 [`MIN_PREVIEW_FIRST_DELAY_MS`]..=[`MAX_PREVIEW_FIRST_DELAY_MS`]）。
    ///
    /// 读路径兜底理由同 [`AppSettings::effective_image_height_px`]：0 会让首个预览变成
    /// 「面板一打开就闪一下预览」，远超上限则等于关掉了预览。
    pub fn effective_preview_first_delay_ms(&self) -> u32 {
        self.preview_first_delay_ms
            .clamp(MIN_PREVIEW_FIRST_DELAY_MS, MAX_PREVIEW_FIRST_DELAY_MS)
    }

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
        assert_eq!(d.sort_order, SortOrder::Recent);
        assert_eq!(d.image_height_px, DEFAULT_IMAGE_HEIGHT_PX);
        assert_eq!(d.preview_first_delay_ms, DEFAULT_PREVIEW_FIRST_DELAY_MS);
        assert_eq!(d.highlight_style, HighlightStyle::Default);
    }

    /// 序列化后再反序列化，值不变（round-trip）。
    #[test]
    fn round_trip_preserves_all_fields() {
        let original = AppSettings {
            excluded_apps: vec!["com.bar.app".to_string()],
            exclude_mode: ExcludeMode::Whitelist,
            ignore_regexes: vec![r"^\d{16}$".to_string(), "secret".to_string()],
            selected_provider: "openai".to_string(),
            pause_capture: true,
            skip_sensitive: false,
            stay_in_tray: false,
            auto_update: false,
            theme: "dark".to_string(),
            max_image_bytes: 5 * 1024 * 1024,
            max_history_items: 42,
            sort_order: SortOrder::FirstCopied,
            image_height_px: 40,
            preview_first_delay_ms: 800,
            highlight_style: HighlightStyle::Underline,
        };
        let json = serde_json::to_string(&original).expect("序列化应成功");
        let restored: AppSettings = serde_json::from_str(&json).expect("反序列化应成功");

        assert_eq!(restored.excluded_apps, original.excluded_apps);
        assert_eq!(restored.exclude_mode, ExcludeMode::Whitelist);
        assert_eq!(restored.ignore_regexes, original.ignore_regexes);
        assert_eq!(restored.selected_provider, original.selected_provider);
        assert_eq!(restored.pause_capture, original.pause_capture);
        assert_eq!(restored.skip_sensitive, original.skip_sensitive);
        assert_eq!(restored.stay_in_tray, original.stay_in_tray);
        assert_eq!(restored.auto_update, original.auto_update);
        assert_eq!(restored.theme, original.theme);
        assert_eq!(restored.max_image_bytes, 5 * 1024 * 1024);
        assert_eq!(restored.max_history_items, 42, "历史条目上限应原样保留");
        assert_eq!(restored.sort_order, SortOrder::FirstCopied);
        assert_eq!(restored.image_height_px, 40);
        assert_eq!(restored.preview_first_delay_ms, 800);
        assert_eq!(restored.highlight_style, HighlightStyle::Underline);
    }

    /// 旧 JSON（没有 exclude_mode / ignore_regexes）能反序列化：模式回落到黑名单、
    /// 内容规则为空，而既有排除名单原样保留。
    #[test]
    fn legacy_json_without_rule_fields_keeps_old_behavior() {
        let json = r#"{"excluded_apps":["1Password"],"pause_capture":false}"#;
        let settings: AppSettings = serde_json::from_str(json).expect("旧 JSON 反序列化应成功");

        assert_eq!(settings.excluded_apps, vec!["1Password".to_string()]);
        assert_eq!(
            settings.exclude_mode,
            ExcludeMode::Blacklist,
            "缺字段时必须回落到黑名单（历史行为）"
        );
        assert!(
            settings.ignore_regexes.is_empty(),
            "缺字段时内容规则应为空（不启用）"
        );
    }

    /// 模式字段在 JSON 里是小写字符串，与前端传输的形状一致。
    #[test]
    fn exclude_mode_serializes_as_lowercase_string() {
        let settings = AppSettings {
            exclude_mode: ExcludeMode::Whitelist,
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&settings).expect("序列化应成功");

        assert!(
            json.contains(r#""exclude_mode":"whitelist""#),
            "模式必须序列化为小写字符串，实际：{json}"
        );

        let restored: AppSettings =
            serde_json::from_str(r#"{"exclude_mode":"blacklist"}"#).expect("反序列化应成功");
        assert_eq!(restored.exclude_mode, ExcludeMode::Blacklist);
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

    /// 旧 JSON（无 max_history_items 字段）反序列化后取默认 500 条：
    /// 与「手动清理」此前硬编码的 500 条一致，存量用户升级后行为不变。
    #[test]
    fn legacy_json_missing_max_history_items_uses_default_500() {
        let json = r#"{"excluded_apps":["1Password"]}"#;
        let settings: AppSettings = serde_json::from_str(json).expect("旧 JSON 反序列化应成功");

        assert_eq!(
            settings.max_history_items, 500,
            "缺字段时应沿用既有硬编码的 500 条，不能悄悄改成别的值"
        );
        assert_eq!(
            settings.effective_max_history_items(),
            500,
            "生效值应与持久化值一致"
        );
    }

    /// Default impl 与 serde default fn 的 max_history_items 必须同一来源。
    #[test]
    fn default_max_history_items_is_500() {
        let d = AppSettings::default();
        assert_eq!(d.max_history_items, DEFAULT_MAX_HISTORY_ITEMS);
        assert_eq!(d.max_history_items, 500);
    }

    /// 读路径兜底：手写 0 会被收敛到下限 1（保留最新一条），不会变成「每次复制清空历史」。
    #[test]
    fn effective_max_history_items_clamps_zero_to_min() {
        let settings = AppSettings {
            max_history_items: 0,
            ..AppSettings::default()
        };

        assert_eq!(settings.effective_max_history_items(), MIN_HISTORY_ITEMS);
    }

    /// 读路径兜底：手写超大值会被收敛到上限 999，而不是「等于关闭淘汰」。
    #[test]
    fn effective_max_history_items_clamps_oversize_to_max() {
        let settings = AppSettings {
            max_history_items: 10_000,
            ..AppSettings::default()
        };

        assert_eq!(settings.effective_max_history_items(), MAX_HISTORY_ITEMS);
    }

    /// 合法区间两端必须原样透出（clamp 不能把边界值也改掉）。
    #[test]
    fn effective_max_history_items_keeps_boundary_values_intact() {
        for value in [MIN_HISTORY_ITEMS, 500, MAX_HISTORY_ITEMS] {
            let settings = AppSettings {
                max_history_items: value,
                ..AppSettings::default()
            };
            assert_eq!(
                settings.effective_max_history_items(),
                value,
                "区间内的值 {value} 不应被收敛改动"
            );
        }
    }

    /// 旧 JSON（无四个外观字段）反序列化后取默认值：排序=最近复制、缩略图 24px、
    /// 首次预览 1500ms、高亮=默认。这四个值必须与改动前代码里的硬编码一致。
    #[test]
    fn legacy_json_missing_appearance_fields_uses_pre_change_defaults() {
        let json = r#"{"excluded_apps":["1Password"],"max_history_items":300}"#;
        let settings: AppSettings = serde_json::from_str(json).expect("旧 JSON 反序列化应成功");

        assert_eq!(
            settings.sort_order,
            SortOrder::Recent,
            "默认排序必须是最近复制"
        );
        assert_eq!(
            settings.image_height_px, 24,
            "默认缩略图高度必须等于改动前的 24px"
        );
        assert_eq!(
            settings.preview_first_delay_ms, 1500,
            "默认首次预览延迟必须等于改动前的 1500ms"
        );
        assert_eq!(
            settings.highlight_style,
            HighlightStyle::Default,
            "默认高亮样式必须等于改动前的背景+加粗"
        );
    }

    /// 四个外观字段在 JSON 里都是小写/下划线字符串，与前端传输的形状一致。
    #[test]
    fn appearance_enums_serialize_as_snake_case_strings() {
        let settings = AppSettings {
            sort_order: SortOrder::FirstCopied,
            highlight_style: HighlightStyle::Bold,
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&settings).expect("序列化应成功");

        assert!(
            json.contains(r#""sort_order":"first_copied""#),
            "排序方式必须序列化为 first_copied，实际：{json}"
        );
        assert!(
            json.contains(r#""highlight_style":"bold""#),
            "高亮样式必须序列化为 bold，实际：{json}"
        );
    }

    /// 读路径兜底：手写 1px 会被收敛到下限 16px（否则缩略图小到看不见）。
    #[test]
    fn effective_image_height_px_clamps_below_min() {
        let settings = AppSettings {
            image_height_px: 1,
            ..AppSettings::default()
        };

        assert_eq!(settings.effective_image_height_px(), MIN_IMAGE_HEIGHT_PX);
        assert_eq!(settings.effective_image_height_px(), 16);
    }

    /// 读路径兜底：手写 999px 会被收敛到上限 48px（否则一行挤爆弹窗）。
    #[test]
    fn effective_image_height_px_clamps_above_max() {
        let settings = AppSettings {
            image_height_px: 999,
            ..AppSettings::default()
        };

        assert_eq!(settings.effective_image_height_px(), MAX_IMAGE_HEIGHT_PX);
        assert_eq!(settings.effective_image_height_px(), 48);
    }

    /// 缩略图高度区间内的值原样透出（含两端）。
    #[test]
    fn effective_image_height_px_keeps_boundary_values_intact() {
        for value in [MIN_IMAGE_HEIGHT_PX, 24, MAX_IMAGE_HEIGHT_PX] {
            let settings = AppSettings {
                image_height_px: value,
                ..AppSettings::default()
            };
            assert_eq!(settings.effective_image_height_px(), value);
        }
    }

    /// 读路径兜底：手写 0ms 会被收敛到下限 200ms（否则首个预览瞬间闪现）。
    #[test]
    fn effective_preview_first_delay_ms_clamps_below_min() {
        let settings = AppSettings {
            preview_first_delay_ms: 0,
            ..AppSettings::default()
        };

        assert_eq!(
            settings.effective_preview_first_delay_ms(),
            MIN_PREVIEW_FIRST_DELAY_MS
        );
        assert_eq!(settings.effective_preview_first_delay_ms(), 200);
    }

    /// 读路径兜底：手写 60_000ms 会被收敛到上限 5000ms（否则等于关掉预览）。
    #[test]
    fn effective_preview_first_delay_ms_clamps_above_max() {
        let settings = AppSettings {
            preview_first_delay_ms: 60_000,
            ..AppSettings::default()
        };

        assert_eq!(
            settings.effective_preview_first_delay_ms(),
            MAX_PREVIEW_FIRST_DELAY_MS
        );
        assert_eq!(settings.effective_preview_first_delay_ms(), 5000);
    }

    /// 首次预览延迟区间内的值原样透出（含两端）。
    #[test]
    fn effective_preview_first_delay_ms_keeps_boundary_values_intact() {
        for value in [MIN_PREVIEW_FIRST_DELAY_MS, 1500, MAX_PREVIEW_FIRST_DELAY_MS] {
            let settings = AppSettings {
                preview_first_delay_ms: value,
                ..AppSettings::default()
            };
            assert_eq!(settings.effective_preview_first_delay_ms(), value);
        }
    }

    /// 枚举字段写坏时的爆炸半径：serde 反序列化整体失败 → `load_or_default` 回退全默认。
    ///
    /// 这是与 `exclude_mode` 一致的历史行为，不是本项引入的新风险；测试把它显式钉住，
    /// 免得日后有人以为「只坏一个字段只丢一个字段」。
    #[test]
    fn invalid_sort_order_string_falls_back_to_all_defaults() {
        let dir = std::env::temp_dir().join(format!("qq-settings-enum-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(
            &path,
            r#"{"sort_order":"by_magic","excluded_apps":["com.foo.app"]}"#,
        )
        .expect("写临时设置文件失败");

        let settings = AppSettings::load_or_default(&path);

        assert_eq!(
            settings.sort_order,
            SortOrder::Recent,
            "未知排序方式回退默认值"
        );
        assert!(
            settings.excluded_apps.is_empty(),
            "整份设置都回退默认——这正是必须写进文档的爆炸半径"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
