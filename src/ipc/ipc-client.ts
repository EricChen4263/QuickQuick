import { invoke } from "@tauri-apps/api/core";

/** 剪贴板条目，与 Rust ClipItemDto（camelCase）对齐。 */
export interface ClipItem {
  id: string;
  content: string;
  kind: string;
  isFavorite: boolean;
  lastModifiedUtc: number;
}

/** 翻译结果，与 Rust TranslateResultDto（camelCase）对齐。 */
export interface TranslateResult {
  translated: string;
  sourceLang: string;
  targetLang: string;
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
 */
export async function translateText(
  text: string,
  target?: string
): Promise<TranslateResult> {
  try {
    return await invoke<TranslateResult>("translate_text", { text, target });
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
