import type { ClipItem } from "../ipc/ipc-client";

interface PopoverPreviewProps {
  item: ClipItem | null;
}

/** 将 kind 值转为人读标签 */
function kindLabel(kind: ClipItem["kind"]): string {
  if (kind === "image") return "图片";
  if (kind === "richtext") return "富文本";
  return "文本";
}

/** 将 UTC 毫秒时间戳格式化为本地时间字符串 */
function formatTime(utcMs: number): string {
  return new Date(utcMs).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** 文本/富文本内容区 */
function TextContent({ item }: { item: ClipItem }) {
  const isCode = item.kind === "richtext";
  return (
    <div className={isCode ? "popover-preview-content code" : "popover-preview-content"}>
      {item.content}
    </div>
  );
}

/** 图片内容区：优先用 thumbnailDataUrl，无则占位文字 */
function ImageContent({ item }: { item: ClipItem }) {
  if (item.thumbnailDataUrl) {
    return (
      <img
        src={item.thumbnailDataUrl}
        alt="图片预览"
        className="popover-preview-img"
      />
    );
  }
  return <div className="popover-preview-img-placeholder">[图片]</div>;
}

/**
 * Popover 右侧预览区：展示选中条目的类型·时间 kicker + 内容。
 * 无选中项时显示空态提示。
 * selectedId 与扁平列表由 ClipPopoverApp 持有，B2 键盘流直接改 selectedId 即可。
 */
export function PopoverPreview({ item }: PopoverPreviewProps) {
  if (!item) {
    return (
      <div className="popover-preview popover-preview-empty">
        <span>无选中项</span>
      </div>
    );
  }

  return (
    <div className="popover-preview">
      <div className="popover-preview-kicker">
        <span>{kindLabel(item.kind)}</span>
        <span className="popover-preview-dot" aria-hidden="true" />
        <span>{formatTime(item.lastModifiedUtc)}</span>
        {item.isFavorite && <span className="popover-preview-fav-badge">已收藏</span>}
      </div>
      {item.kind === "image" ? (
        <ImageContent item={item} />
      ) : (
        <TextContent item={item} />
      )}
    </div>
  );
}
