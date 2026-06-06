/**
 * 右侧预览区：展示当前高亮剪贴板条目的完整内容。
 * 图片项由 ImagePreview 子组件处理原图加载（先缩略图后原图）。
 * 无高亮时用 EmptyState 组件展示空态。
 */

import { useEffect, useState } from "react";
import { copyClipToClipboard, getClipImageOriginal, type ClipItem } from "../../ipc/ipc-client";
import EmptyState from "../../components/EmptyState";
import { sanitizeRichHtml } from "./sanitize-html";

interface ClipPreviewProps {
  item: ClipItem | null;
  onToggleFavorite: (item: ClipItem) => void;
  onDelete: (item: ClipItem) => void;
  onCopy: (item: ClipItem) => void;
  /** 里程碑3接入：粘贴到前台窗口（IPC paste）*/
  onPasteToFront: (item: ClipItem) => void;
  /** 里程碑3接入：跳转翻译页并填入内容 */
  onTranslate: (item: ClipItem) => void;
}

interface ImagePreviewProps {
  imageId: string;
  thumbnailDataUrl?: string;
}

/** 格式化 UTC 时间戳为本地可读字符串 */
function formatTimestamp(utcMs: number): string {
  return new Date(utcMs).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** 类型显示名映射 */
const KIND_LABEL: Record<ClipItem["kind"], string> = {
  text: "纯文本",
  richtext: "富文本",
  image: "图片",
};

/**
 * 图片预览子组件：先显示缩略图占位，异步加载原图后切换为原图。
 * effect 内局部 cancelled 闭包防止切换条目时产生 stale 更新。
 */
function ImagePreview({ imageId, thumbnailDataUrl }: ImagePreviewProps) {
  const [originalDataUrl, setOriginalDataUrl] = useState<string | null>(null);

  useEffect(() => {
    const cancelled = { current: false };
    setOriginalDataUrl(null);

    getClipImageOriginal(imageId)
      .then((url) => {
        if (cancelled.current) return;
        setOriginalDataUrl(url);
      })
      .catch(() => {
        if (cancelled.current) return;
        setOriginalDataUrl(null);
      });

    return () => {
      cancelled.current = true;
    };
  }, [imageId]);

  const displayUrl = originalDataUrl ?? thumbnailDataUrl ?? null;

  if (displayUrl === null) {
    return null;
  }

  return (
    <img
      src={displayUrl}
      alt="图片预览"
      className="preview-img"
    />
  );
}

/** 空态图标 SVG */
const EmptyIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <rect x="8" y="2" width="8" height="4" rx="1" />
    <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
    <line x1="12" y1="11" x2="12" y2="17" />
    <line x1="9" y1="14" x2="15" y2="14" />
  </svg>
);

/** 粘贴箭头图标 */
const PasteIcon = (
  <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M4 12h12M12 6l6 6-6 6" />
  </svg>
);

/** 复制图标 */
const CopyIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <rect x="9" y="9" width="13" height="13" rx="2" />
    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
  </svg>
);

/** 翻译图标 */
const TranslateIcon = (
  <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="m5 8 6 6" />
    <path d="m4 14 6-6 2-3" />
    <path d="M2 5h12" />
    <path d="m22 22-5-10-5 10" />
    <path d="M14 18h6" />
  </svg>
);

/** 收藏星（已收藏，实心） */
const StarFilledIcon = (
  <svg viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
  </svg>
);

/** 收藏星（未收藏，描边） */
const StarOutlineIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
  </svg>
);

/** 垃圾桶图标 */
const TrashIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
  </svg>
);

/** 预览操作栏：五个操作按钮（图片项不显示一键翻译） */
function PreviewActions({ item, onToggleFavorite, onDelete, onCopy, onPasteToFront, onTranslate }: {
  item: ClipItem;
  onToggleFavorite: (item: ClipItem) => void;
  onDelete: (item: ClipItem) => void;
  onCopy: (item: ClipItem) => void;
  onPasteToFront: (item: ClipItem) => void;
  onTranslate: (item: ClipItem) => void;
}) {
  function handleCopy() {
    // 走后端 IPC 按 id 取 text+html 写系统剪贴板，保真富文本格式（纯文本项 html=None 走 set_text，行为等价）。
    copyClipToClipboard(item.id).catch(() => {
      // 复制失败静默处理，不影响主流程
    });
    onCopy(item);
  }

  return (
    <div className="preview-actions">
      <button
        className="btn btn-primary"
        type="button"
        aria-label="粘贴到前台"
        onClick={() => { onPasteToFront(item); }}
      >
        {PasteIcon}
        粘贴到前台
      </button>
      <button
        className="btn"
        type="button"
        aria-label="复制"
        onClick={handleCopy}
      >
        {CopyIcon}
        复制
      </button>
      {item.kind !== "image" && (
        <button
          className="btn"
          type="button"
          aria-label="一键翻译"
          onClick={() => { onTranslate(item); }}
        >
          {TranslateIcon}
          一键翻译
        </button>
      )}
      <button
        className={item.isFavorite ? "icon-btn on" : "icon-btn"}
        type="button"
        aria-label="收藏"
        onClick={() => { onToggleFavorite(item); }}
      >
        {item.isFavorite ? StarFilledIcon : StarOutlineIcon}
      </button>
      <button
        className="icon-btn danger"
        type="button"
        aria-label="删除"
        onClick={() => { onDelete(item); }}
      >
        {TrashIcon}
      </button>
    </div>
  );
}

/**
 * 文本/富文本内容区：富文本经 sanitize 后用 dangerouslySetInnerHTML 渲染保留格式，
 * 纯文本走 React 默认转义渲染。sanitize 在入 DOM 前完成（设计 §五 XSS 红线）。
 */
function PreviewContent({ item }: { item: ClipItem }) {
  if (item.kind === "richtext" && item.htmlContent !== undefined) {
    return (
      <div
        className="preview-content"
        dangerouslySetInnerHTML={{ __html: sanitizeRichHtml(item.htmlContent) }}
      />
    );
  }
  return <div className="preview-content">{item.content}</div>;
}

/** 右侧预览子组件 */
export function ClipPreview({
  item,
  onToggleFavorite,
  onDelete,
  onCopy,
  onPasteToFront,
  onTranslate,
}: ClipPreviewProps) {
  return (
    <div
      role="region"
      aria-label="预览"
      className="clip-preview"
    >
      {item === null ? (
        <EmptyState
          icon={EmptyIcon}
          title="尚未选择条目"
          description="选择左侧条目以预览内容"
        />
      ) : (
        <>
          <div className="preview-body">
            <div className="preview-kicker">
              <span>{KIND_LABEL[item.kind]}</span>
              <span className="dot" />
              <span>{formatTimestamp(item.lastModifiedUtc)}</span>
            </div>
            {item.kind === "image" && item.imageId !== undefined ? (
              <ImagePreview imageId={item.imageId} thumbnailDataUrl={item.thumbnailDataUrl} />
            ) : (
              <PreviewContent item={item} />
            )}
            <dl className="meta-grid">
              <dt>类型</dt>
              <dd>{KIND_LABEL[item.kind]}</dd>
              <dt>收藏</dt>
              <dd>{item.isFavorite ? "是" : "否"}</dd>
              <dt>修改时间</dt>
              <dd>{formatTimestamp(item.lastModifiedUtc)}</dd>
            </dl>
          </div>
          <PreviewActions
            item={item}
            onToggleFavorite={onToggleFavorite}
            onDelete={onDelete}
            onCopy={onCopy}
            onPasteToFront={onPasteToFront}
            onTranslate={onTranslate}
          />
        </>
      )}
    </div>
  );
}
