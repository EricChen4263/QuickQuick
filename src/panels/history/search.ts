/**
 * 历史条目类型定义与实时搜索过滤逻辑。
 * 纯函数，不可变——所有函数返回新数组，不修改入参。
 * 对应验收项 V1-F2-A09。
 */

/** 历史记录条目 */
export interface HistoryItem {
  id: string;
  text: string;
  kind: "text" | "richtext";
}

/**
 * 按搜索词过滤历史条目列表。
 * 大小写不敏感子串匹配；空 query 或纯空白 query 返回全部。
 *
 * @param items - 待过滤的原始列表（不会被修改）
 * @param query - 用户输入的搜索词
 * @returns 匹配的条目组成的新数组
 */
export function filterBySearch(items: HistoryItem[], query: string): HistoryItem[] {
  const trimmed = query.trim();
  if (trimmed === "") {
    return [...items];
  }
  const lower = trimmed.toLowerCase();
  return items.filter((item) => item.text.toLowerCase().includes(lower));
}
