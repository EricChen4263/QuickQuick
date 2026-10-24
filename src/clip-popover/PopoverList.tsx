import { useEffect, useRef } from "react";
import type { ClipItem } from "../ipc/ipc-client";
import { KindIcon } from "../components/KindIcon";
import { useClipItemThumbnail } from "../panels/clipboard/useClipThumbnail";
import { clipSearchText, type SearchRange } from "./clip-search";
import { HighlightedText } from "./HighlightedText";
import type { ClipGroups } from "./grouping";
import type { InputOwnership } from "./keyboard-nav";

interface PopoverListProps {
  groups: ClipGroups;
  selectedId: string | null;
  /** 条目 id → 命中区间；缺省表示无搜索条件，整段按普通文本渲染 */
  hitRanges?: ReadonlyMap<string, SearchRange[]>;
  onSelect: (item: ClipItem) => void;
  /** 悬停与键盘的主导权仲裁器：只有鼠标真实位移过，悬停才允许改选中项 */
  ownership: InputOwnership;
}

/** 内容摘要最大字符数（超出截断加省略号） */
const SUMMARY_MAX = 50;

function truncate(text: string): string {
  if (text.length <= SUMMARY_MAX) return text;
  return text.slice(0, SUMMARY_MAX) + "…";
}

/** 图片行的缩略图：按需拉取，未就绪或取不到时保持既有 `[图片]` 占位 */
function PopoverRowImage({ item }: { item: ClipItem }) {
  const thumbnailUrl = useClipItemThumbnail(item);
  if (thumbnailUrl === null) {
    return <div className="popover-row-text">[图片]</div>;
  }
  return (
    <img
      src={thumbnailUrl}
      alt="缩略图"
      className="popover-row-thumb"
    />
  );
}

/** 单行条目：类型图标 + 内容摘要/缩略图 + 收藏星标 */
function PopoverRow({
  item,
  isSelected,
  hitRanges,
  onSelect,
  onHoverSelect,
}: {
  item: ClipItem;
  isSelected: boolean;
  hitRanges?: SearchRange[];
  onSelect: (item: ClipItem) => void;
  /** 指针停在本行：是否据此改选中项由上层按主导权仲裁，行本身不做判断 */
  onHoverSelect: (item: ClipItem) => void;
}) {
  return (
    <div
      role="option"
      aria-selected={isSelected}
      className="popover-row"
      onClick={() => onSelect(item)}
      onMouseEnter={() => onHoverSelect(item)}
      // 真实移动往往先进入某行（进入事件）再产生移动事件，所以两个事件都要接：
      // 只有移动事件才能让仲裁器确认「鼠标真的动了」，进入事件负责本行的选中。
      onMouseMove={() => onHoverSelect(item)}
    >
      <div className="popover-row-icon">
        <KindIcon kind={item.kind} />
      </div>
      <div className="popover-row-main">
        {item.kind === "image" ? (
          <PopoverRowImage item={item} />
        ) : (
          <div className={item.kind === "richtext" ? "popover-row-text code" : "popover-row-text"}>
            {/* 先按既有上限截断再切分高亮：区间落在被截掉的部分时会被自动丢弃 */}
            <HighlightedText text={truncate(clipSearchText(item))} ranges={hitRanges} />
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
  hitRanges,
  onSelect,
  onHoverSelect,
}: {
  label: string;
  items: ClipItem[];
  selectedId: string | null;
  hitRanges?: ReadonlyMap<string, SearchRange[]>;
  onSelect: (item: ClipItem) => void;
  onHoverSelect: (item: ClipItem) => void;
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
          hitRanges={hitRanges?.get(item.id)}
          onSelect={onSelect}
          onHoverSelect={onHoverSelect}
        />
      ))}
    </>
  );
}

/**
 * Popover 左侧列表区：收藏 / 今天 / 更早三组，支持鼠标点击与悬停选中。
 * selectedId / onSelect 由 ClipPopoverApp 持有，B2 键盘流可直接复用同一状态。
 * 扁平遍历顺序：favorites → today → earlier（与 visibleFlatList 保持一致）。
 *
 * 悬停选中受 ownership 仲裁：键盘导航后鼠标停在原处时，列表滚动会把别的行送到指针下，
 * 浏览器随即补发进入事件与同坐标移动事件——这类「没有真实位移」的悬停不得改选中项。
 */
export function PopoverList({ groups, selectedId, hitRanges, onSelect, ownership }: PopoverListProps) {
  const { favorites, today, earlier } = groups;
  const isEmpty = favorites.length === 0 && today.length === 0 && earlier.length === 0;
  const listRef = useRef<HTMLDivElement>(null);

  /**
   * 指针停到某一行：只有鼠标主导时才把选中项交给它。
   * 键盘主导期间鼠标只是「被动停在行上」，选中项归键盘所有。
   */
  function handleHoverSelect(item: ClipItem): void {
    if (!ownership.isPointerOwner()) return;
    onSelect(item);
  }

  /** 列表区内的 pointer 移动：坐标真的变化过才把主导权交给鼠标 */
  function handlePointerMoveCapture(event: React.MouseEvent<HTMLDivElement>): void {
    ownership.observePointerMove(event.clientX, event.clientY);
  }

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
    <div
      ref={listRef}
      role="listbox"
      aria-label="剪贴板历史"
      className="popover-list"
      // 捕获阶段先于行自身的移动事件执行：行处理悬停时，本次移动是否真实已经裁决完毕
      onMouseMoveCapture={handlePointerMoveCapture}
    >
      <PopoverGroup label="收藏" items={favorites} selectedId={selectedId} hitRanges={hitRanges} onSelect={onSelect} onHoverSelect={handleHoverSelect} />
      <PopoverGroup label="今天" items={today} selectedId={selectedId} hitRanges={hitRanges} onSelect={onSelect} onHoverSelect={handleHoverSelect} />
      <PopoverGroup label="更早" items={earlier} selectedId={selectedId} hitRanges={hitRanges} onSelect={onSelect} onHoverSelect={handleHoverSelect} />
    </div>
  );
}
