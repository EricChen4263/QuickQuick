/**
 * 剪贴板列表单行：类型图标 + 内容摘要 + 收藏操作。
 * 外层用 div[role="option"] 而非 button，避免内部 button 产生非法嵌套。
 * 高亮态由 CSS [aria-selected="true"] 驱动，不用 inline style。
 * 删除操作统一在右侧预览区执行，列表行不重复提供。
 */

import type { ClipItem } from "../../ipc/ipc-client";

/** 内容摘要最大字符数 */
const SUMMARY_MAX_LENGTH = 60;

/** 收藏状态按钮文案 */
const FAVORITE_LABEL_ON = "取消收藏";
const FAVORITE_LABEL_OFF = "收藏";

interface ClipItemRowProps {
  item: ClipItem;
  isHighlighted: boolean;
  onToggleFavorite: (item: ClipItem) => Promise<void>;
  onSelect: (item: ClipItem) => void;
}

/** 截断文本至指定长度，超出加省略号 */
function truncateSummary(text: string): string {
  if (text.length <= SUMMARY_MAX_LENGTH) return text;
  return text.slice(0, SUMMARY_MAX_LENGTH) + "…";
}

/** 类型图标：根据 kind 返回对应 SVG（richtext 用文本图标，image 用图片图标，默认文本图标） */
function KindIcon({ kind }: { kind: ClipItem["kind"] }) {
  if (kind === "image") {
    return (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <rect x="3" y="3" width="18" height="18" rx="2" />
        <circle cx="8.5" cy="8.5" r="1.5" />
        <path d="m21 15-5-5L5 21" />
      </svg>
    );
  }
  if (kind === "richtext") {
    return (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <polyline points="4 7 4 4 20 4 20 7" />
        <line x1="9" y1="20" x2="15" y2="20" />
        <line x1="12" y1="4" x2="12" y2="20" />
      </svg>
    );
  }
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <line x1="3" y1="6" x2="21" y2="6" />
      <line x1="3" y1="12" x2="21" y2="12" />
      <line x1="3" y1="18" x2="15" y2="18" />
    </svg>
  );
}

/** 图片内容区：有缩略图则显示 img，否则显示占位文字 */
function ImageContent({ item }: { item: ClipItem }) {
  if (item.thumbnailDataUrl) {
    return (
      <img
        src={item.thumbnailDataUrl}
        alt="图片缩略图"
        className="clip-thumb"
      />
    );
  }
  return <span>[图片]</span>;
}

/** 收藏星标按钮：所有类型（含图片）均显示 */
function StarButton({ item, onToggleFavorite }: { item: ClipItem; onToggleFavorite: (item: ClipItem) => Promise<void> }) {
  const label = item.isFavorite ? FAVORITE_LABEL_ON : FAVORITE_LABEL_OFF;
  return (
    <button
      className={item.isFavorite ? "clip-star fav" : "clip-star"}
      aria-label={label}
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        onToggleFavorite(item);
      }}
    >
      <svg viewBox="0 0 24 24" fill={item.isFavorite ? "currentColor" : "none"} stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
      </svg>
    </button>
  );
}

/** 剪贴板列表单行子组件 */
export function ClipItemRow({
  item,
  isHighlighted,
  onToggleFavorite,
  onSelect,
}: ClipItemRowProps) {
  const isCode = item.kind === "richtext";

  return (
    <div
      role="option"
      aria-selected={isHighlighted}
      className="clip-row"
      onClick={() => { onSelect(item); }}
    >
      <div className="clip-kind">
        <KindIcon kind={item.kind} />
      </div>
      <div className="clip-main">
        {item.kind === "image" ? (
          <ImageContent item={item} />
        ) : (
          <div className={isCode ? "clip-text code" : "clip-text"}>
            {truncateSummary(item.content)}
          </div>
        )}
      </div>
      <div className="clip-actions">
        <StarButton item={item} onToggleFavorite={onToggleFavorite} />
      </div>
    </div>
  );
}
