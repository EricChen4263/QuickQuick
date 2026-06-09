import { useEffect, useRef } from "react";
import type { ClipItem } from "../ipc/ipc-client";
import { KindIcon } from "../components/KindIcon";
import type { ClipGroups } from "./grouping";

interface PopoverListProps {
  groups: ClipGroups;
  selectedId: string | null;
  onSelect: (item: ClipItem) => void;
}

/** 内容摘要最大字符数（超出截断加省略号） */
const SUMMARY_MAX = 50;

function truncate(text: string): string {
  if (text.length <= SUMMARY_MAX) return text;
  return text.slice(0, SUMMARY_MAX) + "…";
}

/** 单行条目：类型图标 + 内容摘要/缩略图 + 收藏星标 */
function PopoverRow({
  item,
  isSelected,
  onSelect,
}: {
  item: ClipItem;
  isSelected: boolean;
  onSelect: (item: ClipItem) => void;
}) {
  return (
    <div
      role="option"
      aria-selected={isSelected}
      className="popover-row"
      onClick={() => onSelect(item)}
    >
      <div className="popover-row-icon">
        <KindIcon kind={item.kind} />
      </div>
      <div className="popover-row-main">
        {item.kind === "image" && item.thumbnailDataUrl ? (
          <img
            src={item.thumbnailDataUrl}
            alt="缩略图"
            className="popover-row-thumb"
          />
        ) : (
          <div className={item.kind === "richtext" ? "popover-row-text code" : "popover-row-text"}>
            {item.kind === "image" ? "[图片]" : truncate(item.content)}
          </div>
        )}
      </div>
      {item.isFavorite && (
        <div className="popover-row-fav" aria-label="已收藏">
          <svg viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
          </svg>
        </div>
      )}
    </div>
  );
}

/** 分组标题行 */
function GroupHeader({ label }: { label: string }) {
  return <div className="popover-group-header">{label}</div>;
}

/** 渲染单个分组（有条目才显示） */
function PopoverGroup({
  label,
  items,
  selectedId,
  onSelect,
}: {
  label: string;
  items: ClipItem[];
  selectedId: string | null;
  onSelect: (item: ClipItem) => void;
}) {
  if (items.length === 0) return null;
  return (
    <>
      <GroupHeader label={label} />
      {items.map((item) => (
        <PopoverRow
          key={item.id}
          item={item}
          isSelected={item.id === selectedId}
          onSelect={onSelect}
        />
      ))}
    </>
  );
}

/**
 * Popover 左侧列表区：收藏 / 今天 / 更早三组，支持鼠标点击选中。
 * selectedId / onSelect 由 ClipPopoverApp 持有，B2 键盘流可直接复用同一状态。
 * 扁平遍历顺序：favorites → today → earlier（与 visibleFlatList 保持一致）。
 */
export function PopoverList({ groups, selectedId, onSelect }: PopoverListProps) {
  const { favorites, today, earlier } = groups;
  const isEmpty = favorites.length === 0 && today.length === 0 && earlier.length === 0;
  const listRef = useRef<HTMLDivElement>(null);

  // 键盘 ↑/↓ 改 selectedId 后，把选中行滚入可视区（block:nearest 只在越界时滚动，不打断已可见项）
  useEffect(() => {
    if (selectedId === null) return;
    const selectedRow = listRef.current?.querySelector('[aria-selected="true"]');
    selectedRow?.scrollIntoView({ block: "nearest" });
  }, [selectedId]);

  if (isEmpty) {
    return (
      <div role="listbox" aria-label="剪贴板历史" className="popover-list popover-list-empty">
        <span>剪贴板暂无内容</span>
      </div>
    );
  }

  return (
    <div ref={listRef} role="listbox" aria-label="剪贴板历史" className="popover-list">
      <PopoverGroup label="收藏" items={favorites} selectedId={selectedId} onSelect={onSelect} />
      <PopoverGroup label="今天" items={today} selectedId={selectedId} onSelect={onSelect} />
      <PopoverGroup label="更早" items={earlier} selectedId={selectedId} onSelect={onSelect} />
    </div>
  );
}
