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
