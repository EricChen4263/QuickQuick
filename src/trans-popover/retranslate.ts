/**
 * 获焦重读去重逻辑。
 *
 * shouldRetranslate: 判断是否需要重新翻译——newText 为 null 或与上次相同时跳过，
 * 避免窗口 focus 事件触发多余重译（Batch C2 设计决策）。
 */

/**
 * 判断是否应触发重译。
 *
 * @param newText - 剪贴板最新文本，null 表示无可译内容
 * @param lastText - 上次已译的文本，null 表示首次
 * @returns 需要重译时返回 true
 */
export function shouldRetranslate(
  newText: string | null,
  lastText: string | null,
): boolean {
  if (newText === null) return false;
  return newText !== lastText;
}
