/**
 * 剪贴板页顶部搜索栏：搜索输入框 + 类型筛选下拉。
 * 键盘事件透传到父组件处理（↑↓ 高亮、Enter 选中等）。
 */

import type { HistoryFilter } from "../history/filter";

/** 类型筛选选项的显示标签 */
const FILTER_LABELS: Record<HistoryFilter, string> = {
  all: "全部",
  text: "纯文本",
  richtext: "富文本",
  image: "图片",
};

interface ClipSearchBarProps {
  searchQuery: string;
  typeFilter: HistoryFilter;
  onSearchChange: (query: string) => void;
  onTypeFilterChange: (filter: HistoryFilter) => void;
  onKeyDown: (event: React.KeyboardEvent) => void;
}

/** 搜索栏子组件：搜索框 + 类型筛选下拉 */
export function ClipSearchBar({
  searchQuery,
  typeFilter,
  onSearchChange,
  onTypeFilterChange,
  onKeyDown,
}: ClipSearchBarProps) {
  return (
    <div style={{ display: "flex", gap: "8px", padding: "8px" }}>
      <input
        type="search"
        role="searchbox"
        placeholder="搜索剪贴板内容…"
        autoFocus
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
        onKeyDown={onKeyDown}
        style={{ flex: 1 }}
      />
      <select
        value={typeFilter}
        onChange={(e) => onTypeFilterChange(e.target.value as HistoryFilter)}
        aria-label="类型筛选"
      >
        {(Object.keys(FILTER_LABELS) as HistoryFilter[]).map((key) => (
          <option key={key} value={key}>
            {FILTER_LABELS[key]}
          </option>
        ))}
      </select>
    </div>
  );
}
