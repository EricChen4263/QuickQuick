/**
 * 右侧预览区：展示当前高亮剪贴板条目的完整内容。
 * 图片项由 ImagePreview 子组件处理原图加载；文本项直接渲染文本。
 * 无高亮时显示占位文案。
 */

import { useEffect, useState } from "react";
import { getClipImageOriginal, type ClipItem } from "../../ipc/ipc-client";

/** 无选中条目时的占位文案 */
const EMPTY_PREVIEW_PLACEHOLDER = "选择条目以预览内容";

interface ClipPreviewProps {
  item: ClipItem | null;
}

interface ImagePreviewProps {
  imageId: string;
  thumbnailDataUrl?: string;
}

/**
 * 图片预览子组件：先显示缩略图占位，异步加载原图后切换为原图。
 * 用 effect 内局部 cancelled 闭包防止切换条目时产生 stale 更新。
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
      style={{
        maxWidth: "100%",
        maxHeight: "100%",
        objectFit: "contain",
      }}
    />
  );
}

/** 右侧预览子组件 */
export function ClipPreview({ item }: ClipPreviewProps) {
  return (
    <div
      role="region"
      aria-label="预览"
      style={{
        flex: 1,
        padding: "16px",
        borderLeft: "1px solid var(--qq-border)",
        overflowY: "auto",
        whiteSpace: "pre-wrap",
        wordBreak: "break-all",
        color: "var(--qq-text)",
      }}
    >
      {item === null ? (
        <p style={{ color: "var(--qq-text-muted)" }}>{EMPTY_PREVIEW_PLACEHOLDER}</p>
      ) : item.kind === "image" && item.imageId !== undefined ? (
        <ImagePreview imageId={item.imageId} thumbnailDataUrl={item.thumbnailDataUrl} />
      ) : (
        <p>{item.content}</p>
      )}
    </div>
  );
}
