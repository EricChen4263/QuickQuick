//! 应用全局设置持久化模块
//!
//! 设计要点：
//! - 镜像 autostart.rs 的文件持久化模式：serde_json、thiserror 枚举错误、不 unwrap/panic。
//! - `AppSettings` 管理排除名单、忽略规则与翻译 provider 选择，不持有也不调用任何 Tauri 句柄。
//! - 路径解析（app_config_dir）属于命令层胶水，不在本模块处理。
//! - 所有新字段均加 `#[serde(default)]`，保证旧 settings.json 缺字段时反序列化不失败，
//!   向前兼容：既有 `excluded_apps` / `selected_provider` 绝不丢失。
//! - **读写都按「逐键」语义**（对齐 Maccy 的 UserDefaults 逐键存储，见
//!   docs/design/v04-work-items.md P5-8）：读走 [`AppSettings::load_lenient`]，
//!   坏字段只回退它自己；写走 [`write_setting_field`] / [`update_settings_value`]，
//!   只改目标键、其余键与未知键原样保留。整份覆盖的 [`AppSettings::save`] 只留给测试。
//!   文件语法损坏时**先备份 `settings.json.bak` 再写**，绝不静默覆盖用户原文件。

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
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

    /// 将当前设置序列化为 JSON 并写入指定路径（**整份覆盖**）。
    ///
    /// 只给测试 / 「整份重写」这类场景用。生产写路径必须走 [`write_setting_field`] 或
    /// [`update_settings_value`]：本方法会把内存里的 `AppSettings` 全量落盘，
    /// 未知键（未来版本或别的工具写的字段）与当前解析失败的字段都会被抹掉。
    ///
    /// # Errors
    /// - `SettingsError::SerdeError`：序列化失败
    /// - `SettingsError::IoError`：写文件失败
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// 从指定路径读取 JSON 并反序列化为 `AppSettings`（**严格**：任一字段不合法即失败）。
    ///
    /// 文件不存在时返回 `SettingsError::IoError`。生产读取请用
    /// [`AppSettings::load_or_default`]（字段级容错）而不是本方法。
    ///
    /// # Errors
    /// - `SettingsError::IoError`：读文件失败
    /// - `SettingsError::SerdeError`：反序列化失败
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        let content = std::fs::read_to_string(path)?;
        let settings = serde_json::from_str(&content)?;
        Ok(settings)
    }

    /// 从路径加载设置：**字段级容错**，任一字段坏掉只回退该字段的默认值。
    ///
    /// 与旧行为的关键差别：以前任一字段/语法错都会让**整份设置**变成默认值，
    /// 于是下一次任意 setter 把整份默认值写回，顺手覆盖掉热键、排除名单、翻译源等
    /// 与该字段毫无关系的设置（对齐 Maccy 的逐键存储语义后，一个坏键只影响它自己）。
    ///
    /// 文件不存在是正常首启动，直接返回默认值；读取其它 I/O 错误也回退默认值，
    /// 但会把原因放进返回值供调用方留痕。
    ///
    /// 返回的第二个元素是「被回退的字段 / 被忽略的原因」清单，空表示整份设置都解析成功。
    pub fn load_lenient(path: &Path) -> (Self, Vec<String>) {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), Vec::new());
            }
            Err(err) => return (Self::default(), vec![format!("读取设置文件失败：{err}")]),
        };

        match serde_json::from_str::<Value>(&content) {
            Ok(Value::Object(map)) => Self::from_json_object_lenient(&map),
            Ok(_) => (
                Self::default(),
                vec!["设置文件根不是 JSON 对象".to_string()],
            ),
            Err(err) => (
                Self::default(),
                vec![format!("设置文件 JSON 语法错误：{err}")],
            ),
        }
    }

    /// 从 JSON 对象逐字段宽松构造：坏字段回退该字段默认值，好字段照常生效。
    ///
    /// 缺失字段与坏字段都取 [`AppSettings::default`] 的对应值——这要求 `Default` 实现
    /// 与 serde 的 `default = "..."` 逐字段一致（有测试锚定）。
    pub fn from_json_object_lenient(map: &Map<String, Value>) -> (Self, Vec<String>) {
        let defaults = Self::default();
        let mut problems: Vec<String> = Vec::new();

        let settings = Self {
            excluded_apps: read_field(map, "excluded_apps", defaults.excluded_apps, &mut problems),
            exclude_mode: read_field(map, "exclude_mode", defaults.exclude_mode, &mut problems),
            ignore_regexes: read_field(
                map,
                "ignore_regexes",
                defaults.ignore_regexes,
                &mut problems,
            ),
            selected_provider: read_field(
                map,
                "selected_provider",
                defaults.selected_provider,
                &mut problems,
            ),
            pause_capture: read_field(map, "pause_capture", defaults.pause_capture, &mut problems),
            skip_sensitive: read_field(
                map,
                "skip_sensitive",
                defaults.skip_sensitive,
                &mut problems,
            ),
            stay_in_tray: read_field(map, "stay_in_tray", defaults.stay_in_tray, &mut problems),
            auto_update: read_field(map, "auto_update", defaults.auto_update, &mut problems),
            theme: read_field(map, "theme", defaults.theme, &mut problems),
            max_image_bytes: read_field(
                map,
                "max_image_bytes",
                defaults.max_image_bytes,
                &mut problems,
            ),
            max_history_items: read_field(
                map,
                "max_history_items",
                defaults.max_history_items,
                &mut problems,
            ),
            sort_order: read_field(map, "sort_order", defaults.sort_order, &mut problems),
            image_height_px: read_field(
                map,
                "image_height_px",
                defaults.image_height_px,
                &mut problems,
            ),
            preview_first_delay_ms: read_field(
                map,
                "preview_first_delay_ms",
                defaults.preview_first_delay_ms,
                &mut problems,
            ),
            highlight_style: read_field(
                map,
                "highlight_style",
                defaults.highlight_style,
                &mut problems,
            ),
        };

        (settings, problems)
    }

    /// 从路径加载设置，文件不存在时静默回退到 `default()`（字段级容错，见 [`Self::load_lenient`]）。
    ///
    /// 刻意**不打日志**：入库轮询每轮（约 500ms）都会读一次 settings.json，
    /// 一个坏字段会让日志每秒刷两行。需要留痕的路径请用 [`Self::load_lenient`]。
    pub fn load_or_default(path: &Path) -> Self {
        Self::load_lenient(path).0
    }
}

/// 同名字段宽松读取：缺失或解析失败返回 `fallback`，失败时把字段名与原因记进 `problems`。
fn read_field<T: serde::de::DeserializeOwned>(
    map: &Map<String, Value>,
    key: &str,
    fallback: T,
    problems: &mut Vec<String>,
) -> T {
    let Some(raw) = map.get(key) else {
        return fallback;
    };
    match serde_json::from_value::<T>(raw.clone()) {
        Ok(value) => value,
        Err(err) => {
            problems.push(format!("{key}（{err}）"));
            fallback
        }
    }
}

/// 与某个设置文件同目录的备份文件名：`settings.json` → `settings.json.bak`。
pub fn backup_path_for(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

/// 读 settings.json 为 JSON 对象；文件不存在返回空对象。
///
/// 文件存在但**语法损坏 / 根不是对象**时：先把原文件备份为 `settings.json.bak`，
/// 再从空对象开始。备份失败直接返回 Err——宁可不写，也不能在没有备份的情况下覆盖用户原文件。
fn read_settings_object_for_write(path: &Path) -> Result<Map<String, Value>, SettingsError> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(err) => return Err(SettingsError::IoError(err)),
    };

    match serde_json::from_str::<Value>(&content) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(other) => {
            backup_broken_settings(
                path,
                &format!("根是 {} 而不是 JSON 对象", value_kind(&other)),
            )?;
            Ok(Map::new())
        }
        Err(err) => {
            backup_broken_settings(path, &format!("JSON 语法错误（{err}）"))?;
            Ok(Map::new())
        }
    }
}

/// JSON 值的类型名（只用于错误/备份日志，不求完整）。
fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "布尔值",
        Value::Number(_) => "数字",
        Value::String(_) => "字符串",
        Value::Array(_) => "数组",
        Value::Object(_) => "对象",
    }
}

/// 备份损坏的 settings.json 并留痕；备份失败返回 Err 以阻止后续写入。
fn backup_broken_settings(path: &Path, reason: &str) -> Result<(), SettingsError> {
    let backup = backup_path_for(path);
    std::fs::copy(path, &backup)?;
    eprintln!(
        "[QuickQuick] settings.json {reason}，已备份原文件到 {}，本次只写入要修改的键",
        backup.display()
    );
    Ok(())
}

/// 以「读原始 JSON 对象 → 只改目标键 → 写回」的方式更新 settings.json。
///
/// 与 [`AppSettings::save`] 的区别就是本函数存在的理由：**未知键与其他字段原样保留**。
/// 于是「某个字段被写坏」不再演变成「整份设置被默认值覆盖」——
/// 写坏的键会留在文件里（用户可修），其余键一个字节都不动。
///
/// 写完顺手体检一次已知字段：仍有解析失败的字段就 `eprintln` 留痕。
/// 只在写入时体检而不是每次读盘，是为了不让入库轮询（约 500ms 一轮）刷日志。
///
/// # Errors
/// - `SettingsError::IoError`：读文件、备份或写文件失败（备份失败时不写）
/// - `SettingsError::SerdeError`：目标键的值无法序列化（只可能是调用方传了非法类型）
pub fn update_settings_value<F>(path: &Path, mutate: F) -> Result<(), SettingsError>
where
    F: FnOnce(&mut Map<String, Value>),
{
    let mut map = read_settings_object_for_write(path)?;
    mutate(&mut map);
    log_unparsable_fields(&map);
    let json = serde_json::to_string_pretty(&Value::Object(map))?;
    std::fs::write(path, json)?;
    Ok(())
}

/// 单字段写入：把 `value` 序列化后写进 `key`，其余键原样保留。
///
/// # Errors
/// 同 [`update_settings_value`]。
pub fn write_setting_field<T: Serialize>(
    path: &Path,
    key: &str,
    value: &T,
) -> Result<(), SettingsError> {
    let json_value = serde_json::to_value(value)?;
    update_settings_value(path, |map| {
        map.insert(key.to_string(), json_value);
    })
}

/// 体检已知字段，把解析失败的字段名与原因打到 stderr（不改变文件内容）。
fn log_unparsable_fields(map: &Map<String, Value>) {
    let (_, problems) = AppSettings::from_json_object_lenient(map);
    for problem in problems {
        eprintln!(
            "[QuickQuick] settings.json 的 {problem} 无法解析，该字段按默认值生效（其他设置不受影响）"
        );
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

    /// 枚举字段写坏时**只回退该字段**：其余字段原样生效（对齐 Maccy 的逐键存储）。
    ///
    /// 旧行为是「任一字段坏掉 → 整份设置变默认值」，于是下一次任意 setter 把整份默认值
    /// 写回文件，顺手抹掉排除名单等无关设置。这个测试就是那条边界的分界桩。
    #[test]
    fn invalid_sort_order_only_resets_that_field() {
        let dir = std::env::temp_dir().join(format!("qq-settings-enum-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(
            &path,
            r#"{"sort_order":"by_magic","excluded_apps":["com.foo.app"],"max_history_items":300}"#,
        )
        .expect("写临时设置文件失败");

        let (settings, problems) = AppSettings::load_lenient(&path);

        assert_eq!(
            settings.sort_order,
            SortOrder::Recent,
            "未知排序方式回退默认值"
        );
        assert_eq!(
            settings.excluded_apps,
            vec!["com.foo.app".to_string()],
            "坏字段不得连累排除名单"
        );
        assert_eq!(settings.max_history_items, 300, "坏字段不得连累历史上限");
        assert_eq!(problems.len(), 1, "只应报告一个坏字段，实际：{problems:?}");
        assert!(
            problems[0].contains("sort_order"),
            "留痕必须点名坏字段，实际：{problems:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 整份文件 JSON 语法错误：读路径回退默认并留痕（不 panic、不静默）。
    #[test]
    fn syntax_broken_file_falls_back_to_defaults_with_problem() {
        let dir = std::env::temp_dir().join(format!("qq-settings-syntax-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ not json at all ").expect("写临时设置文件失败");

        let (settings, problems) = AppSettings::load_lenient(&path);

        assert_eq!(settings.selected_provider, "google_free", "回退到默认设置");
        assert_eq!(problems.len(), 1, "应报告一条问题，实际：{problems:?}");
        assert!(
            problems[0].contains("语法"),
            "留痕应说明是语法错误，实际：{problems:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 文件不存在是正常首启动：默认值 + **零问题**（不应被当成损坏）。
    #[test]
    fn missing_file_is_not_a_problem() {
        let dir = std::env::temp_dir().join(format!("qq-settings-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("missing.json");

        let (settings, problems) = AppSettings::load_lenient(&path);

        assert_eq!(settings.max_history_items, DEFAULT_MAX_HISTORY_ITEMS);
        assert!(problems.is_empty(), "缺文件不算问题，实际：{problems:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// `Default` 实现必须与 serde 的逐字段默认值完全一致。
    ///
    /// 宽松解析用 `Default` 给坏字段兜底，两者一旦分叉，
    /// 「坏字段回退」就会悄悄变成「变成另一个默认值」。
    #[test]
    fn default_impl_equals_serde_defaults_field_by_field() {
        let from_impl = serde_json::to_value(AppSettings::default()).expect("序列化失败");
        let from_serde: AppSettings = serde_json::from_str("{}").expect("空对象反序列化失败");
        let from_serde = serde_json::to_value(from_serde).expect("序列化失败");

        assert_eq!(
            from_impl, from_serde,
            "AppSettings::default() 与 serde 缺省值必须逐字段相同"
        );
    }

    /// 写入单字段时：其他字段与**未知键**必须原样保留（Maccy 逐键语义）。
    #[test]
    fn write_setting_field_preserves_other_fields_and_unknown_keys() {
        let dir = std::env::temp_dir().join(format!("qq-settings-write-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(
            &path,
            r#"{
  "excluded_apps": ["1Password"],
  "selected_provider": "deepl",
  "max_history_items": 300,
  "theme": 123,
  "sort_order": "by_magic",
  "future_field": {"kept": true}
}"#,
        )
        .expect("写临时设置文件失败");

        write_setting_field(&path, "image_height_px", &40u32).expect("写字段失败");

        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("读回失败"))
                .expect("回写内容必须是合法 JSON");
        let map = written.as_object().expect("根必须是对象");

        assert_eq!(map["image_height_px"], Value::from(40));
        assert_eq!(
            map["excluded_apps"],
            serde_json::json!(["1Password"]),
            "排除名单不得被覆盖"
        );
        assert_eq!(map["selected_provider"], Value::from("deepl"));
        assert_eq!(map["max_history_items"], Value::from(300));
        assert_eq!(
            map["future_field"],
            serde_json::json!({"kept": true}),
            "未知键必须原样保留（未来版本/别的工具写的字段不能被抹掉）"
        );
        assert_eq!(
            map["theme"],
            Value::from(123),
            "本次没被写的坏字段也不该被改写——留给用户修"
        );
        assert_eq!(map["sort_order"], Value::from("by_magic"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 语法损坏的文件在写入前必须备份为 `settings.json.bak`，绝不静默覆盖。
    #[test]
    fn write_setting_field_backs_up_syntax_broken_file() {
        let dir = std::env::temp_dir().join(format!("qq-settings-bak-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        let original = "{ broken json ";
        std::fs::write(&path, original).expect("写临时设置文件失败");

        write_setting_field(&path, "theme", &"dark").expect("写字段失败");

        let backup = backup_path_for(&path);
        assert_eq!(
            std::fs::read_to_string(&backup).expect("备份文件应存在"),
            original,
            "备份必须是原文件的完整副本"
        );
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("读回失败"))
                .expect("回写内容必须是合法 JSON");
        assert_eq!(
            written,
            serde_json::json!({"theme": "dark"}),
            "损坏文件从空对象重建，只含本次写入的键"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 文件不存在时写入：直接创建，不产生无意义的 .bak。
    #[test]
    fn write_setting_field_creates_file_without_backup_when_missing() {
        let dir = std::env::temp_dir().join(format!("qq-settings-new-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");

        write_setting_field(&path, "stay_in_tray", &false).expect("写字段失败");

        assert!(!backup_path_for(&path).exists(), "首次创建不该留 .bak");
        assert!(!AppSettings::load_or_default(&path).stay_in_tray);
        std::fs::remove_dir_all(&dir).ok();
    }
}
