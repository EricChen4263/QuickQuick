import type { ClipItem } from "../ipc/ipc-client";

/**
 * 从剪贴板条目列表中取首条可译文本。
 *
 * 仅 text / richtext 类型可译；image 无论 content 是否为空一律返回 null。
 * 其余情况（空数组、content 纯空白）也返回 null。
 */
export function pickLatestText(items: ClipItem[]): string | null {
  if (items.length === 0) return null;
  const item = items[0];
  if (item.kind !== "text" && item.kind !== "richtext") return null;
  const content = item.content.trim();
  return content.length > 0 ? content : null;
}
