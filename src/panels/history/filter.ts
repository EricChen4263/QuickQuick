/**
 * 类型筛选过滤逻辑。
 * 纯函数，不可变——返回新数组，不修改入参。
 * 对应验收项 V1-F2-A10。
 */

import type { HistoryItem } from "./search";

/** 历史面板类型筛选选项 */
export type HistoryFilter = "all" | "text" | "richtext";

/**
 * 按类型筛选历史条目列表。
 * "all" 返回全部；"text" 仅返回纯文本；"richtext" 仅返回富文本。
 *
 * @param items - 待筛选的原始列表（不会被修改）
 * @param filter - 筛选类型
 * @returns 符合筛选条件的条目组成的新数组
 */
export function filterByType(items: HistoryItem[], filter: HistoryFilter): HistoryItem[] {
  if (filter === "all") {
    return [...items];
  }
  return items.filter((item) => item.kind === filter);
}
