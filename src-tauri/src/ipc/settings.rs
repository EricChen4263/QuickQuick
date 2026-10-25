//! 设置 IPC 命令层
//!
//! 模式：每个命令 = 薄的 `#[tauri::command]` 包装 + 可单测的纯函数 impl。
//! 单测只测 impl 函数（传显式路径 + fake registrar），命令层把错误映射为 String。
//!
//! 命令清单（前端通过 invoke 对应命令名调用，命名与 A09/S05 前端对齐）：
//! - `get_hotkeys`              — 读 HotkeyConfig，返回 { history, translate, main }
//! - `set_hotkey`               — load → runtime register new → save → unregister old
//! - `get_exclude_list`         — 读 AppSettings.excluded_apps
//! - `set_exclude_list`         — 写 AppSettings.excluded_apps 并 save
//! - `get_translate_providers`  — 返回 registry() 映射的 ProviderDto 列表
//! - `get_selected_provider`    — 读 AppSettings.selected_provider
//! - `set_selected_provider`    — 校验 id 在 registry 内，合法才写入
//! - `get_pause_capture`        — 读 CaptureState.paused AtomicBool
//! - `set_pause_capture`        — 写 AtomicBool + 持久化 pause_capture
//! - `get_skip_sensitive`       — 读 CaptureState.skip_sensitive AtomicBool
//! - `set_skip_sensitive`       — 写 AtomicBool + 持久化 skip_sensitive
//! - `get_stay_in_tray`         — 读 CaptureState.stay_in_tray AtomicBool
//! - `set_stay_in_tray`         — 写 AtomicBool + 持久化 stay_in_tray
//! - `get_auto_update`          — 读 AppSettings.auto_update（纯配置）
//! - `set_auto_update`          — 写 AppSettings.auto_update 并 save
//! - `get_theme`                — 读 AppSettings.theme
//! - `set_theme`                — 校验值∈{auto,light,dark}，合法才写入
//! - `get_image_threshold`      — 读 AppSettings.max_image_bytes
//! - `set_image_threshold`      — 校验范围后写入 max_image_bytes
//! - `get_max_history_items`    — 读 AppSettings.max_history_items（已收敛到 1..=999）
//! - `set_max_history_items`    — 校验 1..=999 → 写入 → 立即按新上限淘汰历史
//! - `get_appearance_settings`  — 一次返回排序方式 / 图片高度 / 首次预览延迟 / 命中高亮样式
//!   （两个数值已收敛到合法区间）
//! - `set_sort_order`           — 校验 ∈{recent, first_copied}，合法才写入
//! - `set_image_height`         — 校验 1..=200 px，合法才写入
//! - `set_preview_first_delay`  — 校验 200..=100000 ms，合法才写入
//! - `set_highlight_style`      — 校验 ∈{color, bold, italic, underline}，合法才写入
//! - `get_launch_on_login`      — 读 autostart.json enabled
//! - `set_launch_on_login`      — 写 autostart.json + 调用 OS autolaunch

use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::RwLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use std::collections::HashMap;

use crate::autostart::AutostartConfig;
use crate::hotkey::{HotkeyAction, HotkeyConfig, HotkeyCorruptInfo, HotkeyError};
use crate::ipc::translate::resolve_provider_or_fallback;
use crate::settings::{
    write_setting_field, AppSettings, HighlightStyle, SortOrder, MAX_HISTORY_ITEMS,
    MAX_IMAGE_HEIGHT_PX, MAX_PREVIEW_FIRST_DELAY_MS, MIN_HISTORY_ITEMS, MIN_IMAGE_HEIGHT_PX,
    MIN_PREVIEW_FIRST_DELAY_MS,
};
use crate::translate::credential::{
    credential_schema, delete_credentials, load_credentials_for_display, save_credentials,
    DbCredStore,
};
use crate::translate::providers::registry;
use crate::CaptureState;

/// 热键配置 DTO（返回给前端）。
///
/// 字段用 camelCase 序列化，与前端 TypeScript 接口对齐。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyDto {
    pub history: String,
    pub translate: String,
    pub main: String,
    /// 配置文件损坏时的修复指引（正常为 null）。
    ///
    /// 读取不再因文件损坏而整体失败：键位照常返回（坏文件用默认键位），
    /// 面板据此在键位行上方展示「已备份到 X，请修复或删除后重启」，而不是整块失效。
    pub corrupt: Option<HotkeyCorruptInfo>,
}

/// 翻译 provider 能力 DTO（返回给前端）。
///
/// 字段用 camelCase 序列化，与前端 TypeScript 接口对齐。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDto {
    pub id: String,
    pub name: String,
    /// 是否需要 API Key（apiKey 语义）。仅表示有无密钥字段，不代表是否可配置。
    pub needs_key: bool,
    /// 是否有可配置字段（凭据 schema 非空，序列化为 camelCase `needsConfig`）。
    ///
    /// 独立于 needs_key：Ollama 本地无鉴权（needs_key=false）但有必填 model 字段，
    /// needs_config=true，前端据此显示「配置」按钮——否则用户无法填必填字段致源不可用。
    pub needs_config: bool,
    /// 是否为非官方/自建接口（序列化为 camelCase `isUnofficial`）。
    /// 前端据此渲染「非官方」标注与失败降级提示（设计文档§三.决策3）。
    pub is_unofficial: bool,
}

/// provider 凭据配置变化事件名。与前端 src/ipc/events.ts 的 PROVIDER_CONFIG_CHANGED_EVENT 必须一致。
/// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
const PROVIDER_CONFIG_CHANGED_EVENT: &str = "provider-config-changed";

/// 默认翻译源切换事件名。与前端 src/ipc/events.ts 的 SELECTED_PROVIDER_CHANGED_EVENT 必须一致。
/// 设置页与翻译页各自缓存当前默认 provider，一方改动后 emit 此事件令另一方刷新；
/// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
const SELECTED_PROVIDER_CHANGED_EVENT: &str = "selected-provider-changed";

/// 将 `action` 字符串解析为 `HotkeyAction`。
///
/// 只接受 "history" / "translate" / "main" 三个合法值，其余返回 Err。
/// 集中在此处做边界校验，impl 函数不再重复处理。
fn parse_action(action: &str) -> Result<HotkeyAction, String> {
    match action {
        "history" => Ok(HotkeyAction::History),
        "translate" => Ok(HotkeyAction::Translate),
        "main" => Ok(HotkeyAction::Main),
        other => Err(format!(
            "未知 action：{other}，合法值为 history / translate / main"
        )),
    }
}

/// `get_hotkeys` 的纯函数实现，可在测试中直接调用。
///
/// **逐键容错读**（与写路径口径一致）：坏字段只回退该字段的默认值；文件整体损坏时
/// 仍返回默认键位，并在 `corrupt` 里给出修复指引（含保护性备份路径）。
/// 面板因此不会整块失效——用户至少能看到当前实际生效的键位，以及该去哪儿修。
///
/// # Errors
/// 本实现不返回 Err：损坏信息走 `corrupt` 字段（这也是「去掉『稍后重试』误导文案」的前提）。
pub fn get_hotkeys_impl(hotkey_path: &Path) -> Result<HotkeyDto, String> {
    let outcome = HotkeyConfig::load_lenient(hotkey_path);
    Ok(HotkeyDto {
        history: outcome
            .config
            .get_accelerator(HotkeyAction::History)
            .to_string(),
        translate: outcome
            .config
            .get_accelerator(HotkeyAction::Translate)
            .to_string(),
        main: outcome
            .config
            .get_accelerator(HotkeyAction::Main)
            .to_string(),
        corrupt: outcome.corrupt,
    })
}

/// 命令层运行时热键注册器：封装真实 Tauri global shortcut 和测试 fake。
pub trait RuntimeHotkeyRegistrar {
    /// 注册指定动作的新热键，并绑定动作对应的真实回调。
    ///
    /// # Errors
    /// 当 OS 或本应用已有注册占用该热键时返回错误字符串。
    fn register_action_shortcut(
        &self,
        action: HotkeyAction,
        accelerator: &str,
    ) -> Result<(), String>;

    /// 注销指定热键。调用方会忽略注销失败以避免丢失已保存的新键。
    ///
    /// # Errors
    /// 当底层 global shortcut API 注销失败时返回错误字符串。
    fn unregister(&self, accelerator: &str) -> Result<(), String>;
}

/// `set_hotkey` 的运行时安全实现，可在测试中直接注入 fake runtime。
///
/// 顺序刻意保持为：读旧键 → old==new no-op → 注册新键 → 保存配置 → 注销旧键。
/// 这样新键被其他应用占用时，失败发生在保存和注销之前，旧配置与旧热键都不会丢。
/// 保存失败时会尽量注销刚注册的新键，避免运行时留下未持久化的临时绑定。
///
/// # Errors
/// - 新键运行时注册失败：配置不变，旧键不注销
/// - 文件读写失败：返回对应错误字符串
pub fn set_hotkey_runtime_impl(
    action: HotkeyAction,
    accelerator: &str,
    hotkey_path: &Path,
    runtime: &dyn RuntimeHotkeyRegistrar,
) -> Result<(), String> {
    // 读用容错版（与 get_hotkeys 同一函数）：hotkey.json 损坏时按默认键位继续，
    // 让写路径去做「备份 + 拒写」，用户才能拿到带修复指引的错误。
    let mut config = HotkeyConfig::load_lenient(hotkey_path).config;

    let old_accelerator = config.get_accelerator(action).to_string();
    if old_accelerator == accelerator {
        return Ok(());
    }

    runtime.register_action_shortcut(action, accelerator)?;

    config.set_accelerator(action, accelerator);
    // 逐键落盘：只改这个动作的键，其余动作与未知键原样保留；
    // hotkey.json 语法坏时先备份再拒写（见 settings::update_json_object）
    if let Err(e) = HotkeyConfig::save_accelerator(hotkey_path, action, accelerator) {
        let _ = runtime.unregister(accelerator);
        return Err(e.to_string());
    }

    let _ = runtime.unregister(&old_accelerator);
    Ok(())
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
/// 读取现有设置（保留其他字段），仅替换 excluded_apps 后 save。
/// 写文件成功后重建运行时规则集，使轮询线程在下一次迭代即刻生效，无需重启。
///
/// # Errors
/// 文件写入失败时返回错误字符串（写入失败则不更新运行时，保持一致性）。
pub fn set_exclude_list_impl(
    list: Vec<String>,
    settings_path: &Path,
    runtime_rules: Option<&RwLock<crate::privacy::IgnoreRules>>,
) -> Result<(), String> {
    write_setting_field(settings_path, "excluded_apps", &list).map_err(|e| e.to_string())?;
    rebuild_runtime_rules(runtime_rules, &AppSettings::load_or_default(settings_path));
    Ok(())
}

/// `get_exclude_mode` 的纯函数实现：返回应用名单模式（黑名单 / 白名单）。
///
/// # Errors
/// 此函数实际上永不返回 `Err`（`load_or_default` 静默回退默认值），
/// 签名保留 `Result` 以与命令层 `?` 传播保持一致。
pub fn get_exclude_mode_impl(settings_path: &Path) -> Result<crate::privacy::ExcludeMode, String> {
    Ok(AppSettings::load_or_default(settings_path).exclude_mode)
}

/// `set_exclude_mode_impl`：切换名单方向，**保留已填的应用条目与内容正则**。
///
/// 切换模式只改方向，不动条目——用户从黑名单换到白名单时，
/// 刚填的那批应用名正好就是他想要的「只记录这些」，清空反而是破坏。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_exclude_mode_impl(
    mode: crate::privacy::ExcludeMode,
    settings_path: &Path,
    runtime_rules: Option<&RwLock<crate::privacy::IgnoreRules>>,
) -> Result<(), String> {
    write_setting_field(settings_path, "exclude_mode", &mode).map_err(|e| e.to_string())?;
    rebuild_runtime_rules(runtime_rules, &AppSettings::load_or_default(settings_path));
    Ok(())
}

/// `get_ignore_regexes_impl`：返回用户配置的内容忽略正则原文（未编译）。
///
/// 返回原文而非编译结果：前端要能原样编辑，编译失败的那条也要能被用户看到并修正。
///
/// # Errors
/// 此函数实际上永不返回 `Err`（`load_or_default` 静默回退默认值）。
pub fn get_ignore_regexes_impl(settings_path: &Path) -> Result<Vec<String>, String> {
    Ok(AppSettings::load_or_default(settings_path).ignore_regexes)
}

/// `set_ignore_regexes_impl`：整体覆盖内容忽略正则，并重建运行时规则集。
///
/// 非法正则**不阻塞写入**：原文照存，运行时编译时丢弃该条并留痕。
/// 若在这里拒收，用户存了一半的规则会在下次打开设置页时消失，反而更难修。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_ignore_regexes_impl(
    patterns: Vec<String>,
    settings_path: &Path,
    runtime_rules: Option<&RwLock<crate::privacy::IgnoreRules>>,
) -> Result<(), String> {
    write_setting_field(settings_path, "ignore_regexes", &patterns).map_err(|e| e.to_string())?;
    rebuild_runtime_rules(runtime_rules, &AppSettings::load_or_default(settings_path));
    Ok(())
}

/// 把设置里的忽略规则整体重建进运行时规则集。
///
/// 三个设置入口（应用名单 / 名单模式 / 内容正则）共用这一条重建路径，任何一处写入后
/// 规则集都从**完整设置**重新构造——避免「改模式时把正则丢了」这类字段互相覆盖。
/// 正则的编译也只发生在这里，轮询路径只读不编译。
fn rebuild_runtime_rules(
    lock: Option<&RwLock<crate::privacy::IgnoreRules>>,
    settings: &AppSettings,
) {
    let Some(lock) = lock else {
        return;
    };
    let rules = crate::privacy::IgnoreRules::with_rules(
        &settings.excluded_apps,
        settings.exclude_mode,
        &settings.ignore_regexes,
    );
    match lock.write() {
        Ok(mut guard) => *guard = rules,
        Err(e) => eprintln!("[QuickQuick] 忽略规则写锁中毒，跳过运行时更新: {e}"),
    }
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
            // 可配置 = 凭据 schema 非空（含 Ollama 等无 key 但有必填字段的源）。
            needs_config: !credential_schema(cap.id).is_empty(),
            is_unofficial: cap.is_unofficial,
        })
        .collect()
}

/// `get_selected_provider` 的纯函数实现，可在测试中直接调用。
///
/// 从 settings 文件读取当前选中的 provider id；文件不存在时返回默认值（lingva）。
/// 设置迁移（设计文档§六）：若持久化的 id 不在当前注册表内（含旧值 mymemory、
/// 任意已删除源），回退默认源并在写路径持久化修正，使后续读取稳定。
///
/// # Errors
/// 读取永不失败（`load_or_default` 静默回退默认值）；但写路径持久化修正失败时返回 Err。
pub fn get_selected_provider_impl(settings_path: &Path) -> Result<String, String> {
    let settings = AppSettings::load_or_default(settings_path);
    let resolved = resolve_provider_or_fallback(&settings.selected_provider);

    // 仅当发生回退（存储值非法）时才写回，避免每次读取都触发磁盘写。
    if resolved != settings.selected_provider {
        write_setting_field(settings_path, "selected_provider", &resolved)
            .map_err(|e| e.to_string())?;
    }

    Ok(resolved)
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

    write_setting_field(settings_path, "selected_provider", &id).map_err(|e| e.to_string())
}

/// 从 AppHandle 解析配置目录下的指定文件路径。
///
/// 若无法取得配置目录则返回 Err；目录不存在时尝试创建。
/// 此函数是不可单测的胶水层，仅供命令函数调用。
pub(crate) fn resolve_config_path(
    app: &AppHandle,
    filename: &str,
) -> Result<std::path::PathBuf, String> {
    Ok(resolve_config_dir(app)?.join(filename))
}

/// 返回 app_config_dir（必要时创建），用于构造 dev 文件密钥库等需要配置目录的对象。
///
/// 与 `resolve_config_path` 共享同一目录解析逻辑（后者在其基础上 join 文件名）。
/// 此函数是不可单测的胶水层，仅供命令函数调用。
pub(crate) fn resolve_config_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录：{e}"))?;

    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建配置目录：{e}"))?;

    Ok(dir)
}

/// 判断底层注册错误是否属于「热键已被占用」这一类。
///
/// 判据用两条**上游常量级**的英文前缀，均不随系统语言变化：
/// - `HotKey already registered`：底层注册器对「同一个键已被注册」给出的固定前缀
///   （Linux / Windows 的重复注册，以及 macOS 媒体键重复注册）。
/// - `RegisterEventHotKey failed for`：macOS 上 Carbon 注册接口返回非 `noErr`。
///   组合键被其它应用占用时 Carbon 返回 `eventHotKeyExistsErr`，即本项目最常遇到的
///   「被别的 App 抢走了」场景。
fn is_hotkey_conflict_error(error: &str) -> bool {
    const CONFLICT_SIGNATURES: [&str; 2] = [
        "HotKey already registered",
        "RegisterEventHotKey failed for",
    ];
    CONFLICT_SIGNATURES
        .iter()
        .any(|signature| error.contains(signature))
}

/// 把运行时注册失败的底层错误映射成**用户可见**文案。
///
/// 为什么只能按文案识别冲突：全局快捷键插件在错误边界就把底层的结构化错误折叠成
/// `GlobalHotkey(String)`（其 Display 即原文），到调用侧时「已被注册」与「注册动作本身
/// 失败」的区分已经丢了，只剩文本可判。
///
/// 冲突命中时返回 `HotkeyError::AlreadyInUse` 的中文文案（中文 UI 不应漏出英文原文）；
/// 其它错误保持原映射不变，英文原文仍随返回串带给用户。
fn map_runtime_register_error(error: &str) -> String {
    if is_hotkey_conflict_error(error) {
        // 英文原文被替换掉了，必须留痕：否则现场无法分辨是「被别的 App 占用」还是别的注册失败。
        eprintln!("[QuickQuick] 热键注册被占用，已折叠为中文文案。原始错误: {error}");
        return HotkeyError::AlreadyInUse.to_string();
    }
    format!("热键运行时注册失败: {error}")
}

/// Tauri 运行时热键注册器：通过 global shortcut API 注册热键并绑定回调。
struct TauriRuntimeHotkeyRegistrar<'a> {
    app: &'a AppHandle,
}

impl RuntimeHotkeyRegistrar for TauriRuntimeHotkeyRegistrar<'_> {
    fn register_action_shortcut(
        &self,
        action: HotkeyAction,
        accelerator: &str,
    ) -> Result<(), String> {
        crate::register_action_shortcut(self.app, action, accelerator)
            .map_err(|e| map_runtime_register_error(&e.to_string()))
    }

    fn unregister(&self, accelerator: &str) -> Result<(), String> {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        self.app
            .global_shortcut()
            .unregister(accelerator)
            .map_err(|e| format!("热键运行时注销失败: {e}"))
    }
}

/// Tauri 命令：读取热键配置，返回 { history, translate, main }。
#[tauri::command]
pub fn get_hotkeys(app: AppHandle) -> Result<HotkeyDto, String> {
    let path = resolve_config_path(&app, "hotkey.json")?;
    get_hotkeys_impl(&path)
}

/// Tauri 命令：将指定动作改绑到新加速键（含冲突检测 + 运行时即时应用）。
///
/// 流程：读出旧键 → 注册新键并绑回调 → 持久化新键 → 注销旧键。
/// 运行时注册失败时直接返回 Err，配置不写、旧键不卸，避免外部占用导致热键丢失。
#[tauri::command]
pub fn set_hotkey(app: AppHandle, action: String, accelerator: String) -> Result<(), String> {
    let hotkey_action = parse_action(&action)?;
    let path = resolve_config_path(&app, "hotkey.json")?;
    let runtime = TauriRuntimeHotkeyRegistrar { app: &app };
    set_hotkey_runtime_impl(hotkey_action, &accelerator, &path, &runtime)
}

/// Tauri 命令：读取排除名单。
#[tauri::command]
pub fn get_exclude_list(app: AppHandle) -> Result<Vec<String>, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_exclude_list_impl(&path)
}

/// Tauri 命令：写入排除名单（持久化 + 运行时即时生效）。
#[tauri::command]
pub fn set_exclude_list(
    app: AppHandle,
    state: State<'_, CaptureState>,
    list: Vec<String>,
) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_exclude_list_impl(list, &path, Some(&state.exclude_list))
}

/// Tauri 命令：读取应用名单模式（blacklist / whitelist）。
#[tauri::command]
pub fn get_exclude_mode(app: AppHandle) -> Result<crate::privacy::ExcludeMode, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_exclude_mode_impl(&path)
}

/// Tauri 命令：切换应用名单模式（持久化 + 运行时即时生效，保留已填条目）。
#[tauri::command]
pub fn set_exclude_mode(
    app: AppHandle,
    state: State<'_, CaptureState>,
    mode: crate::privacy::ExcludeMode,
) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_exclude_mode_impl(mode, &path, Some(&state.exclude_list))
}

/// Tauri 命令：读取内容忽略正则原文。
#[tauri::command]
pub fn get_ignore_regexes(app: AppHandle) -> Result<Vec<String>, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_ignore_regexes_impl(&path)
}

/// Tauri 命令：整体覆盖内容忽略正则（持久化 + 运行时即时生效）。
#[tauri::command]
pub fn set_ignore_regexes(
    app: AppHandle,
    state: State<'_, CaptureState>,
    patterns: Vec<String>,
) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_ignore_regexes_impl(patterns, &path, Some(&state.exclude_list))
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
///
/// 写入成功后 emit `selected-provider-changed`，让设置页与翻译页双向同步当前默认源
/// （任一页改动，另一页据事件重读刷新）。emit 失败仅记日志，不影响命令返回值。
#[tauri::command]
pub fn set_selected_provider(app: AppHandle, id: String) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_selected_provider_impl(&id, &path)?;
    if let Err(e) = app.emit(SELECTED_PROVIDER_CHANGED_EVENT, ()) {
        eprintln!("[QuickQuick] selected-provider-changed emit 失败: {e}");
    }
    Ok(())
}

/// `get_pause_capture` 的纯函数实现：从 AtomicBool 读取当前暂停状态。
pub fn get_pause_capture_impl(state: &CaptureState) -> bool {
    state.paused.load(Ordering::Relaxed)
}

/// `set_pause_capture` 的纯函数实现：写 AtomicBool 并持久化到 settings.json。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_pause_capture_impl(
    value: bool,
    state: &CaptureState,
    settings_path: &Path,
) -> Result<(), String> {
    state.paused.store(value, Ordering::Relaxed);
    write_setting_field(settings_path, "pause_capture", &value).map_err(|e| e.to_string())
}

/// `get_skip_sensitive` 的纯函数实现：从 AtomicBool 读取当前敏感跳过状态。
pub fn get_skip_sensitive_impl(state: &CaptureState) -> bool {
    state.skip_sensitive.load(Ordering::Relaxed)
}

/// `set_skip_sensitive` 的纯函数实现：写 AtomicBool 并持久化到 settings.json。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_skip_sensitive_impl(
    value: bool,
    state: &CaptureState,
    settings_path: &Path,
) -> Result<(), String> {
    state.skip_sensitive.store(value, Ordering::Relaxed);
    write_setting_field(settings_path, "skip_sensitive", &value).map_err(|e| e.to_string())
}

/// `get_stay_in_tray` 的纯函数实现：从 AtomicBool 读取当前托盘驻留状态。
pub fn get_stay_in_tray_impl(state: &CaptureState) -> bool {
    state.stay_in_tray.load(Ordering::Relaxed)
}

/// `set_stay_in_tray` 的纯函数实现：写 AtomicBool 并持久化到 settings.json。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_stay_in_tray_impl(
    value: bool,
    state: &CaptureState,
    settings_path: &Path,
) -> Result<(), String> {
    state.stay_in_tray.store(value, Ordering::Relaxed);
    write_setting_field(settings_path, "stay_in_tray", &value).map_err(|e| e.to_string())
}

/// `get_auto_update` 的纯函数实现：从 settings.json 读取 auto_update 值。
///
/// # Errors
/// 此函数实际上永不返回 Err（load_or_default 回退默认值）。
/// 签名保留 Result 与命令层保持一致。
pub fn get_auto_update_impl(settings_path: &Path) -> Result<bool, String> {
    let settings = AppSettings::load_or_default(settings_path);
    Ok(settings.auto_update)
}

/// `set_auto_update` 的纯函数实现：写 settings.json auto_update 字段。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_auto_update_impl(value: bool, settings_path: &Path) -> Result<(), String> {
    write_setting_field(settings_path, "auto_update", &value).map_err(|e| e.to_string())
}

/// `get_theme` 的纯函数实现：从 settings.json 读取 theme 值。
///
/// # Errors
/// 此函数实际上永不返回 Err（load_or_default 回退默认值）。
pub fn get_theme_impl(settings_path: &Path) -> Result<String, String> {
    let settings = AppSettings::load_or_default(settings_path);
    Ok(settings.theme)
}

/// `set_theme` 的纯函数实现：校验值合法后写入 settings.json。
///
/// 合法值：`auto` / `light` / `dark`；其余返回 Err，不触发文件写。
///
/// # Errors
/// - 非法 theme 值：返回包含合法值说明的错误字符串
/// - 文件写入失败：返回错误字符串
pub fn set_theme_impl(theme: &str, settings_path: &Path) -> Result<(), String> {
    if !matches!(theme, "auto" | "light" | "dark") {
        return Err(format!(
            "非法 theme 值：{theme}，合法值为 auto / light / dark"
        ));
    }
    write_setting_field(settings_path, "theme", &theme).map_err(|e| e.to_string())
}

/// 单张图片原图阈值合法区间下限（1 MiB）。
const MIN_IMAGE_THRESHOLD: u64 = 1024 * 1024;
/// 单张图片原图阈值合法区间上限（500 MiB）。
const MAX_IMAGE_THRESHOLD: u64 = 500 * 1024 * 1024;

/// `get_image_threshold` 的纯函数实现：从 settings.json 读取 max_image_bytes。
///
/// # Errors
/// 此函数实际上永不返回 `Err`：`AppSettings::load_or_default` 在文件不存在或
/// 内容损坏时均静默回退默认值，调用方可安全地 `unwrap_or_default`。
/// 签名保留 `Result` 以与命令层 `?` 传播保持一致，便于未来升级为真正可失败路径。
pub fn get_image_threshold_impl(settings_path: &Path) -> Result<u64, String> {
    let settings = AppSettings::load_or_default(settings_path);
    Ok(settings.max_image_bytes)
}

/// `set_image_threshold` 的纯函数实现：校验范围后写入 settings.json。
///
/// 合法区间：`1MiB ..= 500MiB`；越界返回中文 Err，不触发文件写。
///
/// # Errors
/// - 越界值：返回说明合法范围的错误字符串
/// - 文件写入失败：返回错误字符串
pub fn set_image_threshold_impl(bytes: u64, settings_path: &Path) -> Result<(), String> {
    if !(MIN_IMAGE_THRESHOLD..=MAX_IMAGE_THRESHOLD).contains(&bytes) {
        return Err(format!(
            "图片阈值超出范围：{bytes} 字节，合法范围为 {MIN_IMAGE_THRESHOLD}（1MiB）到 {MAX_IMAGE_THRESHOLD}（500MiB）字节"
        ));
    }
    write_setting_field(settings_path, "max_image_bytes", &bytes).map_err(|e| e.to_string())
}

/// `get_max_history_items` 的纯函数实现：从 settings.json 读取历史条目上限。
///
/// 返回值已按 `1..=999` 收敛（见 [`AppSettings::effective_max_history_items`]）：
/// 前端把读到的值回填到输入框，若把手工写坏的 0 或 10000 原样透出，
/// 用户会看到一个自己没设过、也输不进去的值。
///
/// # Errors
/// 此函数实际上永不返回 `Err`（`load_or_default` 静默回退默认值）。
/// 签名保留 `Result` 以与命令层 `?` 传播保持一致。
pub fn get_max_history_items_impl(settings_path: &Path) -> Result<usize, String> {
    Ok(AppSettings::load_or_default(settings_path).effective_max_history_items())
}

/// `set_max_history_items` 的纯函数实现：校验 → 持久化 → 立即按新上限收敛。
///
/// 合法区间 `1..=999`（对齐 Maccy 的 1–999 语义）；越界返回中文 Err 且**不落盘**，
/// 避免把非法值写进设置文件后每次开库都要靠读侧 clamp 兜底。
///
/// 为什么写完要立刻裁剪而不是等下一次复制：上限调小后历史可能立刻超限，
/// 若只等下一次捕获才收敛，用户会看到「上限已经写 50 了、列表还有 400 条」。
/// 裁剪失败只留痕不报错——设置已经落盘，下一次新增条目还会再收敛一遍；
/// 因为一次裁剪失败把用户的设置回滚掉，代价更大。
///
/// 返回本轮被淘汰的条数（`conn` 为 `None` 时恒为 0，用于设置与裁剪解耦的场景）。
///
/// # Errors
/// - 越界值：返回说明合法范围的错误字符串（不写文件）
/// - 文件写入失败：返回错误字符串
pub fn set_max_history_items_impl(
    items: usize,
    settings_path: &Path,
    conn: Option<&rusqlite::Connection>,
) -> Result<usize, String> {
    if !(MIN_HISTORY_ITEMS..=MAX_HISTORY_ITEMS).contains(&items) {
        return Err(format!(
            "历史条目上限超出范围：{items}，合法范围为 {MIN_HISTORY_ITEMS} 到 {MAX_HISTORY_ITEMS} 条"
        ));
    }

    write_setting_field(settings_path, "max_history_items", &items).map_err(|e| e.to_string())?;

    let Some(conn) = conn else {
        return Ok(0);
    };
    match crate::db::evict_over_limit(conn, items) {
        Ok(evicted) => Ok(evicted),
        Err(e) => {
            eprintln!("[QuickQuick] 按新上限淘汰历史失败（非致命，下次入库会再收敛）：{e}");
            Ok(0)
        }
    }
}

/// 外观设置 DTO（返回给前端）。
///
/// 四个字段一次性返回：弹窗每次打开都要读其中三个，分四次 IPC 只会让打开变慢。
/// 枚举以 snake_case 字符串下发（`recent` / `first_copied` /
/// `color` / `bold` / `italic` / `underline`）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettingsDto {
    /// 列表排序方式
    pub sort_order: SortOrder,
    /// 弹出面板图片缩略图高度（px，已收敛到 16..=48）
    pub image_height_px: u32,
    /// 首次预览延迟（毫秒，已收敛到 200..=100000）
    pub preview_first_delay_ms: u32,
    /// 搜索命中高亮样式
    pub highlight_style: HighlightStyle,
}

/// `get_appearance_settings` 的纯函数实现：一次读盘返回四个外观设置（数值已收敛）。
///
/// # Errors
/// 此函数实际上永不返回 `Err`（`load_or_default` 静默回退默认值）。
/// 签名保留 `Result` 以与命令层 `?` 传播保持一致。
pub fn get_appearance_settings_impl(settings_path: &Path) -> Result<AppearanceSettingsDto, String> {
    let settings = AppSettings::load_or_default(settings_path);
    Ok(AppearanceSettingsDto {
        sort_order: settings.sort_order,
        image_height_px: settings.effective_image_height_px(),
        preview_first_delay_ms: settings.effective_preview_first_delay_ms(),
        highlight_style: settings.highlight_style,
    })
}

/// 从 settings.json 读列表排序方式；读取或解析失败时回退默认（最近复制）。
///
/// 供剪贴板列表命令使用：排序是展示偏好，读不到就按默认顺序出列表。
pub fn read_sort_order_from_path(settings_path: &Path) -> SortOrder {
    AppSettings::load_or_default(settings_path).sort_order
}

/// 把前端传来的排序方式字符串解析成枚举。
///
/// 只认 `recent` / `first_copied`：命令参数是字符串而不是枚举，是为了在非法值时
/// 返回中文说明，而不是 serde 那句「unknown variant」。
fn parse_sort_order(raw: &str) -> Result<SortOrder, String> {
    match raw {
        "recent" => Ok(SortOrder::Recent),
        "first_copied" => Ok(SortOrder::FirstCopied),
        other => Err(format!(
            "非法排序方式：{other}，合法值为 recent / first_copied"
        )),
    }
}

/// 把前端传来的高亮样式字符串解析成枚举（四档，与 Maccy 的 `HighlightMatch` 同名）。
fn parse_highlight_style(raw: &str) -> Result<HighlightStyle, String> {
    match raw {
        "color" => Ok(HighlightStyle::Color),
        "bold" => Ok(HighlightStyle::Bold),
        "italic" => Ok(HighlightStyle::Italic),
        "underline" => Ok(HighlightStyle::Underline),
        other => Err(format!(
            "非法命中高亮样式：{other}，合法值为 color / bold / italic / underline"
        )),
    }
}

/// `set_sort_order` 的纯函数实现：校验后写入 settings.json。
///
/// # Errors
/// - 非法取值：返回说明合法值的错误字符串（不写文件）
/// - 文件写入失败：返回错误字符串
pub fn set_sort_order_impl(order: &str, settings_path: &Path) -> Result<(), String> {
    let parsed = parse_sort_order(order)?;
    write_setting_field(settings_path, "sort_order", &parsed).map_err(|e| e.to_string())
}

/// `set_image_height` 的纯函数实现：校验范围后写入 settings.json。
///
/// 合法区间 `MIN_IMAGE_HEIGHT_PX..=MAX_IMAGE_HEIGHT_PX`（1..=200 px，对齐 Maccy）。
///
/// # Errors
/// - 越界值：返回说明合法范围的错误字符串（不写文件）
/// - 文件写入失败：返回错误字符串
pub fn set_image_height_impl(px: u32, settings_path: &Path) -> Result<(), String> {
    if !(MIN_IMAGE_HEIGHT_PX..=MAX_IMAGE_HEIGHT_PX).contains(&px) {
        return Err(format!(
            "图片高度超出范围：{px}px，合法范围为 {MIN_IMAGE_HEIGHT_PX} 到 {MAX_IMAGE_HEIGHT_PX}px"
        ));
    }
    write_setting_field(settings_path, "image_height_px", &px).map_err(|e| e.to_string())
}

/// `set_preview_first_delay` 的纯函数实现：校验范围后写入 settings.json。
///
/// 合法区间 `MIN_PREVIEW_FIRST_DELAY_MS..=MAX_PREVIEW_FIRST_DELAY_MS`（200..=100000 ms，对齐 Maccy）。
///
/// # Errors
/// - 越界值：返回说明合法范围的错误字符串（不写文件）
/// - 文件写入失败：返回错误字符串
pub fn set_preview_first_delay_impl(ms: u32, settings_path: &Path) -> Result<(), String> {
    if !(MIN_PREVIEW_FIRST_DELAY_MS..=MAX_PREVIEW_FIRST_DELAY_MS).contains(&ms) {
        return Err(format!(
            "首次预览延迟超出范围：{ms}ms，合法范围为 {MIN_PREVIEW_FIRST_DELAY_MS} 到 {MAX_PREVIEW_FIRST_DELAY_MS}ms"
        ));
    }
    write_setting_field(settings_path, "preview_first_delay_ms", &ms).map_err(|e| e.to_string())
}

/// `set_highlight_style` 的纯函数实现：校验后写入 settings.json。
///
/// # Errors
/// - 非法取值：返回说明合法值的错误字符串（不写文件）
/// - 文件写入失败：返回错误字符串
pub fn set_highlight_style_impl(style: &str, settings_path: &Path) -> Result<(), String> {
    let parsed = parse_highlight_style(style)?;
    write_setting_field(settings_path, "highlight_style", &parsed).map_err(|e| e.to_string())
}

/// `get_launch_on_login` 的纯函数实现：从 autostart.json 读取 enabled 值。
///
/// # Errors
/// 此函数实际上永不返回 Err（load_or_default 回退默认值）。
pub fn get_launch_on_login_impl(autostart_path: &Path) -> Result<bool, String> {
    let config = AutostartConfig::load_or_default(autostart_path);
    Ok(config.enabled)
}

/// `set_launch_on_login` 的纯函数实现（持久化部分）：写 autostart.json。
///
/// OS autolaunch 的 enable/disable 由命令层在写入后调用 `autostart::apply_to_os`
/// 完成，纯函数只负责持久化，保持可单测。
///
/// # Errors
/// 文件写入失败时返回错误字符串。
pub fn set_launch_on_login_impl(value: bool, autostart_path: &Path) -> Result<(), String> {
    // 逐键落盘：只改 enabled，未知键原样保留；语法坏的文件先备份再拒写
    AutostartConfig::save_enabled(autostart_path, value).map_err(|e| e.to_string())
}

/// Tauri 命令：读取当前暂停捕获状态。
#[tauri::command]
pub fn get_pause_capture(state: State<'_, CaptureState>) -> bool {
    get_pause_capture_impl(&state)
}

/// Tauri 命令：设置暂停捕获状态（运行时生效 + 持久化 + 刷新托盘可见状态）。
///
/// 设置页与托盘 `⌥` 点击改的是同一个状态，托盘那侧也要立刻反映出来，
/// 否则从设置页暂停后菜单仍写着「暂停捕获」。
#[tauri::command]
pub fn set_pause_capture(
    app: AppHandle,
    state: State<'_, CaptureState>,
    value: bool,
) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_pause_capture_impl(value, &state, &path)?;
    crate::tray::refresh_tray_indicator(&app, value, state.skip_once.is_armed());
    Ok(())
}

/// Tauri 命令：读取当前敏感内容跳过状态。
#[tauri::command]
pub fn get_skip_sensitive(state: State<'_, CaptureState>) -> bool {
    get_skip_sensitive_impl(&state)
}

/// Tauri 命令：设置敏感内容跳过状态（运行时生效 + 持久化）。
#[tauri::command]
pub fn set_skip_sensitive(
    app: AppHandle,
    state: State<'_, CaptureState>,
    value: bool,
) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_skip_sensitive_impl(value, &state, &path)
}

/// Tauri 命令：读取当前托盘驻留状态。
#[tauri::command]
pub fn get_stay_in_tray(state: State<'_, CaptureState>) -> bool {
    get_stay_in_tray_impl(&state)
}

/// Tauri 命令：设置托盘驻留状态（运行时生效 + 持久化）。
#[tauri::command]
pub fn set_stay_in_tray(
    app: AppHandle,
    state: State<'_, CaptureState>,
    value: bool,
) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_stay_in_tray_impl(value, &state, &path)
}

/// Tauri 命令：读取自动更新开关。
#[tauri::command]
pub fn get_auto_update(app: AppHandle) -> Result<bool, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_auto_update_impl(&path)
}

/// Tauri 命令：设置自动更新开关。
#[tauri::command]
pub fn set_auto_update(app: AppHandle, value: bool) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_auto_update_impl(value, &path)
}

/// Tauri 命令：读取当前 UI 主题。
#[tauri::command]
pub fn get_theme(app: AppHandle) -> Result<String, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_theme_impl(&path)
}

/// Tauri 命令：设置 UI 主题（校验值合法性：auto / light / dark）。
#[tauri::command]
pub fn set_theme(app: AppHandle, theme: String) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_theme_impl(&theme, &path)
}

/// Tauri 命令：读取单张图片原图阈值（字节）。
#[tauri::command]
pub fn get_image_threshold(app: AppHandle) -> Result<u64, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_image_threshold_impl(&path)
}

/// Tauri 命令：设置单张图片原图阈值（字节，合法范围 1MiB–500MiB）。
#[tauri::command]
pub fn set_image_threshold(app: AppHandle, bytes: u64) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_image_threshold_impl(bytes, &path)
}

/// Tauri 命令：读取历史条目上限（1–999 条，已收敛）。
#[tauri::command]
pub fn get_max_history_items(app: AppHandle) -> Result<usize, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_max_history_items_impl(&path)
}

/// Tauri 命令：设置历史条目上限并立即按新上限淘汰（返回本轮淘汰条数）。
///
/// 裁剪成功且真的删了条目时发 `clipboard-changed`：列表页常驻挂载、只按该事件刷新，
/// 不发的话用户会看到「设置页说已清理、列表还是旧数据」。emit 失败只记日志，不影响返回值。
#[tauri::command]
pub fn set_max_history_items(
    app: AppHandle,
    state: State<'_, super::AppDb>,
    items: usize,
) -> Result<usize, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    // 连接可能因启动期开库失败而为 None：那时设置照常落盘，只是没法立刻裁剪。
    let guard = state
        .conn
        .lock()
        .map_err(|e| format!("数据库锁获取失败：{e}"))?;
    let evicted = set_max_history_items_impl(items, &path, guard.as_ref())?;
    drop(guard);

    if evicted > 0 {
        if let Err(e) = app.emit(crate::CLIPBOARD_CHANGED_EVENT, ()) {
            eprintln!("[QuickQuick] 历史上限收敛后发送 clip-change 事件失败: {e}");
        }
    }
    Ok(evicted)
}

/// Tauri 命令：读取外观设置（排序方式 / 图片高度 / 首次预览延迟 / 命中高亮样式）。
#[tauri::command]
pub fn get_appearance_settings(app: AppHandle) -> Result<AppearanceSettingsDto, String> {
    let path = resolve_config_path(&app, "settings.json")?;
    get_appearance_settings_impl(&path)
}

/// Tauri 命令：设置列表排序方式（`recent` / `first_copied`）。
///
/// 落盘后发 `clipboard-changed`：列表的**顺序**变了，前端只按该事件重新取数。
/// 不发的话用户会看到「设置里选了首次复制、列表还是旧顺序」，直到下一次复制。
#[tauri::command]
pub fn set_sort_order(app: AppHandle, order: String) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_sort_order_impl(&order, &path)?;
    notify_list_needs_refresh(&app);
    Ok(())
}

/// Tauri 命令：设置弹出面板图片缩略图高度（px，1–200，对齐 Maccy 的 imageMaxHeight）。
///
/// 不发刷新事件：高度是纯 CSS 尺寸，列表数据与顺序都没变，重取一次只会白传 payload。
#[tauri::command]
pub fn set_image_height(app: AppHandle, px: u32) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_image_height_impl(px, &path)
}

/// Tauri 命令：设置首次预览延迟（毫秒，200–100000，对齐 Maccy）。
///
/// 不发刷新事件：它只影响面板打开后的等待时长，下一次开面板自然读到新值。
#[tauri::command]
pub fn set_preview_first_delay(app: AppHandle, ms: u32) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_preview_first_delay_impl(ms, &path)
}

/// Tauri 命令：设置搜索命中高亮样式（`default` / `underline` / `bold`）。
///
/// 落盘后发 `clipboard-changed`：主窗列表与弹窗都常驻挂载，只按该事件重新取数，
/// 不发的话新样式要等重启才生效——那等于设置页在骗人。
#[tauri::command]
pub fn set_highlight_style(app: AppHandle, style: String) -> Result<(), String> {
    let path = resolve_config_path(&app, "settings.json")?;
    set_highlight_style_impl(&style, &path)?;
    notify_list_needs_refresh(&app);
    Ok(())
}

/// 通知前端「列表需要重新取数」。
///
/// 事件名复用 `clipboard-changed`：它是本工程既有的「列表数据需要刷新」信号
/// （见 `crate::CLIPBOARD_CHANGED_EVENT` 的文档与 `set_max_history_items` 的先例），
/// 并不字面表示「剪贴板内容变了」。排序与命中高亮都会改变列表的呈现结果。
fn notify_list_needs_refresh(app: &AppHandle) {
    if let Err(e) = app.emit(crate::CLIPBOARD_CHANGED_EVENT, ()) {
        eprintln!("[QuickQuick] 外观设置落盘后发送列表刷新事件失败: {e}");
    }
}

/// Tauri 命令：读取开机自启状态。
#[tauri::command]
pub fn get_launch_on_login(app: AppHandle) -> Result<bool, String> {
    let path = resolve_config_path(&app, "autostart.json")?;
    get_launch_on_login_impl(&path)
}

/// Tauri 命令：设置开机自启（持久化 autostart.json + 应用到 OS）。
#[tauri::command]
pub fn set_launch_on_login(app: AppHandle, value: bool) -> Result<(), String> {
    let path = resolve_config_path(&app, "autostart.json")?;
    set_launch_on_login_impl(value, &path)?;
    crate::autostart::apply_to_os(&app, value);
    Ok(())
}

/// provider 凭据字段 DTO（返回给前端，camelCase）。
///
/// 描述该字段是否为 secret、是否必填，供前端动态渲染凭据表单。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialFieldDto {
    /// 字段标识符（存取路由键）
    pub key: String,
    /// UI 显示标签
    pub label: String,
    /// 是否为 secret（true = 写 keychain，输入框用密码形式）
    pub is_secret: bool,
    /// 是否必填
    pub required: bool,
}

/// provider 凭据值 DTO（返回给前端，camelCase）。
///
/// 安全约定：secret 字段（is_secret=true）的 value 永远为 None，仅用 is_set 表示是否已存；
/// 非密字段（is_secret=false）的 value 为 Some(已存值) 或 None（未存）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialValueDto {
    /// 字段标识符
    pub key: String,
    /// 字段当前值：secret 字段永远为 None；非密字段为 Some(值) 或 None
    pub value: Option<String>,
    /// 是否已保存过（secret 字段靠此布尔判断；非密字段 value.is_some() 与此等价）
    pub is_set: bool,
}

/// `get_provider_credential_schema` 的纯函数实现，可在测试中直接调用。
///
/// 映射 `credential_schema` 返回的 `CredentialField` 列表为 DTO。
/// 未知 provider_id 返回空 Vec（不 Err），与 credential_schema 语义一致。
pub fn get_provider_credential_schema_impl(provider_id: &str) -> Vec<CredentialFieldDto> {
    credential_schema(provider_id)
        .into_iter()
        .map(|f| CredentialFieldDto {
            key: f.key.to_string(),
            label: f.label.to_string(),
            is_secret: f.is_secret,
            required: f.required,
        })
        .collect()
}

/// `get_provider_credentials` 的纯函数实现，可在测试中直接调用。
///
/// 走只读 DB 展示路径（`load_credentials_for_display`），
/// 遍历 schema 中每个字段组装 DTO 列表：
/// - secret 字段：value 永远为 None，is_set 表示 provider_secret 表中是否已存该 secret
/// - 非密字段：value 为 Some(已存值) 或 None（读 provider_config），is_set 与 value.is_some() 等价
///
/// # 安全
/// secret 字段的实际值永不出现在返回结果中；secret 存于整库加密 DB（不碰钥匙串），不触发弹窗。
///
/// # Errors
/// DB 读取失败时返回错误字符串。
pub fn get_provider_credentials_impl(
    provider_id: &str,
    conn: &rusqlite::Connection,
) -> Result<Vec<CredentialValueDto>, String> {
    // 走只读 DB 展示路径：secret 字段靠 provider_secret 表存在性判断是否已配置，不回明文。
    let loaded = load_credentials_for_display(provider_id, conn).map_err(|e| e.to_string())?;

    let result = credential_schema(provider_id)
        .into_iter()
        .map(|field| {
            let saved_value = loaded
                .iter()
                .find(|(k, _)| k == field.key)
                .map(|(_, v)| v.clone());

            if field.is_secret {
                CredentialValueDto {
                    key: field.key.to_string(),
                    value: None,
                    is_set: saved_value.is_some(),
                }
            } else {
                let is_set = saved_value.is_some();
                CredentialValueDto {
                    key: field.key.to_string(),
                    value: saved_value,
                    is_set,
                }
            }
        })
        .collect();

    Ok(result)
}

/// `set_provider_credentials` 的纯函数实现，可在测试中直接调用。
///
/// 将前端传入的 `HashMap<String, String>` 转为 save_credentials 所需的 `&[(&str, &str)]`，
/// 再调 save_credentials 按 schema 路由（secret→store，非密→DB）。
///
/// # 安全
/// - 函数/日志/错误消息绝不打印 secret 字段的值
/// - 未知 field_key 由 save_credentials 拒绝（返回 CredError::UnknownField）
///
/// # Errors
/// - 未知 provider_id：返回错误字符串
/// - 未知 field_key：返回错误字符串（不含字段值）
/// - store 写入失败 / DB 操作失败：返回错误字符串
pub fn set_provider_credentials_impl(
    provider_id: &str,
    values: HashMap<String, String>,
    store: &dyn crate::translate::credential::CredStore,
    conn: &rusqlite::Connection,
) -> Result<(), String> {
    let pairs: Vec<(&str, &str)> = values
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    save_credentials(provider_id, &pairs, store, conn).map_err(|e| e.to_string())
}

/// Tauri 命令：获取指定 provider 的凭据字段 schema。
#[tauri::command]
pub fn get_provider_credential_schema(provider_id: String) -> Vec<CredentialFieldDto> {
    get_provider_credential_schema_impl(&provider_id)
}

/// Tauri 命令：获取指定 provider 的已保存凭据（secret 字段只返回 is_set，不回明文）。
#[tauri::command]
pub fn get_provider_credentials(
    state: State<'_, super::AppDb>,
    provider_id: String,
) -> Result<Vec<CredentialValueDto>, String> {
    // 展示路径只读 DB（provider_secret 表存在性），不构造 store、不回明文
    super::with_db(&state, |conn| {
        get_provider_credentials_impl(&provider_id, conn)
    })
}

/// Tauri 命令：保存指定 provider 的凭据（secret→provider_secret，非密→provider_config，同一加密库）。
///
/// 保存成功后向前端 emit `provider-config-changed` 事件，
/// 使翻译页实时刷新 configuredIds、解禁已配置的 keyed 源选项。
/// emit 失败仅记录日志，不影响命令返回值。
#[tauri::command]
pub fn set_provider_credentials(
    app: AppHandle,
    state: State<'_, super::AppDb>,
    provider_id: String,
    values: HashMap<String, String>,
) -> Result<(), String> {
    super::with_db(&state, |conn| {
        let store = DbCredStore::new(conn);
        set_provider_credentials_impl(&provider_id, values, &store, conn)
    })?;
    if let Err(e) = app.emit(PROVIDER_CONFIG_CHANGED_EVENT, ()) {
        eprintln!("[QuickQuick] provider-config-changed emit 失败: {e}");
    }
    Ok(())
}

/// `delete_provider_credentials` 的纯函数实现，可在测试中直接调用。
///
/// 按 schema 遍历 provider 所有字段：secret→store.delete_secret，非密→DELETE FROM DB。
/// 幂等——字段不存在也不报错。
///
/// # Errors
/// - 未知 provider_id：返回错误字符串
/// - store 删除失败 / DB 操作失败：返回错误字符串
pub fn delete_provider_credentials_impl(
    provider_id: &str,
    store: &dyn crate::translate::credential::CredStore,
    conn: &rusqlite::Connection,
) -> Result<(), String> {
    delete_credentials(provider_id, store, conn).map_err(|e| e.to_string())
}

/// Tauri 命令：清除指定 provider 的所有已保存凭据（provider_secret + provider_config 均清）。
///
/// 删除成功后向前端 emit `provider-config-changed` 事件，
/// 使翻译页徽标从「已配置」变回「待配置」。
/// emit 失败仅记录日志，不影响命令返回值。
#[tauri::command]
pub fn delete_provider_credentials(
    app: AppHandle,
    state: State<'_, super::AppDb>,
    provider_id: String,
) -> Result<(), String> {
    super::with_db(&state, |conn| {
        let store = DbCredStore::new(conn);
        delete_provider_credentials_impl(&provider_id, &store, conn)
    })?;
    if let Err(e) = app.emit(PROVIDER_CONFIG_CHANGED_EVENT, ()) {
        eprintln!("[QuickQuick] provider-config-changed emit 失败（delete）: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, Arc};
    use tempfile::NamedTempFile;

    fn make_state(paused: bool, skip_sensitive: bool, stay_in_tray: bool) -> CaptureState {
        use crate::privacy::{IgnoreRules, SkipOnce};
        use std::sync::RwLock;
        CaptureState {
            paused: Arc::new(AtomicBool::new(paused)),
            skip_sensitive: Arc::new(AtomicBool::new(skip_sensitive)),
            stay_in_tray: Arc::new(AtomicBool::new(stay_in_tray)),
            skip_once: Arc::new(SkipOnce::new()),
            exclude_list: Arc::new(RwLock::new(IgnoreRules::default())),
        }
    }

    #[test]
    fn get_pause_capture_reads_atomic() {
        let state = make_state(true, true, true);
        assert!(get_pause_capture_impl(&state));
        state.paused.store(false, Ordering::Relaxed);
        assert!(!get_pause_capture_impl(&state));
    }

    #[test]
    fn set_pause_capture_writes_atomic_and_persists() {
        let state = make_state(false, true, true);
        let file = NamedTempFile::new().unwrap();
        set_pause_capture_impl(true, &state, file.path()).unwrap();
        assert!(state.paused.load(Ordering::Relaxed));
        let settings = AppSettings::load_or_default(file.path());
        assert!(settings.pause_capture);
    }

    #[test]
    fn get_skip_sensitive_reads_atomic() {
        let state = make_state(false, false, true);
        assert!(!get_skip_sensitive_impl(&state));
    }

    #[test]
    fn set_skip_sensitive_writes_atomic_and_persists() {
        let state = make_state(false, true, true);
        let file = NamedTempFile::new().unwrap();
        set_skip_sensitive_impl(false, &state, file.path()).unwrap();
        assert!(!state.skip_sensitive.load(Ordering::Relaxed));
        let settings = AppSettings::load_or_default(file.path());
        assert!(!settings.skip_sensitive);
    }

    #[test]
    fn get_stay_in_tray_reads_atomic() {
        let state = make_state(false, true, false);
        assert!(!get_stay_in_tray_impl(&state));
    }

    #[test]
    fn set_stay_in_tray_writes_atomic_and_persists() {
        let state = make_state(false, true, true);
        let file = NamedTempFile::new().unwrap();
        set_stay_in_tray_impl(false, &state, file.path()).unwrap();
        assert!(!state.stay_in_tray.load(Ordering::Relaxed));
        let settings = AppSettings::load_or_default(file.path());
        assert!(!settings.stay_in_tray);
    }

    #[test]
    fn get_auto_update_returns_default_when_no_file() {
        let file = NamedTempFile::new().unwrap();
        let nonexistent = file.path().with_extension("nonexistent_settings.json");
        let result = get_auto_update_impl(&nonexistent).unwrap();
        assert!(result, "auto_update 默认应为 true");
    }

    #[test]
    fn set_auto_update_persists_value() {
        let file = NamedTempFile::new().unwrap();
        set_auto_update_impl(false, file.path()).unwrap();
        assert!(!get_auto_update_impl(file.path()).unwrap());
    }

    #[test]
    fn get_theme_returns_default_when_no_file() {
        let file = NamedTempFile::new().unwrap();
        let nonexistent = file.path().with_extension("nonexistent_settings.json");
        assert_eq!(get_theme_impl(&nonexistent).unwrap(), "auto");
    }

    #[test]
    fn set_theme_rejects_invalid_value() {
        let file = NamedTempFile::new().unwrap();
        let err = set_theme_impl("purple", file.path()).unwrap_err();
        assert!(err.contains("非法 theme 值"), "错误信息应说明非法值：{err}");
    }

    #[test]
    fn set_theme_accepts_valid_values() {
        let file = NamedTempFile::new().unwrap();
        for theme in ["auto", "light", "dark"] {
            set_theme_impl(theme, file.path()).unwrap();
            assert_eq!(get_theme_impl(file.path()).unwrap(), theme);
        }
    }

    #[test]
    fn get_launch_on_login_returns_default_when_no_file() {
        let file = NamedTempFile::new().unwrap();
        let nonexistent = file.path().with_extension("nonexistent_autostart.json");
        let result = get_launch_on_login_impl(&nonexistent).unwrap();
        assert!(result, "launch_on_login 默认应为 true（设计§二）");
    }

    #[test]
    fn set_launch_on_login_persists_value() {
        let file = NamedTempFile::new().unwrap();
        set_launch_on_login_impl(false, file.path()).unwrap();
        assert!(!get_launch_on_login_impl(file.path()).unwrap());
        set_launch_on_login_impl(true, file.path()).unwrap();
        assert!(get_launch_on_login_impl(file.path()).unwrap());
    }

    #[test]
    fn set_pause_capture_preserves_existing_fields() {
        let state = make_state(false, true, true);
        let file = NamedTempFile::new().unwrap();
        let initial = AppSettings {
            excluded_apps: vec!["com.test.app".to_string()],
            selected_provider: "deepl".to_string(),
            ..AppSettings::default()
        };
        initial.save(file.path()).unwrap();
        set_pause_capture_impl(true, &state, file.path()).unwrap();
        let restored = AppSettings::load_or_default(file.path());
        assert_eq!(restored.excluded_apps, vec!["com.test.app"]);
        assert_eq!(restored.selected_provider, "deepl");
        assert!(restored.pause_capture);
    }

    #[test]
    fn set_exclude_list_persists_to_file() {
        let file = NamedTempFile::new().unwrap();
        let apps = vec![
            "com.1password.1password".to_string(),
            "com.foo.bar".to_string(),
        ];
        set_exclude_list_impl(apps.clone(), file.path(), None).unwrap();
        let settings = AppSettings::load_or_default(file.path());
        assert_eq!(settings.excluded_apps, apps);
    }

    #[test]
    fn get_image_threshold_returns_default_when_no_file() {
        let file = NamedTempFile::new().unwrap();
        let nonexistent = file.path().with_extension("nonexistent_settings.json");
        let result = get_image_threshold_impl(&nonexistent).unwrap();
        assert_eq!(result, 20 * 1024 * 1024, "默认阈值应为 20MiB");
    }

    #[test]
    fn set_image_threshold_persists_and_get_reads_back() {
        let file = NamedTempFile::new().unwrap();
        let threshold = 50 * 1024 * 1024u64;
        set_image_threshold_impl(threshold, file.path()).unwrap();
        let got = get_image_threshold_impl(file.path()).unwrap();
        assert_eq!(got, threshold, "set 后 get 应返回相同值");
    }

    #[test]
    fn set_image_threshold_rejects_too_small() {
        let file = NamedTempFile::new().unwrap();
        let err = set_image_threshold_impl(512 * 1024, file.path()).unwrap_err();
        assert!(err.contains("范围"), "错误信息应说明范围限制：{err}");
    }

    #[test]
    fn set_image_threshold_rejects_too_large() {
        let file = NamedTempFile::new().unwrap();
        let too_large = 600 * 1024 * 1024u64;
        let err = set_image_threshold_impl(too_large, file.path()).unwrap_err();
        assert!(err.contains("范围"), "错误信息应说明范围限制：{err}");
    }

    #[test]
    fn set_image_threshold_out_of_range_does_not_modify_file() {
        let file = NamedTempFile::new().unwrap();
        set_image_threshold_impl(30 * 1024 * 1024, file.path()).unwrap();
        let _ = set_image_threshold_impl(0, file.path()).unwrap_err();
        let got = get_image_threshold_impl(file.path()).unwrap();
        assert_eq!(got, 30 * 1024 * 1024, "越界 set 后文件不应被改动");
    }

    #[test]
    fn set_exclude_list_updates_runtime_immediately() {
        use crate::privacy::ExcludeList;
        use std::sync::RwLock;

        let lock = RwLock::new(ExcludeList::default());
        let apps = vec!["com.1password.1password".to_string()];
        let file = NamedTempFile::new().unwrap();

        // 写入前名单为空
        assert!(!lock.read().unwrap().contains("com.1password.1password"));

        set_exclude_list_impl(apps, file.path(), Some(&lock)).unwrap();

        // 写入后运行时名单立即包含新 app，无需重启
        assert!(
            lock.read().unwrap().contains("com.1password.1password"),
            "运行时排除名单应在 set_exclude_list 后立即生效"
        );
    }

    // ===== 忽略规则增强（P3-5） =====

    /// 默认模式是黑名单：旧 settings.json 与全新安装都走历史行为。
    #[test]
    fn get_exclude_mode_defaults_to_blacklist() {
        use crate::privacy::ExcludeMode;

        let file = NamedTempFile::new().unwrap();
        assert_eq!(
            get_exclude_mode_impl(file.path()).unwrap(),
            ExcludeMode::Blacklist,
            "无配置文件时模式应回落到黑名单"
        );
    }

    /// 切换模式只改方向，不动已填条目与内容正则。
    ///
    /// 这是本项最容易写错的地方：三个设置入口各自 save 一次 settings，
    /// 若从局部变量重建规则集，很容易把另外两个字段覆盖掉。
    #[test]
    fn set_exclude_mode_keeps_apps_and_regexes() {
        use crate::privacy::{ExcludeMode, IgnoreRules};
        use std::sync::RwLock;

        let lock = RwLock::new(IgnoreRules::default());
        let file = NamedTempFile::new().unwrap();
        set_exclude_list_impl(vec!["1Password".to_string()], file.path(), Some(&lock)).unwrap();
        set_ignore_regexes_impl(vec!["^secret-".to_string()], file.path(), Some(&lock)).unwrap();

        // Act：切到白名单
        set_exclude_mode_impl(ExcludeMode::Whitelist, file.path(), Some(&lock)).unwrap();

        // Assert：模式变了，条目与正则都还在（持久化与运行时两侧都要成立）
        let settings = AppSettings::load_or_default(file.path());
        assert_eq!(settings.exclude_mode, ExcludeMode::Whitelist);
        assert_eq!(settings.excluded_apps, vec!["1Password".to_string()]);
        assert_eq!(settings.ignore_regexes, vec!["^secret-".to_string()]);

        let rules = lock.read().unwrap();
        assert_eq!(rules.mode(), ExcludeMode::Whitelist);
        assert!(rules.contains("1password"), "切换模式不该清空应用条目");
        assert_eq!(rules.regex_count(), 1, "切换模式不该丢掉内容正则");
    }

    /// 内容正则原文照存：非法条目也要能被前端读回来修正，不能被静默丢弃。
    #[test]
    fn set_ignore_regexes_persists_raw_patterns_even_invalid() {
        use crate::privacy::IgnoreRules;
        use std::sync::RwLock;

        let lock = RwLock::new(IgnoreRules::default());
        let file = NamedTempFile::new().unwrap();
        let patterns = vec!["^\\d{16}$".to_string(), "([unclosed".to_string()];

        set_ignore_regexes_impl(patterns.clone(), file.path(), Some(&lock)).unwrap();

        assert_eq!(
            get_ignore_regexes_impl(file.path()).unwrap(),
            patterns,
            "非法正则的原文也必须存下来，否则用户没法在设置页修它"
        );
        assert_eq!(
            lock.read().unwrap().regex_count(),
            1,
            "运行时只编译合法的那一条，非法条目降级不生效"
        );
    }

    fn make_cred_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS provider_config (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
                value        TEXT NOT NULL,
                PRIMARY KEY (provider_id, field_key)
            );
            CREATE TABLE IF NOT EXISTS provider_secret (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
                value        TEXT NOT NULL,
                PRIMARY KEY (provider_id, field_key)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn get_provider_credential_schema_impl_baidu_returns_two_fields() {
        let fields = get_provider_credential_schema_impl("baidu");
        assert_eq!(fields.len(), 2, "百度 schema 应有 2 个字段");
        let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
        assert!(keys.contains(&"app_id"), "应含 app_id 字段");
        assert!(keys.contains(&"secret_key"), "应含 secret_key 字段");
    }

    #[test]
    fn get_provider_credential_schema_impl_unknown_returns_empty() {
        let fields = get_provider_credential_schema_impl("unknown_provider");
        assert!(fields.is_empty(), "未知 provider 应返回空 schema");
    }

    #[test]
    fn get_provider_credentials_impl_unset_fields_are_not_set() {
        let conn = make_cred_db();

        let results = get_provider_credentials_impl("baidu", &conn).unwrap();
        assert_eq!(results.len(), 2, "百度凭据应返回 2 个字段");
        for item in &results {
            assert!(!item.is_set, "未保存时所有字段 is_set 应为 false");
            assert!(item.value.is_none(), "未保存时 value 应为 None");
        }
    }

    #[test]
    fn get_provider_credentials_impl_secret_field_value_is_always_none() {
        use crate::translate::credential::DbCredStore;
        use std::collections::HashMap;
        let conn = make_cred_db();
        let store = DbCredStore::new(&conn);

        let mut values = HashMap::new();
        values.insert("app_id".to_string(), "my_app_id".to_string());
        values.insert("secret_key".to_string(), "super_secret".to_string());
        set_provider_credentials_impl("baidu", values, &store, &conn).unwrap();

        let results = get_provider_credentials_impl("baidu", &conn).unwrap();
        let secret_field = results.iter().find(|f| f.key == "secret_key").unwrap();
        assert!(
            secret_field.value.is_none(),
            "secret 字段的 value 永远应为 None（不回明文）"
        );
        assert!(secret_field.is_set, "已保存的 secret 字段 is_set 应为 true");

        let non_secret = results.iter().find(|f| f.key == "app_id").unwrap();
        assert_eq!(
            non_secret.value,
            Some("my_app_id".to_string()),
            "非密字段已保存后应返回 Some(值)"
        );
        assert!(non_secret.is_set, "已保存的非密字段 is_set 应为 true");
    }

    #[test]
    fn set_provider_credentials_impl_persists_and_loadable() {
        use crate::translate::credential::DbCredStore;
        use std::collections::HashMap;
        let conn = make_cred_db();
        let store = DbCredStore::new(&conn);

        let mut values = HashMap::new();
        values.insert("app_id".to_string(), "test_id".to_string());
        values.insert("secret_key".to_string(), "test_secret".to_string());

        set_provider_credentials_impl("baidu", values, &store, &conn).unwrap();

        let results = get_provider_credentials_impl("baidu", &conn).unwrap();
        let app_id = results.iter().find(|f| f.key == "app_id").unwrap();
        assert_eq!(app_id.value, Some("test_id".to_string()));
        let secret_key = results.iter().find(|f| f.key == "secret_key").unwrap();
        assert!(secret_key.is_set);
        assert!(secret_key.value.is_none(), "secret 字段 value 永远 None");
    }

    #[test]
    fn set_provider_credentials_impl_unknown_field_returns_err() {
        use crate::translate::credential::MockCredStore;
        use std::collections::HashMap;
        let store = MockCredStore::new();
        let conn = make_cred_db();

        let mut values = HashMap::new();
        values.insert("nonexistent_field".to_string(), "val".to_string());

        let result = set_provider_credentials_impl("baidu", values, &store, &conn);
        assert!(result.is_err(), "未知 field_key 应返回 Err");
    }

    #[test]
    fn delete_provider_credentials_impl_clears_all_fields() {
        use crate::translate::credential::DbCredStore;
        use std::collections::HashMap;
        let conn = make_cred_db();
        let store = DbCredStore::new(&conn);

        let mut values = HashMap::new();
        values.insert("app_id".to_string(), "test_id".to_string());
        values.insert("secret_key".to_string(), "test_secret".to_string());
        set_provider_credentials_impl("baidu", values, &store, &conn).unwrap();
        // 前置断言：删除前 secret 与非密字段都确实已配置（避免删除后全 false 是假绿）
        let before = get_provider_credentials_impl("baidu", &conn).unwrap();
        assert!(
            before.iter().all(|item| item.is_set),
            "删除前所有字段应已配置"
        );

        delete_provider_credentials_impl("baidu", &store, &conn).unwrap();

        let results = get_provider_credentials_impl("baidu", &conn).unwrap();
        for item in &results {
            assert!(
                !item.is_set,
                "删除后所有字段 is_set 应为 false，实际：{item:?}"
            );
        }
    }

    #[test]
    fn delete_provider_credentials_impl_unknown_provider_returns_err() {
        use crate::translate::credential::MockCredStore;
        let store = MockCredStore::new();
        let conn = make_cred_db();

        let result = delete_provider_credentials_impl("unknown_xyz", &store, &conn);
        assert!(result.is_err(), "未知 provider 应返回 Err");
    }

    #[test]
    fn delete_provider_credentials_impl_idempotent() {
        use crate::translate::credential::MockCredStore;
        let store = MockCredStore::new();
        let conn = make_cred_db();

        let result = delete_provider_credentials_impl("baidu", &store, &conn);
        assert!(result.is_ok(), "空 provider 也应成功（幂等）: {result:?}");
    }

    // 对齐 acceptance TV1-F4-A01：DTO 透出 is_unofficial（前端据此渲染「非官方」标注）。
    // 非官方免 key 源 true、官方 keyed 源 false，与 capability 一致。
    #[test]
    fn get_translate_providers_impl_exposes_is_unofficial() {
        let dtos = get_translate_providers_impl();
        let find = |id: &str| {
            dtos.iter()
                .find(|d| d.id == id)
                .unwrap_or_else(|| panic!("DTO 列表应含 {id}"))
        };
        assert!(find("lingva").is_unofficial, "lingva 应标注非官方");
        assert!(find("bing").is_unofficial, "bing 应标注非官方");
        assert!(!find("baidu").is_unofficial, "baidu 为官方源");
        assert!(!find("google").is_unofficial, "google 官方源");
    }

    // needs_config = 凭据 schema 非空，作为「是否可配置」的判据，独立于 needs_key（apiKey 语义）。
    // Ollama needs_key=false 但有必填 model 字段 → needs_config=true，前端据此显示「配置」按钮。
    #[test]
    fn provider_dto_needs_config_true_for_ollama_and_keyed() {
        let dtos = get_translate_providers_impl();
        let find = |id: &str| {
            dtos.iter()
                .find(|d| d.id == id)
                .unwrap_or_else(|| panic!("DTO 列表应含 {id}"))
        };
        // Ollama：无 apiKey（needs_key=false）但有必填 model → 可配置。
        assert!(
            !find("ollama").needs_key,
            "ollama 本地无鉴权，needs_key=false"
        );
        assert!(
            find("ollama").needs_config,
            "ollama 有必填 model 字段，应可配置"
        );
        // 需 key 源仍可配置（有 apiKey 等字段）。
        assert!(find("baidu").needs_config, "baidu 有凭据字段，应可配置");
        assert!(find("openai").needs_config, "openai 有凭据字段，应可配置");
    }

    // 无凭据 schema 的免 key 源（google_free/lingva）→ needs_config=false，前端徽标「无需配置」。
    #[test]
    fn provider_dto_needs_config_false_for_keyless_sources() {
        let dtos = get_translate_providers_impl();
        let find = |id: &str| {
            dtos.iter()
                .find(|d| d.id == id)
                .unwrap_or_else(|| panic!("DTO 列表应含 {id}"))
        };
        assert!(
            !find("google_free").needs_config,
            "google_free 无凭据字段，不可配置"
        );
        assert!(!find("lingva").needs_config, "lingva 无凭据字段，不可配置");
    }

    // ===== 历史条目上限（P4-4） =====

    /// 无配置文件时读到默认 500 条（与旧版硬编码的「手动清理 500 条」一致）。
    #[test]
    fn get_max_history_items_returns_default_500_when_no_file() {
        let file = NamedTempFile::new().unwrap();
        let nonexistent = file.path().with_extension("nonexistent_settings.json");

        let items = get_max_history_items_impl(&nonexistent).unwrap();

        assert_eq!(items, 500, "默认上限应为 500 条");
    }

    /// 写入后能读回同一个值。
    #[test]
    fn set_max_history_items_persists_and_read_reads_back() {
        let file = NamedTempFile::new().unwrap();

        let evicted = set_max_history_items_impl(42, file.path(), None).unwrap();

        assert_eq!(evicted, 0, "没给连接时不应淘汰任何条目");
        assert_eq!(get_max_history_items_impl(file.path()).unwrap(), 42);
    }

    /// 区间两端（1 / 999）都必须被接受，且原样持久化。
    #[test]
    fn set_max_history_items_accepts_boundaries() {
        let file = NamedTempFile::new().unwrap();

        set_max_history_items_impl(1, file.path(), None).unwrap();
        assert_eq!(get_max_history_items_impl(file.path()).unwrap(), 1);

        set_max_history_items_impl(999, file.path(), None).unwrap();
        assert_eq!(get_max_history_items_impl(file.path()).unwrap(), 999);
    }

    /// 0 被拒绝：不写文件、报错文案点明合法范围。
    ///
    /// 前置写入 300 是关键：只断言 `Err` 无法证明「没落盘」，
    /// 必须证明文件里还是上一次的合法值。
    #[test]
    fn set_max_history_items_rejects_zero_without_touching_file() {
        let file = NamedTempFile::new().unwrap();
        set_max_history_items_impl(300, file.path(), None).unwrap();

        let err = set_max_history_items_impl(0, file.path(), None).unwrap_err();

        assert!(err.contains("范围"), "错误信息应说明范围：{err}");
        assert!(
            err.contains("1") && err.contains("999"),
            "错误信息应点明 1 到 999：{err}"
        );
        assert_eq!(
            get_max_history_items_impl(file.path()).unwrap(),
            300,
            "非法值不得落盘（文件应保留上一次的合法值）"
        );
    }

    /// 1000 被拒绝（上限之外不能悄悄截成 999）。
    #[test]
    fn set_max_history_items_rejects_over_max() {
        let file = NamedTempFile::new().unwrap();
        set_max_history_items_impl(300, file.path(), None).unwrap();

        let err = set_max_history_items_impl(1000, file.path(), None).unwrap_err();

        assert!(err.contains("范围"), "错误信息应说明范围：{err}");
        assert_eq!(
            get_max_history_items_impl(file.path()).unwrap(),
            300,
            "越界 set 后文件不应被改动"
        );
    }

    /// 上限调小后必须**立刻**按新上限收敛，不能等下一次复制。
    #[test]
    fn set_max_history_items_trims_existing_rows_immediately() {
        // Arrange：库里 5 条非收藏条目（时间戳递增）
        let (_dir, conn) = crate::image::open_test_db();
        for i in 0..5 {
            conn.execute(
                "INSERT INTO clip_items
                     (id, content, kind, created_utc, last_modified_utc, is_deleted,
                      text_hash, is_favorite)
                 VALUES (?1, ?2, 'text', ?3, ?3, 0, ?4, 0)",
                rusqlite::params![
                    format!("row-{i}"),
                    format!("content-{i}"),
                    1_000 + i,
                    format!("hash-{i}")
                ],
            )
            .expect("插入种子行失败");
        }
        let file = NamedTempFile::new().unwrap();

        // Act
        let evicted = set_max_history_items_impl(2, file.path(), Some(&conn)).unwrap();

        // Assert：写入 + 立即淘汰 + 只留最新的两条
        assert_eq!(evicted, 3, "5 条超 2 条上限应淘汰 3 条");
        assert_eq!(get_max_history_items_impl(file.path()).unwrap(), 2);
        assert_eq!(
            crate::db::count_live(&conn).expect("count_live"),
            2,
            "调小上限后历史应立刻收敛"
        );
        let survivors: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT id FROM clip_items WHERE is_deleted = 0 ORDER BY last_modified_utc ASC",
                )
                .expect("准备查询失败");
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .expect("查询失败");
            rows.map(|r| r.expect("读取失败")).collect()
        };
        assert_eq!(
            survivors,
            vec!["row-3".to_string(), "row-4".to_string()],
            "保留的必须是最新的两条"
        );
    }

    /// 裁剪失败（连接不可写）不影响设置落盘：设置已经保存，下一次入库还会再收敛。
    #[test]
    fn set_max_history_items_keeps_settings_when_trim_fails() {
        // Arrange：任何「标记软删」的 UPDATE 都被触发器阻断
        let (_dir, conn) = crate::image::open_test_db();
        conn.execute_batch(
            "CREATE TRIGGER block_evict BEFORE UPDATE ON clip_items
             WHEN OLD.is_deleted = 0 AND NEW.is_deleted = 1
             BEGIN SELECT RAISE(ABORT, 'eviction blocked by test trigger'); END;",
        )
        .expect("创建触发器应失败");
        for i in 0..3 {
            conn.execute(
                "INSERT INTO clip_items
                     (id, content, kind, created_utc, last_modified_utc, is_deleted,
                      text_hash, is_favorite)
                 VALUES (?1, ?2, 'text', ?3, ?3, 0, ?4, 0)",
                rusqlite::params![
                    format!("row-{i}"),
                    format!("content-{i}"),
                    1_000 + i,
                    format!("hash-{i}")
                ],
            )
            .expect("插入种子行失败");
        }
        let file = NamedTempFile::new().unwrap();

        // Act：裁剪必然失败，但整体调用不应失败
        let result = set_max_history_items_impl(1, file.path(), Some(&conn));

        // Assert：设置已落盘，条目保持原样（淘汰没成功也没留半状态）
        assert_eq!(result.expect("裁剪失败不应让设置失败"), 0);
        assert_eq!(
            get_max_history_items_impl(file.path()).unwrap(),
            1,
            "设置必须已落盘"
        );
        assert_eq!(
            crate::db::count_live(&conn).expect("count_live"),
            3,
            "淘汰被阻断时条目应原样保留"
        );
    }

    /// 读路径兜底：settings.json 被手工写成 0 时读回的是下限 1，不是 0。
    #[test]
    fn get_max_history_items_clamps_hand_edited_zero() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), r#"{"max_history_items":0}"#).unwrap();

        assert_eq!(
            get_max_history_items_impl(file.path()).unwrap(),
            1,
            "手写 0 必须被收敛到下限 1，否则每次复制都会清空历史"
        );
    }

    /// 读路径兜底：手写超大值收敛到上限 999。
    #[test]
    fn get_max_history_items_clamps_hand_edited_oversize() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), r#"{"max_history_items":10000}"#).unwrap();

        assert_eq!(
            get_max_history_items_impl(file.path()).unwrap(),
            999,
            "手写超大值必须收敛到上限 999"
        );
    }

    /// 写入历史上限不能丢其他字段（排除名单/翻译源等）。
    #[test]
    fn set_max_history_items_preserves_other_settings() {
        let file = NamedTempFile::new().unwrap();
        let initial = AppSettings {
            excluded_apps: vec!["1Password".to_string()],
            selected_provider: "deepl".to_string(),
            ..AppSettings::default()
        };
        initial.save(file.path()).unwrap();

        set_max_history_items_impl(77, file.path(), None).unwrap();

        let restored = AppSettings::load_or_default(file.path());
        assert_eq!(restored.excluded_apps, vec!["1Password".to_string()]);
        assert_eq!(restored.selected_provider, "deepl");
        assert_eq!(restored.max_history_items, 77);
    }

    /// 没有 settings.json 时，外观设置四项都必须等于改动前的硬编码默认值。
    #[test]
    fn get_appearance_settings_defaults_match_pre_change_behaviour() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such-settings.json");

        let appearance = get_appearance_settings_impl(&missing).unwrap();

        assert_eq!(appearance.sort_order, SortOrder::Recent, "默认最近复制优先");
        assert_eq!(
            appearance.image_height_px, 40,
            "默认缩略图高度 40px（对齐 Maccy）"
        );
        assert_eq!(
            appearance.preview_first_delay_ms, 1500,
            "默认首个预览延迟 1500ms（与改动前一致）"
        );
        assert_eq!(appearance.highlight_style, HighlightStyle::Bold);
    }

    /// 读路径兜底：手写越界的图片高度/预览延迟要收敛到区间内，而不是原样透出。
    #[test]
    fn get_appearance_settings_clamps_hand_edited_numbers() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"{"image_height_px":999,"preview_first_delay_ms":0}"#,
        )
        .unwrap();

        let appearance = get_appearance_settings_impl(file.path()).unwrap();

        assert_eq!(appearance.image_height_px, MAX_IMAGE_HEIGHT_PX);
        assert_eq!(
            appearance.preview_first_delay_ms,
            MIN_PREVIEW_FIRST_DELAY_MS
        );
    }

    /// 每个 setter 都要能落盘并被批量读取读回（读写同一来源，不各自解析）。
    #[test]
    fn appearance_setters_persist_and_read_back() {
        let file = NamedTempFile::new().unwrap();

        set_sort_order_impl("first_copied", file.path()).unwrap();
        set_image_height_impl(40, file.path()).unwrap();
        set_preview_first_delay_impl(800, file.path()).unwrap();
        set_highlight_style_impl("underline", file.path()).unwrap();

        let appearance = get_appearance_settings_impl(file.path()).unwrap();
        assert_eq!(appearance.sort_order, SortOrder::FirstCopied);
        assert_eq!(appearance.image_height_px, 40);
        assert_eq!(appearance.preview_first_delay_ms, 800);
        assert_eq!(appearance.highlight_style, HighlightStyle::Underline);
    }

    /// 区间两端必须被接受（边界值不能被误判为越界）。
    #[test]
    fn appearance_setters_accept_boundary_values() {
        let file = NamedTempFile::new().unwrap();

        for px in [MIN_IMAGE_HEIGHT_PX, MAX_IMAGE_HEIGHT_PX] {
            set_image_height_impl(px, file.path()).unwrap();
            assert_eq!(
                get_appearance_settings_impl(file.path())
                    .unwrap()
                    .image_height_px,
                px
            );
        }
        for ms in [MIN_PREVIEW_FIRST_DELAY_MS, MAX_PREVIEW_FIRST_DELAY_MS] {
            set_preview_first_delay_impl(ms, file.path()).unwrap();
            assert_eq!(
                get_appearance_settings_impl(file.path())
                    .unwrap()
                    .preview_first_delay_ms,
                ms
            );
        }
    }

    /// 越界图片高度被拒收且**不落盘**：先写一个合法值，再证明非法值没改动文件。
    #[test]
    fn set_image_height_rejects_out_of_range_without_touching_file() {
        let file = NamedTempFile::new().unwrap();
        set_image_height_impl(40, file.path()).unwrap();

        for bad in [0u32, 201, 999] {
            let err = set_image_height_impl(bad, file.path()).unwrap_err();
            assert!(err.contains("范围"), "错误信息应说明范围：{err}");
            assert!(
                err.contains("1") && err.contains("200"),
                "错误信息应点明 1 到 200：{err}"
            );
        }

        assert_eq!(
            get_appearance_settings_impl(file.path())
                .unwrap()
                .image_height_px,
            40,
            "非法值不得落盘（文件应保留上一次的合法值）"
        );
    }

    /// 越界首次预览延迟被拒收且不落盘。
    #[test]
    fn set_preview_first_delay_rejects_out_of_range_without_touching_file() {
        let file = NamedTempFile::new().unwrap();
        set_preview_first_delay_impl(1500, file.path()).unwrap();

        for bad in [0u32, 199, 100_001] {
            let err = set_preview_first_delay_impl(bad, file.path()).unwrap_err();
            assert!(err.contains("范围"), "错误信息应说明范围：{err}");
            assert!(
                err.contains("200") && err.contains("100000"),
                "错误信息应点明 200 到 100000：{err}"
            );
        }

        assert_eq!(
            get_appearance_settings_impl(file.path())
                .unwrap()
                .preview_first_delay_ms,
            1500,
            "非法值不得落盘（文件应保留上一次的合法值）"
        );
    }

    /// 非法排序方式被拒收，且错误文案列全合法值；文件保持上一次的合法值。
    #[test]
    fn set_sort_order_rejects_unknown_value_without_touching_file() {
        let file = NamedTempFile::new().unwrap();
        set_sort_order_impl("recent", file.path()).unwrap();

        let err = set_sort_order_impl("by_magic", file.path()).unwrap_err();

        assert!(err.contains("by_magic"), "错误信息应回显非法值：{err}");
        assert!(
            err.contains("recent") && err.contains("first_copied"),
            "错误信息应列出合法值：{err}"
        );
        assert_eq!(
            get_appearance_settings_impl(file.path())
                .unwrap()
                .sort_order,
            SortOrder::Recent,
            "非法值不得落盘"
        );
    }

    /// 非法高亮样式被拒收，且错误文案列全四档（与 Maccy 的 HighlightMatch 同名）。
    #[test]
    fn set_highlight_style_rejects_unknown_value_without_touching_file() {
        let file = NamedTempFile::new().unwrap();
        set_highlight_style_impl("underline", file.path()).unwrap();

        let err = set_highlight_style_impl("rainbow", file.path()).unwrap_err();

        assert!(err.contains("rainbow"), "错误信息应回显非法值：{err}");
        for allowed in ["color", "bold", "italic", "underline"] {
            assert!(
                err.contains(allowed),
                "错误信息应列出合法值 {allowed}：{err}"
            );
        }
        assert_eq!(
            get_appearance_settings_impl(file.path())
                .unwrap()
                .highlight_style,
            HighlightStyle::Underline,
            "非法值不得落盘"
        );
    }

    /// 外观 setter 不能丢其他字段（与历史上限的同类用例同口径）。
    #[test]
    fn appearance_setters_preserve_other_settings() {
        let file = NamedTempFile::new().unwrap();
        let initial = AppSettings {
            excluded_apps: vec!["1Password".to_string()],
            selected_provider: "deepl".to_string(),
            max_history_items: 300,
            ..AppSettings::default()
        };
        initial.save(file.path()).unwrap();

        set_image_height_impl(32, file.path()).unwrap();

        let restored = AppSettings::load_or_default(file.path());
        assert_eq!(restored.excluded_apps, vec!["1Password".to_string()]);
        assert_eq!(restored.selected_provider, "deepl");
        assert_eq!(restored.max_history_items, 300);
        assert_eq!(restored.image_height_px, 32);
    }

    /// `read_sort_order_from_path` 是列表命令的读口：缺文件回退最近复制。
    #[test]
    fn read_sort_order_from_path_defaults_to_recent_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.json");

        assert_eq!(read_sort_order_from_path(&missing), SortOrder::Recent);
    }

    /// `read_sort_order_from_path` 读回已写入的首次复制。
    #[test]
    fn read_sort_order_from_path_reads_persisted_value() {
        let file = NamedTempFile::new().unwrap();
        set_sort_order_impl("first_copied", file.path()).unwrap();

        assert_eq!(
            read_sort_order_from_path(file.path()),
            SortOrder::FirstCopied
        );
    }

    /// 运行时注册失败映射：底层 `AlreadyRegistered`（X11 / Windows 重复注册形态）
    /// 必须折叠成中文文案，英文原文不得漏进用户可见字符串。
    ///
    /// 文案逐字锁定（不比对 `HotkeyError::AlreadyInUse.to_string()`）：这是用户可见契约，
    /// 改它需要产品决定，不能让改错误枚举 Display 的人悄悄把 UI 文案也改掉。
    #[test]
    fn ipc_settings_map_runtime_register_error_already_registered_is_chinese() {
        let mapped = map_runtime_register_error(
            "HotKey already registered: HotKey { id: 1, mods: SUPER | SHIFT, key: KeyC }",
        );

        assert_eq!(
            mapped, "热键已被占用，无法绑定",
            "已占用类错误应折叠为中文文案"
        );
        assert!(
            !mapped.contains("already registered"),
            "中文文案不应把英文原文漏给用户，实际: {mapped}"
        );
    }

    /// 运行时注册失败映射：macOS `RegisterEventHotKey` 失败（其它 App 占用该组合键）
    /// 同样属于「已被占用」，文案必须与 `AlreadyRegistered` 一致。
    #[test]
    fn ipc_settings_map_runtime_register_error_macos_carbon_conflict_is_chinese() {
        let mapped = map_runtime_register_error("RegisterEventHotKey failed for CmdOrCtrl+Shift+C");

        assert_eq!(
            mapped, "热键已被占用，无法绑定",
            "macOS Carbon 注册失败应归入已占用类"
        );
    }

    /// 运行时注册失败映射：**正向对照**——非占用类错误保持原映射（前缀 + 英文原文），
    /// 证明上一条的中文文案不是「所有错误都被改成中文」。
    #[test]
    fn ipc_settings_map_runtime_register_error_other_errors_keep_original_mapping() {
        let mapped = map_runtime_register_error("Unknown scancode for F99");

        assert_eq!(
            mapped, "热键运行时注册失败: Unknown scancode for F99",
            "非占用类错误应保持原映射（含英文原文）"
        );
    }

    /// P5-8 验收：文件里有一个坏字段时，调用**任一** setter 都不得覆盖其余字段。
    ///
    /// 旧行为：`load_or_default` 因坏字段整份回退默认 → setter 把整份默认值写回，
    /// 排除名单 / 翻译源 / 历史上限全被抹掉。这里用「写一个跟坏字段毫无关系的键」来验。
    #[test]
    fn setter_preserves_other_fields_when_one_field_is_invalid() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"{
  "excluded_apps": ["1Password", "Terminal"],
  "selected_provider": "deepl",
  "max_history_items": 300,
  "sort_order": "by_magic"
}"#,
        )
        .unwrap();

        set_image_height_impl(40, file.path()).unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(file.path()).unwrap()).unwrap();
        let map = written.as_object().unwrap();

        assert_eq!(
            map["excluded_apps"],
            serde_json::json!(["1Password", "Terminal"]),
            "排除名单必须原样保留"
        );
        assert_eq!(map["selected_provider"], serde_json::json!("deepl"));
        assert_eq!(map["max_history_items"], serde_json::json!(300));
        assert_eq!(
            map["sort_order"],
            serde_json::json!("by_magic"),
            "坏键保持原样留给用户修，不得被静默改成默认值"
        );
        assert_eq!(map["image_height_px"], serde_json::json!(40));
    }

    /// P5-8 验收：语法损坏的文件 → setter **返回中文 Err 并拒写**，原文件逐字节不变，
    /// 同时留一份备份（带时间戳）。
    ///
    /// 拒写而不是「从空对象重建」：无法解析的文件里可能全是用户的真实设置，
    /// 重建后写回等于把它们删掉。错误文案要能直接给用户看，所以断言中文关键句与备份名。
    #[test]
    fn setter_rejects_syntax_broken_file_and_keeps_it_intact() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let original = "{ this is not json";
        std::fs::write(&path, original).unwrap();

        let err = set_theme_impl("dark", &path).expect_err("损坏文件必须拒写");

        assert!(
            err.contains("已拒绝写入") && err.contains("修复"),
            "IPC 错误文案必须可读，实际：{err}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            original,
            "拒写后原文件必须逐字节不变"
        );

        let backups: Vec<std::path::PathBuf> = std::fs::read_dir(dir.path())
            .unwrap()
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
        assert_eq!(backups.len(), 1, "应留一份备份，实际：{backups:?}");
        assert_eq!(
            std::fs::read_to_string(&backups[0]).unwrap(),
            original,
            "备份必须是原文件完整副本"
        );
        assert!(
            err.contains(
                backups[0]
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap()
            ),
            "错误文案应带上备份文件名，实际：{err}"
        );
    }

    /// P5-8 读侧验收：同文件里有个坏字段时，读别的字段仍拿得到真实值。
    ///
    /// 之前这条会返回空列表——用户会看到「排除名单突然没了」，
    /// 而文件里其实一个字都没丢。
    #[test]
    fn getters_keep_working_when_a_sibling_field_is_invalid() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"{"excluded_apps":["1Password"],"highlight_style":"rainbow"}"#,
        )
        .unwrap();

        assert_eq!(
            get_exclude_list_impl(file.path()).unwrap(),
            vec!["1Password".to_string()],
            "坏的高亮样式不该连累排除名单"
        );
        assert_eq!(
            get_appearance_settings_impl(file.path())
                .unwrap()
                .highlight_style,
            crate::settings::HighlightStyle::Bold,
            "坏字段自身回退默认值（对齐 Maccy 的 bold）"
        );
    }

    /// P5-8：写入保留未知键——未来版本/别的工具写的字段不能被这次写抹掉。
    #[test]
    fn setter_keeps_unknown_keys() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"{"excluded_apps":[],"future_flag":true,"future_nested":{"a":1}}"#,
        )
        .unwrap();

        set_theme_impl("light", file.path()).unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(file.path()).unwrap()).unwrap();
        assert_eq!(written["future_flag"], serde_json::json!(true));
        assert_eq!(written["future_nested"], serde_json::json!({"a": 1}));
        assert_eq!(written["theme"], serde_json::json!("light"));
    }
}
