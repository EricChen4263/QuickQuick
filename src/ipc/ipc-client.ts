import { invoke } from "@tauri-apps/api/core";

/** 剪贴板条目，与 Rust ClipItemDto（camelCase）对齐。 */
export interface ClipItem {
  id: string;
  content: string;
  /** 条目类型，与 Rust ClipKind 对齐；用字面量联合消除拼写错误静默通过的隐患。 */
  kind: "text" | "richtext" | "image";
  isFavorite: boolean;
  lastModifiedUtc: number;
  /** 图片项的缩略图 data URL（data:image/webp;base64,...），文本项无此字段。 */
  thumbnailDataUrl?: string;
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
  needsKey: boolean;
  /** 是否为非官方/自建接口；前端据此渲染「非官方」标注与失败降级提示（设计文档§三.决策3）。 */
  isUnofficial: boolean;
}

/** 热键配置，与 Rust HotkeyDto（camelCase）对齐。 */
export interface Hotkeys {
  history: string;
  translate: string;
}

/** 热键动作类型，与 Rust HotkeyAction 的合法字符串值对齐。 */
export type HotkeyAction = "history" | "translate";

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

/** 读取热键配置，返回 { history, translate }。 */
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
 * @param action - 热键动作："history" | "translate"
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
  liveCount: number;
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

/** 读取存储统计（活跃条目数 + 库文件大小）。 */
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

/** 打开 macOS 辅助功能系统设置深链。 */
export async function openAccessibilitySettings(): Promise<void> {
  try {
    await invoke<void>("open_accessibility_settings");
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
