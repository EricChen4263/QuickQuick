/**
 * 历史条目类型定义与实时搜索过滤逻辑。
 * 纯函数，不可变——所有函数返回新数组，不修改入参。
 * 对应验收项 V1-F2-A09。
 */

import {
  DEFAULT_SEARCH_MODE,
  matchTextWithPlan,
  prepareQuery,
} from "../../clip-popover/clip-search";

/** 历史记录条目 */
export interface HistoryItem {
  id: string;
  text: string;
  kind: "text" | "richtext" | "image";
}

/**
 * 按搜索词过滤历史条目列表。
 *
 * 匹配语义与剪贴板搜索同源（委托 `clip-search` 的 mixed 模式）：
 * 多词 AND、大小写不敏感、整串看起来像正则时按正则处理，命中判定只有一份实现。
 * 查询计划只准备一次，逐条目复用。
 *
 * @param items - 待过滤的原始列表（不会被修改）
 * @param query - 用户输入的搜索词；空串或纯空白返回全部
 * @returns 匹配的条目组成的新数组
 */
export function filterBySearch(items: HistoryItem[], query: string): HistoryItem[] {
  const plan = prepareQuery(query, DEFAULT_SEARCH_MODE);
  if (plan.isEmpty) {
    return [...items];
  }
  return items.filter((item) => matchTextWithPlan(item.text, plan).matched);
}
