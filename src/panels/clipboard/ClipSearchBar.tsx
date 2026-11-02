/**
 * 剪贴板页顶部搜索栏：搜索输入框 + 类型筛选 + 命中计数。
 * 键盘事件透传到父组件处理（↑↓ 高亮、Enter 选中等）。
 *
 * 类型筛选保留公共 Select（可访问下拉：trigger + role="option" 选项列表），
 * 不使用分段控件——筛选的交互契约与键盘可达性由 Select 保证，样式统一由
 * .qq-select 一套承担；`--surface-2` 容器 + 选中项抬升的内描边语汇在 Select.css 内。
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
  /** 搜索模式提示（如正则写法非法降级为文本搜索）；null / 缺省不显示 */
  notice?: string | null;
  /** 当前可见条数 / 总条数；总条数为 0（空库）时不显示计数 */
  visibleCount?: number;
  totalCount?: number;
  onSearchChange: (query: string) => void;
  onTypeFilterChange: (filter: HistoryFilter) => void;
  onKeyDown: (event: React.KeyboardEvent) => void;
}

/** 搜索栏子组件：搜索框 + 类型筛选下拉 + 模式提示 + 命中计数 */
export function ClipSearchBar({
  searchQuery,
  typeFilter,
  notice,
  visibleCount,
  totalCount,
  onSearchChange,
  onTypeFilterChange,
  onKeyDown,
}: ClipSearchBarProps) {
  const showCount =
    visibleCount !== undefined && totalCount !== undefined && totalCount > 0;

  return (
    <>
      <div className="clip-search">
        <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="11" cy="11" r="7" />
          <path d="M20.5 20.5 16 16" />
        </svg>
        <input
          className="clip-search-input body-6"
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
      <div className="clip-filters">
        <Select
          className="clip-type-select"
          ariaLabel="类型筛选"
          value={typeFilter}
          options={FILTER_OPTIONS}
          onChange={(value) => onTypeFilterChange(value as HistoryFilter)}
        />
        {showCount && (
          <span className="clip-count caption-4 mono" aria-live="off">
            {`${visibleCount} / ${totalCount}`}
          </span>
        )}
      </div>
      {notice !== null && notice !== undefined && (
        // role="status"：降级这类「用户需要知道」的状态变化交给读屏播报，不靠颜色
        <p className="clip-notice caption-2" role="status">
          {notice}
        </p>
      )}
    </>
  );
}
