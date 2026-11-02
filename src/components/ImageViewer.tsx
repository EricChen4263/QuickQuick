import { useEffect, useRef, useState, type MouseEvent } from "react";
import "./ImageViewer.css";
import { useClipPreviewImage } from "../clip-popover/useClipPreviewImage";
import { useClipThumbnail } from "../panels/clipboard/useClipThumbnail";
import {
  effectiveScale,
  formatZoomPercent,
  stepZoom,
  viewerKeyAction,
  type ViewerAction,
} from "./image-viewer-zoom";

interface ImageViewerProps {
  /** 图片条目的 clip_images 行 ID */
  imageId: string;
  /** 关闭覆盖层：点击空白处或按 Esc 时调用，关闭动作由宿主决定 */
  onClose: () => void;
}

/** 图片原始像素尺寸：加载完成后才可知 */
interface NaturalSize {
  width: number;
  height: number;
}

/**
 * 大图覆盖层：盖住所在容器（浮窗或主窗口预览区），支持缩放与原比例查看。
 *
 * 显示：大图优先取 1600px 预览图，取不到回落缩略图，两者都没有显示 `[图片]` 占位。
 * 操作：工具栏（缩小 / 放大 / 适应窗口 / 原比例）与键盘（- / + / 0 / 1）。
 * 关闭：点击空白处或按 Esc；点击图片本身与工具栏不关闭。
 * 键盘在覆盖层打开期间于捕获阶段拦截，宿主不会收到这些按键。
 * 容器需设为 position: relative，覆盖层以 absolute 铺满它。
 */
export function ImageViewer({ imageId, onClose }: ImageViewerProps) {
  const previewUrl = useClipPreviewImage(imageId);
  const thumbnailUrl = useClipThumbnail(imageId);
  const displayUrl = previewUrl ?? thumbnailUrl;

  // null = 适应窗口；数字 = 相对原始像素的倍率
  const [zoom, setZoom] = useState<number | null>(null);
  const [natural, setNatural] = useState<NaturalSize | null>(null);
  const imgRef = useRef<HTMLImageElement>(null);

  function applyAction(action: ViewerAction): void {
    if (action === "close") {
      onClose();
      return;
    }
    if (action === "fit") {
      setZoom(null);
      return;
    }
    if (action === "actual") {
      setZoom(1);
      return;
    }
    // 缩放需要原始尺寸做基准；图片未加载完成时忽略
    if (natural === null) return;
    const renderedWidth = imgRef.current?.getBoundingClientRect().width ?? 0;
    const scale = effectiveScale(zoom, renderedWidth, natural.width);
    setZoom(stepZoom(scale, action === "zoom-in" ? "in" : "out"));
  }

  // 每次渲染重新绑定：applyAction 闭包捕获的 zoom/natural 必须是最新值。
  // 捕获阶段拦截，覆盖层打开期间这些按键不会到达宿主（搜索框、预览区等）。
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      const action = viewerKeyAction(event.key);
      if (action === null) return;
      event.preventDefault();
      event.stopPropagation();
      applyAction(action);
    }
    window.addEventListener("keydown", onKeyDown, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
    };
  });

  /** 只有点在覆盖层或舞台的空白处才关闭；点到图片或工具栏不关闭 */
  function closeOnBackdrop(event: MouseEvent<HTMLElement>): void {
    if (event.target === event.currentTarget) onClose();
  }

  const zoomable = natural !== null;
  const imageStyle =
    zoom !== null && natural !== null
      ? { width: `${natural.width * zoom}px` }
      : undefined;
  const zoomLabel = zoom === null ? "适应" : formatZoomPercent(zoom);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="图片大图"
      className="image-viewer"
      onClick={closeOnBackdrop}
    >
      <div className="image-viewer-stage" onClick={closeOnBackdrop}>
        {displayUrl === null ? (
          <div className="image-viewer-placeholder body-5">[图片]</div>
        ) : (
          <img
            ref={imgRef}
            src={displayUrl}
            alt="图片大图"
            className={zoom === null ? "image-viewer-img" : "image-viewer-img is-zoomed"}
            style={imageStyle}
            onLoad={(event) => {
              // 新图加载后回到适应窗口，避免沿用上一张的倍率
              setNatural({
                width: event.currentTarget.naturalWidth,
                height: event.currentTarget.naturalHeight,
              });
              setZoom(null);
            }}
          />
        )}
      </div>
      <div className="image-viewer-toolbar" role="toolbar" aria-label="图片缩放">
        <button
          type="button"
          className="image-viewer-btn"
          aria-label="缩小"
          disabled={!zoomable}
          onClick={() => applyAction("zoom-out")}
        >
          −
        </button>
        <span className="image-viewer-zoom-label" aria-live="polite">
          {zoomLabel}
        </span>
        <button
          type="button"
          className="image-viewer-btn"
          aria-label="放大"
          disabled={!zoomable}
          onClick={() => applyAction("zoom-in")}
        >
          +
        </button>
        <button
          type="button"
          className="image-viewer-btn"
          aria-label="适应窗口"
          onClick={() => applyAction("fit")}
        >
          适应
        </button>
        <button
          type="button"
          className="image-viewer-btn"
          aria-label="原比例"
          disabled={!zoomable}
          onClick={() => applyAction("actual")}
        >
          原比例
        </button>
      </div>
    </div>
  );
}
