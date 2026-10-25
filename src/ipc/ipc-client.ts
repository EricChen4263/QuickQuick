import { invoke } from "@tauri-apps/api/core";

/** 剪贴板条目，与 Rust ClipItemDto（camelCase）对齐。 */
export interface ClipItem {
  id: string;
  content: string;
  /**
   * 搜索/展示用的预计算标题（后端落库，截断到 1000 字符）。
   *
   * 可选字段：后端还未提供时前端回退 `content`；
   * 搜索与高亮统一经 `clipSearchText` 取文本，避免两处口径分叉。
   */
  title?: string;
  /** 条目类型，与 Rust ClipKind 对齐；用字面量联合消除拼写错误静默通过的隐患。 */
  kind: "text" | "richtext" | "image";
  isFavorite: boolean;
  lastModifiedUtc: number;
  /**
   * 图片项是否带缩略图；文本项为 false。
   *
   * 缩略图字节不随列表传输（列表每次刷新都会重传一遍 base64），
   * 前端凭这个标志按需调 `getClipThumbnail`。
   */
  hasThumbnail: boolean;
  /** 图片项的原图 ID，用于调 getClipImageOriginal；文本项无此字段。 */
  imageId?: string;
  /** 富文本项的 HTML 串；纯文本项与图片项无此字段。 */
  htmlContent?: string;
}

/** 词典词条结构化内容，与 Rust DictEntry（camelCase）对齐。 */
export interface DictEntry {
  phonetic: string | null;
  definitions: PosDefinition[];
  examples: string[];
  audio: string | null;
  inflections: string[];
}

/** 按词性分组的释义，与 Rust PosDefinition 对齐。 */
export interface PosDefinition {
  pos: string | null;
  meanings: string[];
}

/**
 * 翻译结果，与 Rust TranslateResultDto（camelCase）对齐的可判别联合。
 *
 * `kind` 取值与后端 TranslateResponse 的 serde tag 一致：
 * - `"plain"`：普通译文，`translated` 为译文文本，无 `entry`。
 * - `"dict"`：结构化词条，`entry` 为词条；`translated` 为词条纯文本摘要（回退展示用）。
 */
export type TranslateResult = TranslatePlainResult | TranslateDictResult;

interface TranslateResultBase {
  translated: string;
  sourceLang: string;
  targetLang: string;
}

export interface TranslatePlainResult extends TranslateResultBase {
  kind: "plain";
}

export interface TranslateDictResult extends TranslateResultBase {
  kind: "dict";
  entry: DictEntry;
}

/** 翻译历史条目，与 Rust TranslateHistoryDto（camelCase）对齐。 */
export interface TranslateHistoryItem {
  id: string;
  sourceText: string;
  translatedText: string;
  sourceLang: string;
  targetLang: string;
  providerId: string;
  createdUtc: number;
}

/** 翻译 Provider 描述，与 Rust ProviderDto（camelCase）对齐。 */
export interface Provider {
  id: string;
  name: string;
  /** 是否需要 API Key（apiKey 语义）；仅表示有无密钥字段，不代表是否可配置。 */
  needsKey: boolean;
  /**
   * 是否有可配置字段（凭据 schema 非空）。
   * 独立于 needsKey：Ollama 本地无鉴权（needsKey=false）但有必填字段，needsConfig=true，
   * 前端据此显示「配置」按钮、纳入 configuredIds 判定，否则源不可用。
   */
  needsConfig: boolean;
  /** 是否为非官方/自建接口；前端据此渲染「非官方」标注与失败降级提示（设计文档§三.决策3）。 */
  isUnofficial: boolean;
}

/** 热键配置，与 Rust HotkeyDto（camelCase）对齐。 */
export interface Hotkeys {
  history: string;
  translate: string;
  main: string;
}

/** 热键动作类型，与 Rust HotkeyAction 的合法字符串值对齐。 */
export type HotkeyAction = "history" | "translate" | "main";

/**
 * 列表排序方式，与 Rust `SortOrder` 的 snake_case 序列化值对齐。
 *
 * - `recent`：最近复制优先（默认，与引入本设置之前的列表顺序一致）
 * - `first_copied`：首次复制优先（再次复制不改变位置）
 */
export type SortOrder = "recent" | "first_copied";

/**
 * 搜索命中高亮样式，与 Rust `HighlightStyle` 的 snake_case 序列化值对齐。
 *
 * 四档与 Maccy 2.6.1 的 `HighlightMatch`（color/bold/italic/underline）同名同默认：
 * - `bold`：加粗（**默认**，Maccy 的默认档）
 * - `color`：命中片段整段染色
 * - `italic`：斜体
 * - `underline`：下划线
 */
export type HighlightStyle = "color" | "bold" | "italic" | "underline";

/** 外观设置四项，与 Rust AppearanceSettingsDto（camelCase）对齐。 */
export interface AppearanceSettings {
  sortOrder: SortOrder;
  /** 弹出面板行内缩略图高度（px，1–200，默认 40；对齐 Maccy 的 imageMaxHeight） */
  imageHeightPx: number;
  /** 首次预览延迟（毫秒，200–100000，默认 1500；对齐 Maccy 的 previewDelay） */
  previewFirstDelayMs: number;
  highlightStyle: HighlightStyle;
}

/**
 * 将 invoke 的 reject 值（通常是 Rust 返回的 String）重抛为 Error。
 *
 * Tauri invoke 在 Rust 返回 Err(String) 时以字符串形式 reject，
 * 此辅助函数统一把它包成 Error，保留原始消息，使调用方可用 instanceof Error 判断。
 */
function toError(cause: unknown): Error {
  if (cause instanceof Error) {
    return cause;
  }
  return new Error(String(cause));
}

/** 列出所有未软删的剪贴板条目（收藏优先）。 */
export async function listClipItems(): Promise<ClipItem[]> {
  try {
    return await invoke<ClipItem[]>("list_clip_items");
  } catch (err) {
    throw toError(err);
  }
}

/** 软删指定剪贴板条目。 */
export async function deleteClipItem(id: string): Promise<void> {
  try {
    await invoke<void>("delete_clip_item", { id });
  } catch (err) {
    throw toError(err);
  }
}

/** 设置或取消指定剪贴板条目的收藏状态。 */
export async function toggleFavoriteClip(
  id: string,
  favorite: boolean
): Promise<void> {
  try {
    await invoke<void>("toggle_favorite_clip", { id, favorite });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 翻译文本，返回译文与语言方向。
 *
 * @param text - 待翻译文本
 * @param target - 可选目标语言代码（如 "en"、"zh"、"fr"），不传时由 Rust 侧自动检测
 * @param source - 可选源语言代码；不传或 undefined 时后端回退自动检测
 */
export async function translateText(
  text: string,
  target?: string,
  source?: string
): Promise<TranslateResult> {
  try {
    return await invoke<TranslateResult>("translate_text", { text, target, source });
  } catch (err) {
    throw toError(err);
  }
}

/** 按时间倒序列出翻译历史。 */
export async function listTranslateHistory(): Promise<TranslateHistoryItem[]> {
  try {
    return await invoke<TranslateHistoryItem[]>("list_translate_history");
  } catch (err) {
    throw toError(err);
  }
}

/** 读取热键配置，返回 { history, translate, main }。 */
export async function getHotkeys(): Promise<Hotkeys> {
  try {
    return await invoke<Hotkeys>("get_hotkeys");
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 将指定动作改绑到新加速键（含冲突检测）。
 *
 * @param action - 热键动作："history" | "translate" | "main"
 * @param accelerator - 加速键字符串，如 "CmdOrCtrl+Shift+H"
 */
export async function setHotkey(
  action: HotkeyAction,
  accelerator: string
): Promise<void> {
  try {
    await invoke<void>("set_hotkey", { action, accelerator });
  } catch (err) {
    throw toError(err);
  }
}

/** 读取排除应用名单。 */
export async function getExcludeList(): Promise<string[]> {
  try {
    return await invoke<string[]>("get_exclude_list");
  } catch (err) {
    throw toError(err);
  }
}

/** 写入排除应用名单（整体覆盖）。 */
export async function setExcludeList(list: string[]): Promise<void> {
  try {
    await invoke<void>("set_exclude_list", { list });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 应用名单模式：blacklist（名单内不记录）/ whitelist（只记录名单内）。
 *
 * 与 Rust 侧 `privacy::ExcludeMode` 的 serde 小写表示保持一致，改动需两端同步。
 */
export type ExcludeMode = "blacklist" | "whitelist";

/** 读取应用名单模式。 */
export async function getExcludeMode(): Promise<ExcludeMode> {
  try {
    return await invoke<ExcludeMode>("get_exclude_mode");
  } catch (err) {
    throw toError(err);
  }
}

/** 切换应用名单模式（保留已填的应用条目与内容正则）。 */
export async function setExcludeMode(mode: ExcludeMode): Promise<void> {
  try {
    await invoke<void>("set_exclude_mode", { mode });
  } catch (err) {
    throw toError(err);
  }
}

/** 读取内容忽略正则原文（编译失败的那条也会原样返回，便于用户修正）。 */
export async function getIgnoreRegexes(): Promise<string[]> {
  try {
    return await invoke<string[]>("get_ignore_regexes");
  } catch (err) {
    throw toError(err);
  }
}

/** 写入内容忽略正则（整体覆盖）。 */
export async function setIgnoreRegexes(patterns: string[]): Promise<void> {
  try {
    await invoke<void>("set_ignore_regexes", { patterns });
  } catch (err) {
    throw toError(err);
  }
}

/** 返回所有可用翻译 Provider 列表。 */
export async function getTranslateProviders(): Promise<Provider[]> {
  try {
    return await invoke<Provider[]>("get_translate_providers");
  } catch (err) {
    throw toError(err);
  }
}

/** 读取当前选中的翻译 Provider id。 */
export async function getSelectedProvider(): Promise<string> {
  try {
    return await invoke<string>("get_selected_provider");
  } catch (err) {
    throw toError(err);
  }
}

/** 设置翻译 Provider（Rust 侧校验 id 合法性）。 */
export async function setSelectedProvider(id: string): Promise<void> {
  try {
    await invoke<void>("set_selected_provider", { id });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 隐藏当前窗口并把前台焦点还给上一个外部 app（方案 C）。
 *
 * 替代裸 getCurrentWindow().hide()：后者只隐藏窗口、不把焦点交还触发处的 app。
 * 用于 popover 的 Esc 关闭路径，让用户关闭面板后焦点回到原应用。
 */
export async function hideAndReturnFocus(): Promise<void> {
  try {
    await invoke<void>("hide_and_return_focus");
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 按 id 把条目写入系统剪贴板（富文本条目带 HTML，纯文本兜底）。
 *
 * 后端命令 copy_clip_to_clipboard 仅写回剪贴板、不触发粘贴注入，
 * 供"复制"按钮调用以保真富文本格式（替代 navigator.clipboard.writeText）。
 *
 * @param id - 剪贴板条目 ID
 */
export async function copyClipToClipboard(id: string): Promise<void> {
  try {
    await invoke<void>("copy_clip_to_clipboard", { id });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 获取图片剪贴板条目的原图（PNG）data URL。
 * 降级或无原图时返回 null。
 *
 * @param imageId - 图片条目 ID
 * @returns 原图 data URL（data:image/png;base64,...）或 null
 */
export async function getClipImageOriginal(imageId: string): Promise<string | null> {
  try {
    return await invoke<string | null>("get_clip_image_original", { imageId });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 获取图片剪贴板条目的缩略图（WebP）data URL。
 *
 * 列表 payload 不带缩略图字节，前端渲染图片行时按 imageId 单独拉取。
 * 图片行不存在 / 已软删 / 缩略图为空时返回 null（前端保持占位，不当错误处理）。
 *
 * @param imageId - clip_images 行的 ID
 * @returns 缩略图 data URL（data:image/webp;base64,...）或 null
 */
export async function getClipThumbnail(imageId: string): Promise<string | null> {
  try {
    return await invoke<string | null>("get_clip_thumbnail", { imageId });
  } catch (err) {
    throw toError(err);
  }
}

/** 读取暂停捕获状态。 */
export async function getPauseCapture(): Promise<boolean> {
  try {
    return await invoke<boolean>("get_pause_capture");
  } catch (err) {
    throw toError(err);
  }
}

/** 设置暂停捕获状态（运行时生效 + 持久化）。 */
export async function setPauseCapture(paused: boolean): Promise<void> {
  try {
    await invoke<void>("set_pause_capture", { value: paused });
  } catch (err) {
    throw toError(err);
  }
}

/** 读取敏感内容跳过状态。 */
export async function getSkipSensitive(): Promise<boolean> {
  try {
    return await invoke<boolean>("get_skip_sensitive");
  } catch (err) {
    throw toError(err);
  }
}

/** 设置敏感内容跳过状态（运行时生效 + 持久化）。 */
export async function setSkipSensitive(skip: boolean): Promise<void> {
  try {
    await invoke<void>("set_skip_sensitive", { value: skip });
  } catch (err) {
    throw toError(err);
  }
}

/** 读取托盘驻留状态。 */
export async function getStayInTray(): Promise<boolean> {
  try {
    return await invoke<boolean>("get_stay_in_tray");
  } catch (err) {
    throw toError(err);
  }
}

/** 设置托盘驻留状态（运行时生效 + 持久化）。 */
export async function setStayInTray(enabled: boolean): Promise<void> {
  try {
    await invoke<void>("set_stay_in_tray", { value: enabled });
  } catch (err) {
    throw toError(err);
  }
}

/** 读取自动更新开关。 */
export async function getAutoUpdate(): Promise<boolean> {
  try {
    return await invoke<boolean>("get_auto_update");
  } catch (err) {
    throw toError(err);
  }
}

/** 设置自动更新开关。 */
export async function setAutoUpdate(enabled: boolean): Promise<void> {
  try {
    await invoke<void>("set_auto_update", { value: enabled });
  } catch (err) {
    throw toError(err);
  }
}

/** 读取单张图片阈值（字节）。默认 20971520（20 MiB）。 */
export async function getImageThreshold(): Promise<number> {
  try {
    return await invoke<number>("get_image_threshold");
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 设置单张图片阈值（字节）。
 *
 * 合法范围 1048576..=524288000（1 MiB..=500 MiB），越界 Rust 侧返回中文 Err。
 *
 * @param bytes - 阈值字节数
 */
export async function setImageThreshold(bytes: number): Promise<void> {
  try {
    await invoke<void>("set_image_threshold", { bytes });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 历史条目上限的合法区间（与 Rust 侧 `settings::MIN/MAX_HISTORY_ITEMS` 对齐）。
 *
 * 前端只做输入拦截（min/max + 提交前校验），真正生效的拒收在后端：
 * 输入框可以被脚本绕过，边界只有一处可信。
 */
export const MIN_HISTORY_ITEMS = 1;
export const MAX_HISTORY_ITEMS = 999;

/** 读取历史条目上限（默认 500，后端已收敛到 1–999）。 */
export async function getMaxHistoryItems(): Promise<number> {
  try {
    return await invoke<number>("get_max_history_items");
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 设置历史条目上限。
 *
 * 后端会在写入后**立刻**按新上限淘汰最旧的非收藏条目，并返回本轮淘汰条数；
 * 越界（< 1 或 > 999）时返回中文 Err 且不落盘。
 *
 * @param items - 上限条数
 * @returns 本次调用中被淘汰的历史条目数（未超限时为 0）
 */
export async function setMaxHistoryItems(items: number): Promise<number> {
  try {
    return await invoke<number>("set_max_history_items", { items });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 读取外观设置四项（排序方式 / 图片高度 / 首次预览延迟 / 命中高亮样式）。
 *
 * 一次 IPC 返回四项：弹窗每次打开都要读其中三项，分三次调用只会让打开变慢。
 * 数值项返回时已由后端收敛到合法区间；调用方仍应经 `normalizeAppearanceSettings`
 * 归一，避免旧版后端或缺字段时拿到 undefined。
 */
export async function getAppearanceSettings(): Promise<AppearanceSettings> {
  try {
    return await invoke<AppearanceSettings>("get_appearance_settings");
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 设置列表排序方式。
 *
 * 只影响列表展示顺序，不改变去重、收藏与淘汰语义；
 * 非法值（recent / first_copied 之外）由 Rust 侧拒收并返回中文 Err。
 */
export async function setSortOrder(order: SortOrder): Promise<void> {
  try {
    await invoke<void>("set_sort_order", { order });
  } catch (err) {
    throw toError(err);
  }
}

/** 设置弹出面板行内缩略图高度（px，合法 1–200，对齐 Maccy 的 imageMaxHeight）。 */
export async function setImageHeight(px: number): Promise<void> {
  try {
    await invoke<void>("set_image_height", { px });
  } catch (err) {
    throw toError(err);
  }
}

/** 设置首次预览延迟（毫秒，合法 200–100000，对齐 Maccy 的 previewDelay）。 */
export async function setPreviewFirstDelay(ms: number): Promise<void> {
  try {
    await invoke<void>("set_preview_first_delay", { ms });
  } catch (err) {
    throw toError(err);
  }
}

/** 设置搜索命中高亮样式（合法值：color / bold / italic / underline，与 Maccy 同名）。 */
export async function setHighlightStyle(style: HighlightStyle): Promise<void> {
  try {
    await invoke<void>("set_highlight_style", { style });
  } catch (err) {
    throw toError(err);
  }
}

/** 读取当前 UI 主题（"auto" | "light" | "dark"）。 */
export async function getTheme(): Promise<string> {
  try {
    return await invoke<string>("get_theme");
  } catch (err) {
    throw toError(err);
  }
}

/** 设置 UI 主题（合法值："auto" | "light" | "dark"）。 */
export async function setTheme(theme: string): Promise<void> {
  try {
    await invoke<void>("set_theme", { theme });
  } catch (err) {
    throw toError(err);
  }
}

/** 读取开机自启状态。 */
export async function getLaunchOnLogin(): Promise<boolean> {
  try {
    return await invoke<boolean>("get_launch_on_login");
  } catch (err) {
    throw toError(err);
  }
}

/** 设置开机自启（持久化 autostart.json + 应用到 OS）。 */
export async function setLaunchOnLogin(enabled: boolean): Promise<void> {
  try {
    await invoke<void>("set_launch_on_login", { value: enabled });
  } catch (err) {
    throw toError(err);
  }
}

/** 存储统计，与 Rust StorageStatsDto（camelCase）对齐。 */
export interface StorageStats {
  /** 活跃（未软删）条目数 */
  liveCount: number;
  /**
   * 图片占用逻辑字节（原图 + 缩略图，排除软删行）。
   *
   * 与后端分级清理的 500 MiB 阈值同源，是「是否超出上限」的唯一判据，进度条用它。
   */
  imageBytes: number;
  /**
   * 库文件体积（磁盘占用）。
   *
   * 与 `imageBytes` 不是同一个量：还含文本条目、索引、WAL 与未回收页；
   * SQLite 的 DELETE 只把页标记为可复用、不收缩文件，故清理后该值不会下降。
   */
  fileSizeBytes: number;
}

/** 历史清理结果，与 Rust CleanupResultDto（camelCase）对齐。 */
export interface CleanupResult {
  softDeleted: number;
  purged: number;
}

/** 粘贴路径类型。 */
export type PasteOutcome = "full_paste" | "write_back_only";

/** 粘贴结果，与 Rust PasteResultDto（camelCase）对齐。 */
export interface PasteResult {
  outcome: PasteOutcome;
}

/** 读取存储统计（活跃条目数 + 图片占用 + 库文件体积）。 */
export async function getStorageStats(): Promise<StorageStats> {
  try {
    return await invoke<StorageStats>("get_storage_stats");
  } catch (err) {
    throw toError(err);
  }
}

/** 清理历史（容量裁剪 + GC 物理删除）。 */
export async function cleanupHistory(): Promise<CleanupResult> {
  try {
    return await invoke<CleanupResult>("cleanup_history");
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 清空未固定历史：软删全部非收藏项并物理删除墓碑，返回本轮计数。
 *
 * 与 {@link cleanupHistory}（按用户配置的上限裁剪）的区别是保留上限为 0，
 * 即「非收藏项一条不留」；收藏项由后端查询条件豁免，前端不参与筛选。
 * 调用方必须先取得用户二次确认——本操作不可撤销。
 */
export async function clearUnpinnedHistory(): Promise<CleanupResult> {
  try {
    return await invoke<CleanupResult>("clear_unpinned_history");
  } catch (err) {
    throw toError(err);
  }
}

/** 打开 macOS 辅助功能系统设置深链。 */
export async function openAccessibilitySettings(): Promise<void> {
  try {
    await invoke<void>("open_accessibility_settings");
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 首次引导所需的辅助功能授权状态，与 Rust `OnboardingStatusDto` 对齐。
 *
 * 两个字段来自同一次探针判定，不要在调用方各自再判一遍授权：
 * `action` 决定向导是否还该请求授权，`capability` 决定粘贴会不会降级。
 */
export interface OnboardingStatus {
  /** `"proceed"`（已授权）或 `"show_accessibility_card"`（未授权） */
  action: "proceed" | "show_accessibility_card";
  /** `"full_paste"`（可模拟 ⌘V）或 `"write_back_only"`（仅写回剪贴板） */
  capability: PasteOutcome;
}

/**
 * 读取辅助功能授权状态（纯只读，无副作用）。
 *
 * 判定身份是可执行文件：开发构建与打包构建是两份不同的可执行文件，
 * TCC 授权互不复用，真机确认必须分别做。
 */
export async function getOnboardingStatus(): Promise<OnboardingStatus> {
  try {
    return await invoke<OnboardingStatus>("get_onboarding_status");
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 将指定条目写回系统剪贴板（降级实现）。
 *
 * @param id - 剪贴板条目 ID
 * @returns 粘贴结果，outcome 为 "write_back_only"（当前实现不模拟 ⌘V 注入）
 */
export async function pasteToFront(id: string): Promise<PasteResult> {
  try {
    return await invoke<PasteResult>("paste_to_front", { id });
  } catch (err) {
    throw toError(err);
  }
}

/** 凭据字段 schema，与 Rust CredentialFieldDto（camelCase）对齐。 */
export interface CredentialField {
  key: string;
  label: string;
  isSecret: boolean;
  required: boolean;
}

/** 凭据当前值，与 Rust CredentialValueDto（camelCase）对齐。secret 字段 value 永远 null，只看 isSet。 */
export interface CredentialValue {
  key: string;
  value: string | null;
  isSet: boolean;
}

/**
 * 获取指定 Provider 的凭据字段 schema。
 *
 * 未知 providerId 返回空数组。
 */
export async function getProviderCredentialSchema(
  providerId: string
): Promise<CredentialField[]> {
  try {
    return await invoke<CredentialField[]>("get_provider_credential_schema", { providerId });
  } catch (err) {
    throw toError(err);
  }
}

/** 获取指定 Provider 已存储的凭据值列表。secret 字段 value 永远 null，通过 isSet 判断是否已设置。 */
export async function getProviderCredentials(
  providerId: string
): Promise<CredentialValue[]> {
  try {
    return await invoke<CredentialValue[]>("get_provider_credentials", { providerId });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 保存指定 Provider 的凭据值。
 *
 * @param providerId - Provider ID
 * @param values - 键值对对象（Record<string,string>），后端映射为 HashMap；不传空串以避免覆盖已有 secret。
 */
export async function setProviderCredentials(
  providerId: string,
  values: Record<string, string>
): Promise<void> {
  try {
    await invoke<void>("set_provider_credentials", { providerId, values });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 清除指定 Provider 的所有已保存凭据（keychain + DB 均清）。
 *
 * 幂等——凭据不存在时也不报错。
 * 成功后后端会 emit provider-config-changed 事件使翻译页刷新状态。
 *
 * @param providerId - Provider ID
 */
export async function deleteProviderCredentials(providerId: string): Promise<void> {
  try {
    await invoke<void>("delete_provider_credentials", { providerId });
  } catch (err) {
    throw toError(err);
  }
}

/** 检查更新结果，与 Rust CheckUpdateResult（camelCase）对齐。 */
export interface CheckUpdateResult {
  /** 是否有可用新版本 */
  available: boolean;
  /** 新版本号（有更新时），无更新时为空串 */
  version: string;
  /** 当前已安装版本号 */
  currentVersion: string;
}

/**
 * 手动检查是否有可用的应用更新。
 *
 * endpoint 已是真实地址；网络/清单异常时会 reject，调用方应以友好文案展示错误。
 */
export async function checkForUpdates(): Promise<CheckUpdateResult> {
  try {
    return await invoke<CheckUpdateResult>("check_for_updates");
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 下载并安装可用更新（不重启）。
 *
 * 调用 Rust 侧 `download_and_install_update`。下载/安装失败时 reject，
 * 调用方应以友好文案展示错误；成功后需经 `restartApp` 重启使更新生效。
 */
export async function downloadAndInstallUpdate(): Promise<void> {
  try {
    await invoke<void>("download_and_install_update");
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 重启应用以应用已下载安装的更新。
 *
 * 调用 Rust 侧 `restart_app`（内部 `app.restart()`，进程替换重启）。
 * 正常路径下进程随即终止，Promise 通常不会 resolve。
 */
export async function restartApp(): Promise<void> {
  try {
    await invoke<void>("restart_app");
  } catch (err) {
    throw toError(err);
  }
}

/** 备份导出结果，与 Rust ExportBackupDto（camelCase）对齐。 */
export interface ExportBackupResult {
  /** 备份包落盘的绝对路径（由后端选定目录） */
  path: string;
  /** 写入备份包的条目数 */
  itemCount: number;
  /** 因判重信息不可得而未纳入备份的条目数 */
  skippedCount: number;
  /** 是否因文稿/下载目录不可用而回退到了应用数据目录 */
  usedFallbackDir: boolean;
}

/** 备份导入结果，与 Rust ImportBackupDto（camelCase）对齐。 */
export interface ImportBackupResult {
  /** 新插入的条目数 */
  inserted: number;
  /** 因库内已存在相同内容/图片而跳过的条目数 */
  skipped: number;
}

/**
 * 导出加密备份包。
 *
 * 落盘目录由后端决定（文稿 → 下载 → 应用数据目录），返回值里带绝对路径供展示。
 * 口令只作为参数传入，既不出现在返回值，也不出现在错误消息里。
 *
 * @param passphrase - 备份口令（导入时需输入同一口令）
 */
export async function exportBackup(passphrase: string): Promise<ExportBackupResult> {
  try {
    return await invoke<ExportBackupResult>("export_backup", { passphrase });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 导入加密备份包并合并进当前历史。
 *
 * 后端先完成解密与解析，再在 SAVEPOINT 内合并；失败时库内数据保持导入前状态。
 * 口令错误会以「备份口令错误，或文件已损坏」reject。
 *
 * @param blobBase64 - 备份文件原始字节的 base64
 * @param passphrase - 该备份包的加密口令
 */
export async function importBackup(
  blobBase64: string,
  passphrase: string
): Promise<ImportBackupResult> {
  try {
    return await invoke<ImportBackupResult>("import_backup", { blobBase64, passphrase });
  } catch (err) {
    throw toError(err);
  }
}

/**
 * 在系统文件管理器中定位备份文件。
 *
 * @param path - 导出返回的备份包绝对路径
 */
export async function revealBackupInFinder(path: string): Promise<void> {
  try {
    await invoke<void>("reveal_backup_in_finder", { path });
  } catch (err) {
    throw toError(err);
  }
}
