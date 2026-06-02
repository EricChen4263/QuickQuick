import type { ClipItem } from "../ipc/ipc-client";

/**
 * 从剪贴板条目列表中取首条可译文本。
 *
 * 仅返回 items[0] 的 content（trim 后非空），其余情况返回 null：
 * - 空数组
 * - [0] 是图片项（content 为空串）
 * - [0] 的文本 trim 后为空
 */
export function pickLatestText(items: ClipItem[]): string | null {
  if (items.length === 0) return null;
  const content = items[0].content.trim();
  return content.length > 0 ? content : null;
}
