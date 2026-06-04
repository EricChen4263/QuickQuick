/**
 * 剪贴板页顶部搜索栏：搜索输入框 + 类型筛选下拉。
 * 键盘事件透传到父组件处理（↑↓ 高亮、Enter 选中等）。
 */

import { Select } from "../../components/Select";
import type { SelectOption } from "../../components/Select";
import type { HistoryFilter } from "../history/filter";

/** 类型筛选选项的显示标签 */
const FILTER_LABELS: Record<HistoryFilter, string> = {
  all: "全部",
  text: "纯文本",
  richtext: "富文本",
  image: "图片",
};

/** 类型筛选下拉选项（顺序即 FILTER_LABELS 的声明序） */
const FILTER_OPTIONS: SelectOption[] = (Object.keys(FILTER_LABELS) as HistoryFilter[]).map(
  (key) => ({ value: key, label: FILTER_LABELS[key] })
);

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
    <div className="searchbar">
      <div className="search-field">
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <circle cx="11" cy="11" r="7" />
          <path d="m21 21-4.3-4.3" />
        </svg>
        <input
          type="search"
          role="searchbox"
          placeholder="搜索剪贴板内容…"
          aria-label="搜索剪贴板内容"
          autoFocus
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          onKeyDown={onKeyDown}
        />
      </div>
      <Select
        ariaLabel="类型筛选"
        value={typeFilter}
        options={FILTER_OPTIONS}
        onChange={(value) => onTypeFilterChange(value as HistoryFilter)}
      />
    </div>
  );
}
