/**
 * 键盘流纯逻辑——高亮索引计算、快选映射、Enter 选中。
 * 纯函数，无 DOM 副作用，不移动真实焦点。
 * 对应验收项 V1-F2-A12。
 *
 * 设计语义（§八#4 Spotlight 键盘模型）：
 * 焦点始终保持在搜索框，↑↓ 只更新高亮索引，不触碰 DOM 焦点。
 * 高亮状态由调用方（React 组件）持有并传入，此处只负责计算下一个值。
 */

import type { HistoryItem } from "./search";

/**
 * 键盘事件在输入法组词期间可供判断的最小结构。
 * 只取两个字段而非完整 KeyboardEvent，便于脱离 DOM 单测。
 */
export interface ImeKeyboardLike {
  isComposing?: boolean;
  keyCode?: number;
}

/**
 * 判断按键是否发生在输入法组词（composition）期间。
 * 组词期间的方向键/数字键属于输入法自身的候选词交互，调用方必须跳过列表导航与直达逻辑。
 *
 * 两种平台表现都要覆盖：
 * - `isComposing === true`：标准 KeyboardEvent 字段，Chromium 系 webview 可靠；
 * - `keyCode === 229`：Safari/WKWebView 在组词中的经典表现，此时 isComposing 可能仍为 false。
 *
 * @param event - 原生键盘事件（或其可观测字段）
 * @returns 组词期间返回 true
 */
export function isImeComposing(event: ImeKeyboardLike): boolean {
  return event.isComposing === true || event.keyCode === 229;
}

/**
 * 数字直达：把数字键字符映射为目标列表索引（0-based）。
 * `"1"`..`"9"` → 0..8，`"0"` → 9（第 10 条）。
 *
 * 条目数不足该序号时返回 null——数字直达**不做**「就近取最后一条」的降级，
 * 按到不存在的序号就什么也不取。
 *
 * @param digit     - 键盘按下的字符（如 event.key）
 * @param itemCount - 当前可见条目总数
 * @returns 0-based 目标索引，或 null（非数字 / 序号越界）
 */
export function directPickIndex(digit: string, itemCount: number): number | null {
  if (digit.length !== 1) {
    return null;
  }
  const num = parseInt(digit, 10);
  if (Number.isNaN(num) || num < 0 || num > 9) {
    return null;
  }
  const index = num === 0 ? 9 : num - 1;
  return index < itemCount ? index : null;
}

/**
 * 键盘修饰键集合（只取数字直达关心的四个布尔位）。
 */
export interface KeyboardModifiers {
  metaKey?: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
}

/**
 * 数字直达的前置条件：搜索框为空且未按任何修饰键。
 * 否则数字要能正常输入进搜索框（例如搜索 "2"），或被修饰键组合的既有语义接管
 * （主窗 ⌘1-9 是快速粘贴）。
 *
 * 空判定按字面长度：只有一个空格也算「搜索框非空」，此时数字照常输入。
 *
 * @param query     - 当前搜索框内容
 * @param modifiers - 按键携带的修饰键状态
 * @returns 满足数字直达条件返回 true
 */
export function isDirectPickAllowed(query: string, modifiers: KeyboardModifiers): boolean {
  if (query !== "") {
    return false;
  }
  return !modifiers.metaKey && !modifiers.ctrlKey && !modifiers.altKey && !modifiers.shiftKey;
}

/**
 * 文本编辑结果：编辑后的完整值与编辑后的光标位置（均按 UTF-16 码元计）。
 */
export interface TextEdit {
  value: string;
  caret: number;
}

/**
 * 词边界分隔符：空白与常见中英文标点。
 * 删词只做到「按空白/标点切分」，不做真正的分词。
 */
const WORD_SEPARATOR_PATTERN =
  /[\s、。，．；：！？…—～·「」『』（）【】《》〈〉“”‘’,.;:!?/\\()[\]{}"'`]/;

/**
 * 把光标位置收敛到 [0, length]，防止调用方传入越界值。
 */
function clampCaret(caret: number, length: number): number {
  if (caret < 0) return 0;
  return Math.min(caret, length);
}

/**
 * 删除光标前的一个「词」（对应 ⌃W，readline/vim 语义）。
 * 先跳过紧邻光标前的连续分隔符，再吃掉一整段非分隔符：
 * - 英文按空格与标点切分，`"hello world|"` → `"hello |"`；
 * - 中文按标点切分，`"你好，世界|"` → `"你好，|"`；
 * - 光标在开头时原样返回。
 *
 * @param value - 当前输入框内容
 * @param caret - 当前光标位置
 * @returns 编辑后的内容与光标位置
 */
export function applyDeleteWord(value: string, caret: number): TextEdit {
  const from = clampCaret(caret, value.length);
  let start = from;
  while (start > 0 && WORD_SEPARATOR_PATTERN.test(value[start - 1])) start--;
  while (start > 0 && !WORD_SEPARATOR_PATTERN.test(value[start - 1])) start--;
  return { value: value.slice(0, start) + value.slice(from), caret: start };
}

/**
 * 删除光标前的一个字符（对应 ⌃H，等价 Backspace）。
 * 代理对（emoji 等）整对删除，避免留下半个码元。
 *
 * @param value - 当前输入框内容
 * @param caret - 当前光标位置
 * @returns 编辑后的内容与光标位置
 */
export function applyDeleteChar(value: string, caret: number): TextEdit {
  const from = clampCaret(caret, value.length);
  if (from === 0) {
    return { value, caret: 0 };
  }
  const previous = value.charCodeAt(from - 1);
  const isLowSurrogate = previous >= 0xdc00 && previous <= 0xdfff;
  const isSurrogatePair =
    isLowSurrogate && from >= 2 && value.charCodeAt(from - 2) >= 0xd800 && value.charCodeAt(from - 2) <= 0xdbff;
  const start = isSurrogatePair ? from - 2 : from - 1;
  return { value: value.slice(0, start) + value.slice(from), caret: start };
}

/**
 * vim 风格控制键 → 列表移动方向（Maccy 同款键位）。
 * `⌃N` / `⌃J` 下移一条，`⌃P` / `⌃K` 上移一条。
 *
 * @param key - 小写化的按键字符（如 event.key.toLowerCase()）
 * @returns 移动方向，非移动键返回 null
 */
export function ctrlMoveDirection(key: string): "ArrowUp" | "ArrowDown" | null {
  switch (key) {
    case "n":
    case "j":
      return "ArrowDown";
    case "p":
    case "k":
      return "ArrowUp";
    default:
      return null;
  }
}

/**
 * 列表首条索引。空列表返回 -1（无有效条目）。
 *
 * @param count - 当前可见条目总数
 */
export function firstIndex(count: number): number {
  return count === 0 ? -1 : 0;
}

/**
 * 列表末条索引。空列表返回 -1（无有效条目）。
 *
 * @param count - 当前可见条目总数
 */
export function lastIndex(count: number): number {
  return count === 0 ? -1 : count - 1;
}

/**
 * 根据方向键计算下一个高亮索引。
 * 结果 clamp 在 [0, count-1]，边界不越界。
 * count=0 时返回 -1，表示列表为空、无有效高亮。
 *
 * @param current - 当前高亮索引
 * @param key     - 按下的方向键
 * @param count   - 当前列表条目总数
 * @returns 下一个高亮索引
 */
export function moveHighlight(
  current: number,
  key: "ArrowUp" | "ArrowDown",
  count: number,
): number {
  if (count === 0) {
    return -1;
  }
  const delta = key === "ArrowDown" ? 1 : -1;
  const next = current + delta;
  return Math.min(Math.max(next, 0), count - 1);
}

/**
 * 将 Cmd+1~9 的数字键字符映射为列表索引（0-based）。
 * 接收单字符字符串 "1".."9"，返回对应的 0..8 索引。
 * 其余输入（"0"、多字符、非数字等）返回 null。
 *
 * 调用方负责检测 Cmd 修饰键是否同时按下，此函数只做数字→索引映射。
 *
 * @param digit - 键盘按下的字符（如 event.key）
 * @returns 0-based 列表索引，或 null（不适用）
 */
export function quickSelectIndex(digit: string): number | null {
  if (digit.length !== 1) {
    return null;
  }
  const num = parseInt(digit, 10);
  if (isNaN(num) || num < 1 || num > 9) {
    return null;
  }
  return num - 1;
}

/**
 * 根据当前高亮索引从列表中取出选中条目（Enter 确认语义）。
 * 索引越界或列表为空时返回 null。
 *
 * @param highlight - 当前高亮索引
 * @param items     - 当前展示的过滤后列表
 * @returns 选中的历史条目，或 null（无效选中）
 */
export function resolveEnter(highlight: number, items: HistoryItem[]): HistoryItem | null {
  if (highlight < 0 || highlight >= items.length) {
    return null;
  }
  return items[highlight];
}
