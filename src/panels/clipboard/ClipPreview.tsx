/**
 * 右侧预览区：展示当前高亮剪贴板条目的完整内容。
 * 图片项由 ImagePreview 子组件处理原图加载（先缩略图后原图）。
 * 无高亮时用 EmptyState 组件展示空态。
 */

import { useEffect, useState } from "react";
import { getClipImageOriginal, type ClipItem } from "../../ipc/ipc-client";
import EmptyState from "../../components/EmptyState";

interface ClipPreviewProps {
  item: ClipItem | null;
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

/** 右侧预览子组件 */
export function ClipPreview({ item }: ClipPreviewProps) {
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
              <div className={item.kind === "richtext" ? "preview-content code" : "preview-content"}>
                {item.content}
              </div>
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
          <div className="preview-actions">
            <button className="btn" type="button">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <rect x="9" y="9" width="13" height="13" rx="2" />
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
              </svg>
              复制
            </button>
          </div>
        </>
      )}
    </div>
  );
}
