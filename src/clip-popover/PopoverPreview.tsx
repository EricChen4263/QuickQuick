import type { ClipItem } from "../ipc/ipc-client";
import { sanitizeRichHtml } from "../panels/clipboard/sanitize-html";
import { handleRichLinkClick } from "../panels/clipboard/rich-link";
import { useClipItemThumbnail } from "../panels/clipboard/useClipThumbnail";
import { PREVIEW_WAITING_TEXT, type PreviewState } from "./preview-state";
import { useClipPreviewImage } from "./useClipPreviewImage";

interface PopoverPreviewProps {
  /** 预览区三态，由 resolvePreviewState 判定（见 preview-state.ts） */
  state: PreviewState;
  /** 点击右侧预览图片：请求以大图覆盖层查看 */
  onOpenImage?: (item: ClipItem) => void;
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

/**
 * 文本/富文本内容区：富文本经 sanitize 后用 dangerouslySetInnerHTML 渲染保留格式，
 * 纯文本走 React 默认转义渲染。sanitize 在入 DOM 前完成（设计 §五 XSS 红线）。
 */
function TextContent({ item }: { item: ClipItem }) {
  if (item.kind === "richtext" && item.htmlContent !== undefined) {
    return (
      <div
        className="popover-preview-content body-3"
        onClick={handleRichLinkClick}
        dangerouslySetInnerHTML={{ __html: sanitizeRichHtml(item.htmlContent) }}
      />
    );
  }
  return <div className="popover-preview-content body-3">{item.content}</div>;
}

/**
 * 图片内容区：大图预览优先，缩略图回落，两者都没有时保持 `[图片]` 占位。
 * 缩略图与预览都经 IPC 按需拉取；缩略图通常先到，预览到达后替换。
 * 替换会改变图片的显示尺寸（缩略图是 256px 固有宽度，预览按容器宽度撑开），
 * 这里不做尺寸占位，属于已知的一次性视觉变化。
 */
function ImageContent({
  item,
  onOpen,
}: {
  item: ClipItem;
  onOpen?: (item: ClipItem) => void;
}) {
  const thumbnailUrl = useClipItemThumbnail(item);
  const previewUrl = useClipPreviewImage(item.imageId ?? null);
  const displayUrl = previewUrl ?? thumbnailUrl;
  if (displayUrl === null) {
    return <div className="popover-preview-img-placeholder body-5">[图片]</div>;
  }
  return (
    <img
      src={displayUrl}
      alt="图片预览"
      className="popover-preview-img"
      onClick={onOpen ? () => onOpen(item) : undefined}
    />
  );
}

/**
 * Popover 右侧预览区：展示选中条目的类型·时间 kicker + 内容。
 * 三态渲染：有条目 / 分级延迟占位 / 真的没有选中项——状态本身由 resolvePreviewState 判定，
 * 本组件只按态渲染，避免在 JSX 里堆叠条件。
 * selectedId 与扁平列表由 ClipPopoverApp 持有，B2 键盘流直接改 selectedId 即可。
 */
export function PopoverPreview({ state, onOpenImage }: PopoverPreviewProps) {
  if (state.kind === "waiting") {
    return (
      <div className="popover-preview popover-preview-waiting">
        <span className="caption-2">{PREVIEW_WAITING_TEXT}</span>
      </div>
    );
  }

  if (state.kind === "empty") {
    return (
      <div className="popover-preview popover-preview-empty">
        <span className="caption-2">无选中项</span>
      </div>
    );
  }

  const item = state.item;
  return (
    <div className="popover-preview">
      <div className="popover-preview-kicker">
        <span className="caption-4">{kindLabel(item.kind)}</span>
        <span className="popover-preview-dot" aria-hidden="true" />
        <span className="caption-4 mono">{formatTime(item.lastModifiedUtc)}</span>
        {item.isFavorite && (
          <span className="popover-preview-fav-badge badge caption-5">已收藏</span>
        )}
      </div>
      {item.kind === "image" ? (
        <ImageContent item={item} onOpen={onOpenImage} />
      ) : (
        <TextContent item={item} />
      )}
    </div>
  );
}
