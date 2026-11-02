import { useEffect } from "react";
import "./ImageViewer.css";
import { useClipPreviewImage } from "../clip-popover/useClipPreviewImage";
import { useClipThumbnail } from "../panels/clipboard/useClipThumbnail";

interface ImageViewerProps {
  /** 图片条目的 clip_images 行 ID */
  imageId: string;
  /** 点击覆盖层任意处或按 Esc 时调用；关闭动作由宿主页面决定 */
  onClose: () => void;
}

/**
 * 大图覆盖层：盖住所在容器（浮窗或主窗口预览区），显示图片大图。
 *
 * 大图优先取 1600px 预览图，取不到回落缩略图，两者都没有时显示 `[图片]` 占位。
 * 关闭由宿主通过 onClose 决定；覆盖层打开期间自己拦截 Esc（捕获阶段），
 * 所以宿主无需为 Esc 另写处理。容器需设为 position: relative，覆盖层以 absolute 铺满它。
 */
export function ImageViewer({ imageId, onClose }: ImageViewerProps) {
  const previewUrl = useClipPreviewImage(imageId);
  const thumbnailUrl = useClipThumbnail(imageId);
  const displayUrl = previewUrl ?? thumbnailUrl;

  // 覆盖层打开期间 Esc 只关覆盖层。捕获阶段拦截，阻止事件继续到达宿主的键盘处理：
  // 否则在浮窗里 Esc 会关掉整个面板，在主窗口里会触发页面自己的 Esc 行为。
  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent): void {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
    window.addEventListener("keydown", closeOnEscape, true);
    return () => {
      window.removeEventListener("keydown", closeOnEscape, true);
    };
  }, [onClose]);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="图片大图"
      className="image-viewer"
      onClick={onClose}
    >
      {displayUrl === null ? (
        <div className="image-viewer-placeholder body-5">[图片]</div>
      ) : (
        <img src={displayUrl} alt="图片大图" className="image-viewer-img" />
      )}
    </div>
  );
}
