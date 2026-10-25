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
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// 文件存在但无法解析为 JSON 对象：**拒绝写入**，避免把用户设置覆盖成空对象/默认值。
    ///
    /// `Display` 就是要给用户看的文案：它经命令层 `to_string()` 原样传到前端。
    /// `backup` 是留痕副本的路径（带时间戳），提示里带上，方便用户去修。
    #[error(
        "{file} 无法解析（{reason}），已拒绝写入以免覆盖你的设置；请先修复该文件，本次未改动原文件。备份副本：{backup}"
    )]
    Unparsable {
        file: String,
        reason: String,
        backup: PathBuf,
    },
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

/// 搜索命中高亮样式（四档，对齐 Maccy 2.6.1 的 `HighlightMatch`）。
///
/// Maccy 源码 `Maccy/HighlightMatch.swift` 就是 `color / bold / italic / underline`，
/// 默认 `.bold`（`Defaults.Keys+Names.swift` 的 `highlightMatch` 键）。四档都是纯视觉：
/// 命中区间不变、<mark> 结构不变，只换 CSS 呈现。
///
/// 注意：默认由本项目的「背景+加粗」改为 Maccy 的 `bold`（纯加粗、无背景），
/// 这是**有意的默认视觉变化**，见 docs/design/v04-work-items.md P5-5 的登记。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightStyle {
    /// 背景高亮（对齐 Maccy 的 `.color`：命中片段整段染色）
    Color,
    /// 纯加粗（Maccy 默认档）
    #[default]
    Bold,
    /// 斜体
    Italic,
    /// 下划线
    Underline,
}

/// 弹出面板行内图片缩略图高度的合法区间下限与上限（px）。
///
/// 对齐 Maccy 2.6.1：`imageMaxHeight` 键默认 40，Stepper 范围 `1...200`
/// （`Defaults.Keys+Names.swift` / `Settings/AppearanceSettingsPane.swift`）。
/// Maccy 的输入框 formatter 另有 30–200 的软约束，但 Stepper 允许 1，故本项以 Stepper 为准。
pub const MIN_IMAGE_HEIGHT_PX: u32 = 1;

/// 见 [`MIN_IMAGE_HEIGHT_PX`]（Maccy Stepper 上限 200）。
pub const MAX_IMAGE_HEIGHT_PX: u32 = 200;

/// 弹出面板行内图片缩略图高度的默认值（px）。
///
/// 取 Maccy 的默认 40（本项目此前为 24）。这是**有意的默认视觉变化**：
/// 升级后弹窗行内图片会更高，见 docs/design/v04-work-items.md P5-5 的登记。
pub const DEFAULT_IMAGE_HEIGHT_PX: u32 = 40;

/// 首次预览延迟的合法区间下限（毫秒）。
///
/// 与「后续切换 200ms」同一档：首个预览比后续切换还快就没有「先看列表」的意义。
pub const MIN_PREVIEW_FIRST_DELAY_MS: u32 = 200;

/// 首次预览延迟的合法区间上限（毫秒）。
///
/// 对齐 Maccy 2.6.1：`previewDelay` Stepper 范围 `200...100_000`，默认 1500。
/// 上限放宽到 100 秒是照抄 Maccy 的选择——「等太久等于没预览」只是观感偏好，
/// 不该由我们替用户否决。
pub const MAX_PREVIEW_FIRST_DELAY_MS: u32 = 100_000;

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
    /// 弹出面板首个行内预览的等待时长（毫秒，200..=100000，默认 1500）
    ///
    /// 只作用于「面板本次打开后的首个预览」；后续切换固定 200ms 不开放配置，
    /// 因为那一档的取值只影响翻页时中间项闪不闪，没有第二种合理值。
    #[serde(default = "default_preview_first_delay_ms")]
    pub preview_first_delay_ms: u32,
    /// 搜索命中高亮样式（四档，默认 `Bold`；对齐 Maccy 的 `HighlightMatch`）
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
    /// 文件不存在是正常首启动，直接返回默认值；读取其它 I/O 错误也回退默认值
    /// （**行为不变**），但会走与「文件损坏」同一套一次性留痕：读盘失败只提示一次，
    /// 一旦某次真的读到了内容就复位标记——否则入库轮询每约 500ms 一轮会把 stderr 刷屏。
    ///
    /// 返回的第二个元素是「被回退的字段 / 被忽略的原因」清单，空表示整份设置都解析成功。
    pub fn load_lenient(path: &Path) -> (Self, Vec<String>) {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), Vec::new());
            }
            Err(err) => {
                let reason = format!("读取设置文件失败：{err}");
                warn_read_failure_once(&CORRUPT_SETTINGS_WARNED, &err.to_string());
                return (Self::default(), vec![reason]);
            }
        };

        let (settings, problems, whole_file_fallback) = Self::lenient_from_json_str(&content);
        // 整份回退 = 用户的设置这一轮全部按默认值生效，必须留痕；
        // 但读盘可能每约 500ms 一次，所以同一份损坏只提示一次；解析成功后复位，修复后再坏可再提示。
        apply_corrupt_warning(&CORRUPT_SETTINGS_WARNED, whole_file_fallback, &problems);
        (settings, problems)
    }

    /// 把整份 JSON 文本按字段级容错解析。
    ///
    /// 返回 `(设置, 问题清单, 是否整份回退)`：第三项为真表示语法损坏 / 根不是对象，
    /// 此时所有字段都取了默认值——读路径据此决定要不要打「启动期一次性提示」。
    /// 字段级坏值不算整份回退（只有那个字段回默认）。
    fn lenient_from_json_str(content: &str) -> (Self, Vec<String>, bool) {
        // 空文件（0 字节 / 只有空白）与写路径保持同一口径：按「还没有任何设置」处理，
        // 不算损坏、不整份回退、不消耗「已提示过损坏」的标记——
        // 否则 `touch settings.json` 就会被当成语法错误并吃掉那唯一一次提示。
        if content.trim().is_empty() {
            return (Self::default(), Vec::new(), false);
        }

        match serde_json::from_str::<Value>(content) {
            Ok(Value::Object(map)) => {
                let (settings, problems) = Self::from_json_object_lenient(&map);
                (settings, problems, false)
            }
            Ok(_) => (
                Self::default(),
                vec!["设置文件根不是 JSON 对象，整份按默认值生效".to_string()],
                true,
            ),
            Err(err) => (
                Self::default(),
                vec![format!("设置文件 JSON 语法错误，整份按默认值生效：{err}")],
                true,
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

/// 进程级「已提示过设置文件损坏」标记。
///
/// 读盘在入库轮询里每约 500ms 发生一次；没有它，一个坏文件会把 stderr 刷成每秒两行。
static CORRUPT_SETTINGS_WARNED: AtomicBool = AtomicBool::new(false);

/// 一次性留痕的公共实现：标记已置位就跳过，否则置位并执行 `print`。
///
/// 损坏提示与读盘失败提示共用同一个 [`CORRUPT_SETTINGS_WARNED`]：
/// 一个打不开的 settings.json 无论以哪种形式坏，都只值得提醒用户一次。
fn warn_once(flag: &AtomicBool, print: impl FnOnce()) -> bool {
    if flag.swap(true, Ordering::Relaxed) {
        return false;
    }
    print();
    true
}

/// 根据本次读取结果维护「已提示过损坏」标记：损坏 → 只提示一次；解析成功 → 复位。
///
/// 复位是让「修复后再坏」还能再提示一次：否则用户修好过一次之后，
/// 同一进程里再遇到坏文件就完全静默了。
///
/// `flag` 由调用方注入：生产传进程级 [`CORRUPT_SETTINGS_WARNED`]，
/// 测试可传局部标记验证完整时序，而不受其它用例干扰。
/// 返回本次是否真的打印了提示。
fn apply_corrupt_warning(
    flag: &AtomicBool,
    whole_file_fallback: bool,
    problems: &[String],
) -> bool {
    if !whole_file_fallback {
        // 成功解析为 JSON 对象（哪怕有字段级坏值）：文件已经不是「打不开」的状态了。
        flag.store(false, Ordering::Relaxed);
        return false;
    }
    let reason = problems
        .first()
        .map(String::as_str)
        .unwrap_or("内容无法解析")
        .to_string();
    warn_once(flag, || {
        eprintln!(
            "[QuickQuick] settings.json {reason}；本次按默认设置运行（同一份损坏只提示一次）。请修复该文件后重启应用。"
        );
    })
}

/// 读盘失败（权限、I/O 等；「文件不存在」不算失败）时的一次性留痕。
///
/// 与 [`apply_corrupt_warning`] 共用同一个去重标记：后者在「读到并能解析」时会把它复位，
/// 所以「读取失败 → 修好权限 → 再次失败」仍能再提示一次。
/// **不改变**回退行为本身（调用方继续按默认值跑）。
fn warn_read_failure_once(flag: &AtomicBool, error: &str) -> bool {
    warn_once(flag, || {
        eprintln!(
            "[QuickQuick] settings.json 读取失败（{error}），本次使用默认设置（该提示只打一次）。请检查文件权限后重启应用。"
        );
    })
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

/// 与某个设置文件同目录的备份文件名：`settings.json` → `settings.json.<seq>.<unix_ms>.bak`。
///
/// `seq` 是单调写入序号（排序依据），`unix_ms` 是真实墙钟（只用于展示）。
/// 两个字段都在名字里，是为了既不覆盖已有备份，又能让人一眼看出「第几次、什么时候」。
pub fn backup_path_for(path: &Path, seq: u64, unix_ms: i64) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{seq}.{unix_ms}.bak"));
    path.with_file_name(name)
}

/// 解析备份文件名 → `(写入序号, 时间戳)`；不匹配本应用备份命名的一律返回 None。
///
/// 备份名形如 `<文件名>.<seq>.<ms>.bak`：`seq` 是单调写入序号（排序依据），`ms` 只用于展示。
/// 只认这一种形态：目录里其它文件（用户手放的、旧版遗留的）既不参与排序也不参与清理。
fn parse_backup_name(file_name: &str, prefix: &str) -> Option<(u64, i64)> {
    let rest = file_name.strip_prefix(prefix)?.strip_suffix(".bak")?;
    let (seq_part, ms_part) = rest.split_once('.')?;
    let seq: u64 = seq_part.parse().ok()?;
    let ms: i64 = ms_part.parse().ok()?;
    Some((seq, ms))
}

/// 目标文件对应的备份前缀：`settings.json` → `settings.json.`。
fn backup_prefix(path: &Path) -> Option<String> {
    let file_name = path.file_name().and_then(|name| name.to_str())?;
    Some(format!("{file_name}."))
}

/// 当前 unix 毫秒时间戳（只用于备份文件名；取不到时间时退化为 0，仍能生成备份）。
fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

/// 每个配置文件滚动保留的备份份数（含最新一份）。
///
/// 依据：业界常见做法是只留最近几份（N=5 足够覆盖"改坏了想回退"的场景）；
/// 只增不删会让目录里堆出几十上百个 `.bak`，反而没人愿意去找。
pub const BACKUP_KEEP_COUNT: usize = 5;

/// 某个配置文件的既有备份，按**写入序号**升序（最旧在前）。
///
/// 只有形如 `<文件名>.<seq>.<ms>.bak` 的文件会被收进来；其它文件一概不算备份
/// （既不参与排序，也不会被 prune 删掉）。另一个配置的备份同理不会被误删。
fn list_backups(path: &Path) -> Vec<PathBuf> {
    let Some(prefix) = backup_prefix(path) else {
        return Vec::new();
    };
    let Some(dir) = path.parent() else {
        return Vec::new();
    };

    let mut found: Vec<(u64, i64, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            let (seq, ms) = parse_backup_name(&name, &prefix)?;
            Some((seq, ms, entry.path()))
        })
        .collect();
    found.sort_by_key(|(seq, ms, _)| (*seq, *ms));
    found.into_iter().map(|(_, _, path)| path).collect()
}

/// 下一个备份写入序号：磁盘上已有新格式备份的最大 seq + 1（无备份时为 1）。
///
/// 取「磁盘上已有的最大 seq」而不是进程内计数器，是为了**跨重启仍然递增**；
/// 也正因为它只看磁盘，墙钟回拨不会让序号倒退。
fn next_backup_seq(path: &Path) -> u64 {
    let Some(prefix) = backup_prefix(path) else {
        return 1;
    };
    let Some(dir) = path.parent() else {
        return 1;
    };
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            let (seq, _) = parse_backup_name(&name, &prefix)?;
            Some(seq)
        })
        .max()
        .map(|max| max.saturating_add(1))
        .unwrap_or(1)
}

/// 只保留最近 [`BACKUP_KEEP_COUNT`] 份备份，更旧的删掉。
///
/// `keep` 是**刚创建的那份**，永远不参与淘汰：备份名按时间戳排序，
/// 一旦系统时钟回拨，新备份的时间戳会小于旧的，纯按时间排序会把它当成最旧的删掉——
/// 那正是「刚刚保护下来的文件」被清掉、只剩几份旧垃圾的最坏情形。
fn prune_backups(path: &Path, keep: &Path) {
    let mut candidates: Vec<PathBuf> = list_backups(path)
        .into_iter()
        .filter(|backup| backup != keep)
        .collect();
    if candidates.len() < BACKUP_KEEP_COUNT {
        return;
    }
    // 除 keep 外再留 KEEP_COUNT - 1 份最新的，其余删掉
    let keep_others = BACKUP_KEEP_COUNT - 1;
    let delete_count = candidates.len() - keep_others;
    for stale in candidates.drain(..delete_count) {
        if let Err(err) = std::fs::remove_file(&stale) {
            // 删不掉不是致命问题：留着的备份只是多占一点空间
            eprintln!("[QuickQuick] 清理旧备份失败（{}）：{err}", stale.display());
        }
    }
}

/// 把损坏的配置文件保护性备份一份，返回备份路径（同内容会自动复用已有备份）。
///
/// 供**读路径**使用：启动期发现 hotkey.json 损坏时，先留痕再去展示修复指引，
/// 这样用户点开面板时真的能找到一个 `.bak` 去对照修。
///
/// # Errors
/// 备份失败（目录只读等）返回 [`SettingsError::IoError`]；调用方应降级为「无备份路径」。
pub fn backup_corrupt_file(path: &Path, reason: &str) -> Result<PathBuf, SettingsError> {
    backup_broken_settings(path, reason)
}

/// 校验 `path` 可以安全写入：文件不存在 / 空文件 / 可解析为 JSON 对象都算通过；
/// 语法坏 / 根不是对象 / 读盘失败返回 Err（前者的 Err 里带已生成的备份路径）。
///
/// 供「本来不打算写盘」的早退分支使用——例如改热键时新旧键相同会早退，
/// 但此时文件可能已经损坏，早退返回成功会让用户以为配置是好的。
///
/// # Errors
/// 同 [`update_json_object`]。
pub fn ensure_writable_json_object(path: &Path) -> Result<(), SettingsError> {
    read_settings_object_for_write(path).map(|_| ())
}

/// 选一个尚未占用的备份路径：`<seq>.<ms>.bak`，`seq` 从磁盘最大值 +1 起递增。
///
/// 同一毫秒内连续触发也不会互相覆盖——区分靠的是 seq，不是时间戳。
/// 时间戳由调用方给（生产用 [`now_unix_ms`]，测试用它模拟时钟回拨），
/// 它**只用于展示**，不参与排序。
fn unique_backup_path_at(path: &Path, now_ms: i64) -> Result<PathBuf, SettingsError> {
    let mut seq = next_backup_seq(path);
    for _ in 0..1000 {
        let candidate = backup_path_for(path, seq, now_ms);
        if !candidate.exists() {
            return Ok(candidate);
        }
        seq = seq.saturating_add(1);
    }
    Err(SettingsError::IoError(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "备份文件过多，拒绝继续写入以免覆盖留痕",
    )))
}

/// 读 settings.json 为 JSON 对象；文件不存在返回空对象（首次写就是新建）。
///
/// 文件存在但**语法损坏 / 根不是对象**时：先备份留痕，然后**拒绝写入**（返回
/// [`SettingsError::Unparsable`]）。理由：无法解析的文件里可能全是用户的真实设置，
/// 从空对象重建再写回去等于把它们删掉；宁可让用户看到明确错误去修，也不静默覆盖。
///
/// 备份失败（目录只读等）同样返回 Err，且原文件一个字节都不动。
fn read_settings_object_for_write(path: &Path) -> Result<Map<String, Value>, SettingsError> {
    let file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("配置文件")
        .to_string();

    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(err) => return Err(SettingsError::IoError(err)),
    };

    // 空文件（0 字节 / 只有空白）按「还没有任何设置」处理，不当作损坏：
    // 它里面没有可丢的用户数据，`touch settings.json` 后写第一次设置也不该被拒。
    if content.trim().is_empty() {
        return Ok(Map::new());
    }

    match serde_json::from_str::<Value>(&content) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(other) => {
            let reason = format!("根是 {} 而不是 JSON 对象", value_kind(&other));
            let backup = backup_broken_settings(path, &reason)?;
            Err(SettingsError::Unparsable {
                file,
                reason,
                backup,
            })
        }
        Err(err) => {
            let reason = format!("JSON 语法错误：{err}");
            let backup = backup_broken_settings(path, &reason)?;
            Err(SettingsError::Unparsable {
                file,
                reason,
                backup,
            })
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

/// 备份损坏的 settings.json 并留痕，返回备份路径；备份失败返回 Err。
///
/// 备份成功**也不继续写**——调用方会返回 [`SettingsError::Unparsable`] 拒写。
/// 备份只是留痕，让用户能对照修；活文件保持原样。
fn backup_broken_settings(path: &Path, reason: &str) -> Result<PathBuf, SettingsError> {
    backup_broken_settings_at(path, reason, now_unix_ms())
}

/// 同 [`backup_broken_settings`]，但时间戳由调用方给（测试用来模拟时钟回拨）。
fn backup_broken_settings_at(
    path: &Path,
    reason: &str,
    now_ms: i64,
) -> Result<PathBuf, SettingsError> {
    let file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("配置文件")
        .to_string();
    let content = std::fs::read(path)?;

    // 与最近一份备份字节相同：直接复用，不再新建（同一份坏内容反复触发时不堆文件）
    if let Some(latest) = list_backups(path).last() {
        if std::fs::read(latest)
            .map(|bytes| bytes == content)
            .unwrap_or(false)
        {
            eprintln!(
                "[QuickQuick] {file} {reason}，与最近一份备份内容相同，复用 {} 并拒绝写入以免覆盖你的设置",
                latest.display()
            );
            return Ok(latest.clone());
        }
    }

    // 文件名是 `<seq>.<ms>.bak`：**排序只看 seq（写入次序），ms 是真实墙钟、只用于展示**。
    //
    // 为什么不用墙钟排序：系统时钟回拨时新备份的时间戳会小于旧备份，按名字排就成了「最旧的」，
    // 连续两次回拨会把上一次刚创建的备份挤出保留名额。为什么不用 mtime 排序（另一个候选）：
    // macOS 上 `fs::copy` **保留源文件 mtime**（实测：源设 2001 年，目标也是 2001 年），
    // 且 mtime 可被外部工具（`touch`、`cp -p`、归档还原）任意设定，不代表本进程的写入次序
    // （Linux 行为未验证）。把写入次序编进文件名里的 seq，才是只依赖本进程写入顺序的做法。
    let backup = unique_backup_path_at(path, now_ms)?;
    std::fs::copy(path, &backup)?;
    prune_backups(path, &backup);
    eprintln!(
        "[QuickQuick] {file} {reason}，已备份原文件到 {}，并拒绝写入以免覆盖你的设置",
        backup.display()
    );
    Ok(backup)
}

/// 以「读原始 JSON 对象 → 只改目标键 → 写回」的方式更新 settings.json。
///
/// 与 [`AppSettings::save`] 的区别就是本函数存在的理由：**未知键与其他字段原样保留**。
/// 于是「某个字段的类型写坏」不再演变成「整份设置被默认值覆盖」——
/// 写坏的键会留在文件里（用户可修），其余键一个字节都不动。
///
/// **前提（重要）**：当前设置类 IPC 全部是**同步**命令，Tauri 在同一条命令内串行执行，
/// 所以这里的「读 → 改 → 写」没有加锁。若将来有命令改成 `async`（或在同一条命令里
/// `await` 别的东西），两个并发写会互相吞掉对方的键——那时必须给本函数加互斥
/// （例如进程级 `Mutex<()>` 或文件锁）。改 async 的人请连带补这条。
///
/// 写完顺手体检一次已知字段：仍有解析失败的字段就 `eprintln` 留痕。
/// 只在写入时体检而不是每次读盘，是为了不让入库轮询（约 500ms 一轮）刷日志。
///
/// # Errors
/// - `SettingsError::Unparsable`：文件存在但语法损坏/根不是对象 —— **拒写**（不丢键），
///   备份已落盘，错误文案带备份路径
/// - `SettingsError::IoError`：读文件、备份或写文件失败（备份失败时同样不写）
/// - `SettingsError::SerdeError`：目标键的值无法序列化（只可能是调用方传了非法类型）
pub fn update_settings_value<F>(path: &Path, mutate: F) -> Result<(), SettingsError>
where
    F: FnOnce(&mut Map<String, Value>),
{
    update_json_object(path, |map| {
        mutate(map);
        // settings.json 专属体检：写完顺手看一眼已知字段，坏字段点名但不改文件
        log_unparsable_fields(map);
    })
}

/// 任意「单文件 JSON 对象」配置的逐键更新（`hotkey.json` / `autostart.json` 也走这里）。
///
/// 语义与 settings.json 完全一致：读原始对象 → 只改目标键 → 写回，
/// **未知键与其他字段原样保留**；文件语法坏 / 根不是对象时先备份再**拒写**（[`SettingsError::Unparsable`]）。
/// 与 [`update_settings_value`] 的唯一差别是**不做 settings 专属的字段体检**。
///
/// # Errors
/// 同 [`update_settings_value`]。
pub fn update_json_object<F>(path: &Path, mutate: F) -> Result<(), SettingsError>
where
    F: FnOnce(&mut Map<String, Value>),
{
    let mut map = read_settings_object_for_write(path)?;
    mutate(&mut map);
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
        assert_eq!(d.highlight_style, HighlightStyle::Bold);
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

    /// 旧 JSON（无四个外观字段）反序列化后取默认值：排序=最近复制、缩略图 40px、
    /// 首次预览 1500ms、高亮=加粗。
    ///
    /// 缩略图 40px 与高亮 bold 是**对齐 Maccy 后有意改掉的默认视觉**（此前 24px / 背景+加粗），
    /// 见 docs/design/v04-work-items.md P5-5 登记；排序与预览延迟的默认值未变。
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
            settings.image_height_px, 40,
            "默认缩略图高度对齐 Maccy imageMaxHeight 的 40"
        );
        assert_eq!(
            settings.preview_first_delay_ms, 1500,
            "默认首次预览延迟必须等于改动前的 1500ms"
        );
        assert_eq!(
            settings.highlight_style,
            HighlightStyle::Bold,
            "默认高亮对齐 Maccy highlightMatch 的 bold"
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

    /// 读路径兜底：手写 0px 会被收敛到下限 1px（Maccy Stepper 下限）。
    #[test]
    fn effective_image_height_px_clamps_below_min() {
        let settings = AppSettings {
            image_height_px: 0,
            ..AppSettings::default()
        };

        assert_eq!(settings.effective_image_height_px(), MIN_IMAGE_HEIGHT_PX);
        assert_eq!(settings.effective_image_height_px(), 1);
    }

    /// 读路径兜底：手写 999px 会被收敛到上限 200px（Maccy Stepper 上限）。
    #[test]
    fn effective_image_height_px_clamps_above_max() {
        let settings = AppSettings {
            image_height_px: 999,
            ..AppSettings::default()
        };

        assert_eq!(settings.effective_image_height_px(), MAX_IMAGE_HEIGHT_PX);
        assert_eq!(settings.effective_image_height_px(), 200);
    }

    /// 缩略图高度区间内的值原样透出（含两端）。
    #[test]
    fn effective_image_height_px_keeps_boundary_values_intact() {
        for value in [MIN_IMAGE_HEIGHT_PX, 40, MAX_IMAGE_HEIGHT_PX] {
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

    /// 读路径兜底：手写 200_000ms 会被收敛到上限 100_000ms（Maccy 的上限）。
    #[test]
    fn effective_preview_first_delay_ms_clamps_above_max() {
        let settings = AppSettings {
            preview_first_delay_ms: 200_000,
            ..AppSettings::default()
        };

        assert_eq!(
            settings.effective_preview_first_delay_ms(),
            MAX_PREVIEW_FIRST_DELAY_MS
        );
        assert_eq!(settings.effective_preview_first_delay_ms(), 100_000);
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

    /// 目录下所有备份文件（`settings.json.*.bak`）。
    fn backup_files(dir: &Path) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("读临时目录失败")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("settings.json.") && name.ends_with(".bak")
                    })
            })
            .collect();
        found.sort();
        found
    }

    /// 语法损坏的文件：**备份留痕但拒写**，原文件字节不变、不丢一个键。
    #[test]
    fn write_setting_field_rejects_syntax_broken_file_and_keeps_it_intact() {
        let dir = std::env::temp_dir().join(format!("qq-settings-bak-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        let original = b"{ broken json ".to_vec();
        std::fs::write(&path, &original).expect("写临时设置文件失败");

        let err = write_setting_field(&path, "theme", &"dark").expect_err("损坏文件必须拒写");

        assert!(
            matches!(err, SettingsError::Unparsable { .. }),
            "应是「无法解析」错误而不是 I/O 错误，实际：{err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains("已拒绝写入") && message.contains("修复"),
            "错误文案要能直接给用户看，实际：{message}"
        );

        assert_eq!(
            std::fs::read(&path).expect("读回失败"),
            original,
            "拒写后原文件必须逐字节不变"
        );

        let backups = backup_files(&dir);
        assert_eq!(backups.len(), 1, "应留一份备份，实际：{backups:?}");
        assert_eq!(
            std::fs::read(&backups[0]).expect("读备份失败"),
            original,
            "备份必须是原文件的完整副本"
        );
        assert!(
            message.contains(
                backups[0]
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("备份文件名非法 UTF-8")
            ),
            "错误文案应带上备份文件名，方便用户去找，实际：{message}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 滚动保留：连续 7 次**不同的**损坏内容 → 目录里只留最新 5 份，且就是最后写的那 5 份。
    #[test]
    fn backups_keep_only_the_latest_five() {
        let dir = std::env::temp_dir().join(format!("qq-settings-rotate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");

        for round in 1..=7 {
            std::fs::write(&path, format!("{{ broken {round} ")).expect("写临时设置文件失败");
            write_setting_field(&path, "theme", &"dark").expect_err("每轮都应拒写");
        }

        let backups = backup_files(&dir);
        assert_eq!(
            backups.len(),
            BACKUP_KEEP_COUNT,
            "只应保留最近 {BACKUP_KEEP_COUNT} 份，实际：{backups:?}"
        );
        let contents: Vec<String> = backups
            .iter()
            .map(|path| std::fs::read_to_string(path).expect("读备份失败"))
            .collect();
        for round in 3..=7 {
            assert!(
                contents.contains(&format!("{{ broken {round} ")),
                "第 {round} 轮的内容应在保留范围内，实际：{contents:?}"
            );
        }
        for round in 1..=2 {
            assert!(
                !contents.contains(&format!("{{ broken {round} ")),
                "第 {round} 轮应已被淘汰，实际：{contents:?}"
            );
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 目录里存在非本应用命名形态的文件（旧版遗留、用户手放的）时：
    /// 它们不参与排序与清理，新备份与 prune 照常工作，且**这些文件不会被删**。
    #[test]
    fn foreign_backup_like_files_are_ignored_and_never_pruned() {
        let dir = std::env::temp_dir().join(format!("qq-settings-foreign-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");

        // 非本格式：旧版遗留（<ms>.bak）与完全无关的文件
        let legacy_ms = path.with_file_name("settings.json.1700000000000.bak");
        let legacy_seq_suffix = path.with_file_name("settings.json.1700000000001-2.bak");
        let unrelated = path.with_file_name("settings.json.bak");
        let other_config = path.with_file_name("hotkey.json.1.1700000000000.bak");
        for file in [&legacy_ms, &legacy_seq_suffix, &unrelated, &other_config] {
            std::fs::write(file, "should not be touched").expect("写非本格式文件失败");
        }

        // 连续 6 次损坏 → 6 份新格式备份 → prune 保留 5 份
        for round in 1..=6 {
            std::fs::write(&path, format!("{{ broken {round} ")).expect("写临时设置文件失败");
            write_setting_field(&path, "theme", &"dark").expect_err("每轮都应拒写");
        }

        let backups = backup_files(&dir);
        let ours: Vec<&std::path::PathBuf> = backups
            .iter()
            .filter(|backup| {
                let name = backup.file_name().and_then(|n| n.to_str()).unwrap_or("");
                parse_backup_name(name, "settings.json.").is_some()
            })
            .collect();
        assert_eq!(
            ours.len(),
            BACKUP_KEEP_COUNT,
            "本应用格式的备份应收敛到 {BACKUP_KEEP_COUNT} 份，实际：{backups:?}"
        );
        for file in [&legacy_ms, &legacy_seq_suffix, &unrelated, &other_config] {
            assert!(
                file.exists(),
                "非本格式文件不得被 prune 删除：{}",
                file.display()
            );
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 备份文件名里的 `ms` 必须是**真实墙钟**：目录里的未来时间戳文件不得把新备份的时间戳顶到未来。
    ///
    /// 旧实现用 `max(now, newest+1)` 保序，一次未来时间戳会永久传染——新备份的时间戳再也不是真实时间，
    /// 用户按时间找「今天改坏的那份」会找不到。现在顺序交给 seq，ms 只用于展示。
    #[test]
    fn new_backup_keeps_real_wall_clock_when_legacy_backup_is_in_the_future() {
        let dir = std::env::temp_dir().join(format!("qq-settings-future-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ broken now ").expect("写临时设置文件失败");

        // 非本应用备份格式的文件，时间戳设成 2100 年（旧版遗留/手工放置都可能这样）
        let future_ms: i64 = 4_102_444_800_000;
        let legacy = path.with_file_name(format!("settings.json.{future_ms}.bak"));
        std::fs::write(&legacy, "2100 future file").expect("写未来时间戳文件失败");

        let now_ms = 1_600_000_000_000;
        let created =
            backup_broken_settings_at(&path, "JSON 语法错误（测试）", now_ms).expect("备份应成功");

        let name = created
            .file_name()
            .and_then(|name| name.to_str())
            .expect("备份文件名非法 UTF-8")
            .to_string();
        let (seq, ms) = parse_backup_name(&name, "settings.json.").expect("新备份名应可解析");
        assert_eq!(
            ms, now_ms,
            "新备份的 ms 必须是真实墙钟（注入值），不能被旧备份的 2100 年传染，实际：{name}"
        );
        assert_eq!(
            seq, 1,
            "非本格式文件不占序号，新备份从 1 开始，实际：{name}"
        );
        assert!(
            legacy.exists(),
            "非本格式文件不参与清理，必须原样留着：{}",
            legacy.display()
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 连续写入 7 次：只保留最新 5 份，且它们的 seq 严格递增。
    #[test]
    fn backup_sequence_is_strictly_increasing_and_capped() {
        let dir = std::env::temp_dir().join(format!("qq-settings-seq-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");

        for round in 1..=7 {
            std::fs::write(&path, format!("{{ broken {round} ")).expect("写临时设置文件失败");
            write_setting_field(&path, "theme", &"dark").expect_err("每轮都应拒写");
        }

        let backups = backup_files(&dir);
        assert_eq!(
            backups.len(),
            BACKUP_KEEP_COUNT,
            "只应保留 {BACKUP_KEEP_COUNT} 份"
        );

        let seqs: Vec<u64> = backups
            .iter()
            .map(|path| {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .expect("文件名非法");
                let (seq, _) = parse_backup_name(name, "settings.json.").expect("应可解析");
                seq
            })
            .collect();
        assert!(
            seqs.windows(2).all(|pair| pair[0] < pair[1]),
            "保留的备份 seq 必须严格递增，实际：{seqs:?}"
        );
        assert_eq!(seqs.last().copied(), Some(7), "最新的 seq 应是第 7 次写入");

        // 内容对应第 3–7 轮（第 1、2 轮已被淘汰）
        let contents: Vec<String> = backups
            .iter()
            .map(|path| std::fs::read_to_string(path).expect("读备份失败"))
            .collect();
        for round in 3..=7 {
            assert!(
                contents.contains(&format!("{{ broken {round} ")),
                "缺第 {round} 轮"
            );
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 同一毫秒内连续写入：两份备份都保留，由 seq 区分文件名。
    #[test]
    fn same_millisecond_backups_do_not_collide() {
        let dir = std::env::temp_dir().join(format!("qq-settings-samems-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        let now_ms = 1_600_000_000_000;

        std::fs::write(&path, "{ first ").expect("写临时设置文件失败");
        let first = backup_broken_settings_at(&path, "测试", now_ms).expect("第一次备份应成功");
        std::fs::write(&path, "{ second ").expect("写临时设置文件失败");
        let second = backup_broken_settings_at(&path, "测试", now_ms).expect("第二次备份应成功");

        assert_ne!(first, second, "同一毫秒的两次备份不能共用同一个文件名");
        assert!(first.exists() && second.exists(), "两份备份都应存在");
        let backups = backup_files(&dir);
        assert_eq!(backups.len(), 2, "应有两份备份，实际：{backups:?}");
        // 两份都保留、由 seq 区分（同一毫秒只体现在 ms 相同上，不参与命名唯一性）
        for backup in &backups {
            let name = backup
                .file_name()
                .and_then(|n| n.to_str())
                .expect("文件名非法");
            let (_, ms) = parse_backup_name(name, "settings.json.").expect("应可解析");
            assert_eq!(ms, now_ms, "ms 仍是真实墙钟：{name}");
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 连续两次时钟回拨：第一次创建的备份不能被第二次挤掉，总数不超过保留上限。
    ///
    /// 旧实现按墙钟时间戳排序：第二次回拨（更小的 now）生成的备份排在更前，
    /// 会把「第一次刚创建的备份」当成最旧的删掉。现在文件名时间戳单调化
    /// （取 max(now, 既有最大值 + 1)），新写的永远排最后，`keep` 之外只按写入次序淘汰。
    #[test]
    fn consecutive_clock_rollbacks_keep_both_new_backups() {
        let dir =
            std::env::temp_dir().join(format!("qq-settings-rollback2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");

        // 5 份「未来」备份占满保留名额
        let future_ms = 4_102_444_800_000; // 2100-01-01
        for offset in 0..BACKUP_KEEP_COUNT as i64 {
            let backup = backup_path_for(&path, offset as u64 + 1, future_ms + offset);
            std::fs::write(&backup, format!("old-{offset}")).expect("写旧备份失败");
        }

        // 第一次回拨：now = 1600000000000
        std::fs::write(&path, "{ rollback one ").expect("写临时设置文件失败");
        let first =
            backup_broken_settings_at(&path, "JSON 语法错误（第一次回拨）", 1_600_000_000_000)
                .expect("第一次备份应成功");

        // 第二次回拨：now 只比第一次大 1ms，仍远小于既有备份的时间戳
        std::fs::write(&path, "{ rollback two ").expect("写临时设置文件失败");
        let second =
            backup_broken_settings_at(&path, "JSON 语法错误（第二次回拨）", 1_600_000_000_001)
                .expect("第二次备份应成功");

        assert!(
            first.exists(),
            "第一次创建的备份不得被第二次回拨挤掉：{}",
            first.display()
        );
        assert!(
            second.exists(),
            "第二次创建的备份必须存在：{}",
            second.display()
        );
        let backups = backup_files(&dir);
        assert!(
            backups.len() <= BACKUP_KEEP_COUNT,
            "总份数不得超过保留上限 {BACKUP_KEEP_COUNT}，实际：{backups:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&first).expect("读备份失败"),
            "{ rollback one "
        );
        assert_eq!(
            std::fs::read_to_string(&second).expect("读备份失败"),
            "{ rollback two "
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 时钟回拨保护：新备份即使时间戳比旧备份小，也不得被 prune 当成最旧的删掉。
    ///
    /// 通过可注入的时间源模拟：先造 5 份「未来时间戳」的旧备份，再用一个更小的 now 触发备份，
    /// 旧实现会按时间排序把刚创建的那份排到最前并删掉（等于刚保护下来就丢了）。
    #[test]
    fn prune_never_deletes_the_just_created_backup_when_clock_rolls_back() {
        let dir = std::env::temp_dir().join(format!("qq-settings-rollback-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ rollback broken ").expect("写临时设置文件失败");

        // 5 份「未来」备份（时间戳远大于本次使用的 now）
        let future_ms = 4_102_444_800_000; // 2100-01-01
        for offset in 0..BACKUP_KEEP_COUNT as i64 {
            let backup = backup_path_for(&path, offset as u64 + 1, future_ms + offset);
            std::fs::write(&backup, format!("old-{offset}")).expect("写旧备份失败");
        }

        // 时钟回拨：now 远小于既有备份的时间戳
        let now_ms = 1_600_000_000_000;
        let created =
            backup_broken_settings_at(&path, "JSON 语法错误（测试）", now_ms).expect("备份应成功");

        assert!(
            created.exists(),
            "刚创建的备份必须还在：{}",
            created.display()
        );
        let backups = backup_files(&dir);
        assert_eq!(
            backups.len(),
            BACKUP_KEEP_COUNT,
            "总份数仍应收敛到 {BACKUP_KEEP_COUNT}，实际：{backups:?}"
        );
        assert!(
            backups.contains(&created),
            "prune 不得删除刚创建的备份，实际：{backups:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&created).expect("读备份失败"),
            "{ rollback broken ",
            "刚创建的备份内容必须是本次的原文件"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 去重：同一份坏内容触发两次，只留一份备份（不堆同内容副本）。
    #[test]
    fn identical_corrupt_content_reuses_existing_backup() {
        let dir = std::env::temp_dir().join(format!("qq-settings-dedupe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ same broken content ").expect("写临时设置文件失败");

        write_setting_field(&path, "theme", &"dark").expect_err("第一次应拒写");
        write_setting_field(&path, "theme", &"dark").expect_err("第二次应拒写");

        let backups = backup_files(&dir);
        assert_eq!(backups.len(), 1, "同内容只应留一份备份，实际：{backups:?}");
        assert_eq!(
            std::fs::read_to_string(&backups[0]).expect("读备份失败"),
            "{ same broken content "
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 连续两次损坏：两份备份都在（带时间戳 + 序号，不互相覆盖）。
    #[test]
    fn two_corrupt_writes_keep_two_backups() {
        let dir = std::env::temp_dir().join(format!("qq-settings-bak2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ broken once ").expect("写临时设置文件失败");
        write_setting_field(&path, "theme", &"dark").expect_err("第一次应拒写");

        std::fs::write(&path, "{ broken twice ").expect("写临时设置文件失败");
        write_setting_field(&path, "theme", &"dark").expect_err("第二次应拒写");

        let backups = backup_files(&dir);
        assert_eq!(backups.len(), 2, "两次损坏应留两份备份，实际：{backups:?}");
        let contents: Vec<String> = backups
            .iter()
            .map(|path| std::fs::read_to_string(path).expect("读备份失败"))
            .collect();
        assert!(
            contents.contains(&"{ broken once ".to_string())
                && contents.contains(&"{ broken twice ".to_string()),
            "两份备份应分别是两次的原内容，实际：{contents:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 备份失败（目录不可写）时：返回 Err 且原文件字节不变——绝不在没有留痕的情况下动活文件。
    #[cfg(unix)]
    #[test]
    fn write_setting_field_aborts_when_backup_fails() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("qq-settings-bakfail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        let original = b"{ broken json ".to_vec();
        std::fs::write(&path, &original).expect("写临时设置文件失败");

        // 目录设为只读：备份 copy 无法在目录里创建新文件（不能预建同名目录，因为备份名带时间戳）
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555))
            .expect("设置目录只读失败");

        let result = write_setting_field(&path, "theme", &"dark");

        // 先恢复权限，保证清理与后续断言可靠
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755))
            .expect("恢复目录权限失败");

        let err = result.expect_err("备份失败必须拒写");
        assert!(
            matches!(err, SettingsError::IoError(_)),
            "备份失败应是 I/O 错误，实际：{err:?}"
        );
        assert_eq!(
            std::fs::read(&path).expect("读回失败"),
            original,
            "备份失败时原文件必须逐字节不变"
        );
        assert!(backup_files(&dir).is_empty(), "备份失败时不该留下半份备份");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 空文件（`touch settings.json`、被截断到 0 字节）按「还没有设置」处理：
    /// 允许写入，不当成损坏拒写，也不留无意义备份。
    #[test]
    fn write_setting_field_accepts_empty_file() {
        let dir = std::env::temp_dir().join(format!("qq-settings-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(&path, "  \n").expect("写临时设置文件失败");

        write_setting_field(&path, "theme", &"dark").expect("空文件应允许写入");

        assert!(backup_files(&dir).is_empty(), "空文件不该留备份");
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("读回失败"))
                .expect("回写内容必须是合法 JSON");
        assert_eq!(written, serde_json::json!({"theme": "dark"}));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 文件不存在时写入：直接创建，不产生无意义的 .bak。
    #[test]
    fn write_setting_field_creates_file_without_backup_when_missing() {
        let dir = std::env::temp_dir().join(format!("qq-settings-new-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");

        write_setting_field(&path, "stay_in_tray", &false).expect("写字段失败");

        assert!(backup_files(&dir).is_empty(), "首次创建不该留备份");
        assert!(!AppSettings::load_or_default(&path).stay_in_tray);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 旧高亮值 `"default"`（本项目自造档，已删除）不做 serde alias：
    /// 读路径按「未知枚举值」回退到默认档 `Bold`。
    ///
    /// 依据：v0.4 尚未发布，不存在需要兼容的存量文件；加 alias 只会让「四档」的语义含糊。
    #[test]
    fn legacy_highlight_default_value_falls_back_to_bold() {
        let (settings, problems, whole) =
            AppSettings::lenient_from_json_str(r#"{"highlight_style":"default"}"#);

        assert!(!whole, "单字段坏值不算整份回退");
        assert_eq!(
            settings.highlight_style,
            HighlightStyle::Bold,
            "旧值 default 应回退到默认档 bold（不再保留自造档）"
        );
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("highlight_style"));
    }

    /// 整份回退的判定：语法坏/根不是对象 → 需要提示；字段级坏值 → 不需要。
    #[test]
    fn whole_file_fallback_flag_only_set_for_unparsable_content() {
        let (settings, problems, whole) = AppSettings::lenient_from_json_str("{ broken ");
        assert!(whole, "语法坏必须标记为整份回退");
        assert_eq!(settings.sort_order, SortOrder::Recent);
        assert_eq!(problems.len(), 1);

        let (_, _, whole_root) = AppSettings::lenient_from_json_str("[]");
        assert!(whole_root, "根不是对象也算整份回退");

        let (settings, problems, whole_field) =
            AppSettings::lenient_from_json_str(r#"{"sort_order":"by_magic","theme":"light"}"#);
        assert!(!whole_field, "只有字段坏时不算整份回退");
        assert_eq!(settings.theme, "light", "好字段照常生效");
        assert_eq!(problems.len(), 1);
    }

    /// 启动期提示只打一次：注入局部标记验证「第一次 true、第二次 false」。
    #[test]
    fn corrupt_settings_warning_is_emitted_only_once() {
        let flag = AtomicBool::new(false);
        let problems = vec!["设置文件 JSON 语法错误".to_string()];

        assert!(
            apply_corrupt_warning(&flag, true, &problems),
            "第一次应打印"
        );
        assert!(
            !apply_corrupt_warning(&flag, true, &problems),
            "第二次必须被去重（读盘每约 500ms 一轮，不能刷屏）"
        );
    }

    /// 完整时序：损坏 → 提示一次 → 修复成功（复位）→ 再损坏 → 再次提示。
    #[test]
    fn corrupt_warning_resets_after_successful_parse() {
        let flag = AtomicBool::new(false);
        let problems = vec!["设置文件 JSON 语法错误".to_string()];

        // 1) 损坏：提示
        assert!(
            apply_corrupt_warning(&flag, true, &problems),
            "首次损坏应提示"
        );
        // 2) 仍损坏：去重
        assert!(
            !apply_corrupt_warning(&flag, true, &problems),
            "同一份损坏不重复"
        );
        // 3) 读成功（哪怕有字段级坏值，whole_file_fallback=false）：复位且不打印
        assert!(
            !apply_corrupt_warning(&flag, false, &[]),
            "解析成功不该打印提示"
        );
        // 4) 再损坏：能重新提示
        assert!(
            apply_corrupt_warning(&flag, true, &problems),
            "修复后再次损坏必须能重新提示，否则同一进程里此后永久静默"
        );
    }

    /// 读盘失败（非「文件不存在」）也只提示一次，且复用同一标记：解析成功后复位再失败可再提示。
    #[test]
    fn read_failure_warning_is_emitted_only_once_and_resets() {
        let flag = AtomicBool::new(false);

        assert!(
            warn_read_failure_once(&flag, "Permission denied (os error 13)"),
            "首次读失败应提示"
        );
        assert!(
            !warn_read_failure_once(&flag, "Permission denied (os error 13)"),
            "同一份读失败不得重复刷屏（读盘每约 500ms 一轮）"
        );

        // 某次真的读到了内容（非整份回退）→ 复位
        assert!(
            !apply_corrupt_warning(&flag, false, &[]),
            "解析成功不该打印提示"
        );
        assert!(
            warn_read_failure_once(&flag, "Permission denied (os error 13)"),
            "复位后再次读失败应能重新提示"
        );
    }

    /// 读盘失败时行为不变：仍回退默认值，只把原因放进问题清单。
    #[cfg(unix)]
    #[test]
    fn load_lenient_reports_io_failure_with_defaults() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("qq-settings-iofail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(&path, r#"{"max_history_items":300}"#).expect("写临时设置文件失败");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
            .expect("设置文件只读失败");

        let (settings, problems) = AppSettings::load_lenient(&path);

        // 恢复权限后再断言/清理
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("恢复文件权限失败");

        assert_eq!(
            settings.max_history_items, DEFAULT_MAX_HISTORY_ITEMS,
            "读失败必须回退默认值（行为不变）"
        );
        assert_eq!(problems.len(), 1, "应报告一条读取失败，实际：{problems:?}");
        assert!(
            problems[0].contains("读取设置文件失败"),
            "问题清单应说明是读取失败，实际：{problems:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 空文件不在读路径触发「整份回退」，也不消耗那唯一一次提示。
    ///
    /// 与写路径（`read_settings_object_for_write` 的空文件例外）保持同一口径。
    #[test]
    fn empty_file_is_not_a_corruption_and_does_not_consume_warning() {
        let (settings, problems, whole) = AppSettings::lenient_from_json_str("");
        assert!(!whole, "空文件不算整份回退");
        assert!(problems.is_empty(), "空文件不该产生问题清单");
        assert_eq!(settings.max_history_items, DEFAULT_MAX_HISTORY_ITEMS);

        let (_, problems_ws, whole_ws) = AppSettings::lenient_from_json_str("  \n\t ");
        assert!(!whole_ws, "只有空白的文件同样不算损坏");
        assert!(problems_ws.is_empty());

        // 空文件读取后，真实损坏仍能提示一次
        let flag = AtomicBool::new(false);
        assert!(
            !apply_corrupt_warning(&flag, whole, &problems),
            "空文件读取不该打印损坏提示"
        );
        assert!(
            apply_corrupt_warning(&flag, true, &["设置文件 JSON 语法错误".to_string()]),
            "空文件不该消耗提示机会：随后真实损坏仍要提示"
        );
    }

    /// `load_lenient` 走完整链路：空文件 → 零问题；随后损坏 → 有问题且整份回退。
    #[test]
    fn load_lenient_treats_empty_file_as_no_settings() {
        let dir =
            std::env::temp_dir().join(format!("qq-settings-empty-read-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("临时目录创建失败");
        let path = dir.join("settings.json");
        std::fs::write(&path, "").expect("写空文件失败");

        let (settings, problems) = AppSettings::load_lenient(&path);
        assert!(problems.is_empty(), "空文件不该有问题，实际：{problems:?}");
        assert_eq!(settings.highlight_style, HighlightStyle::Bold);

        std::fs::write(&path, "{ broken ").expect("写损坏文件失败");
        let (_, problems_broken) = AppSettings::load_lenient(&path);
        assert_eq!(problems_broken.len(), 1, "随后损坏应被报告");

        std::fs::remove_dir_all(&dir).ok();
    }
}
