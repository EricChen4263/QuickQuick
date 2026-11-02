import { useClipThumbnail } from "../panels/clipboard/useClipThumbnail";
import { useClipPreviewImage } from "./useClipPreviewImage";

interface PopoverImageViewerProps {
  /** 图片条目的 clip_images 行 ID */
  imageId: string;
  /** 任意点击覆盖层即关闭（含点击图片本身） */
  onClose: () => void;
}

/**
 * 浮窗内的大图覆盖层：盖住整个浮窗，显示该图片的大图预览。
 *
 * 大图优先取 1600px 预览图（浮窗内能完整铺开），取不到时回落缩略图，
 * 两者都没有时保持 `[图片]` 占位。关闭由上层负责（点击与 Esc 都经 onClose）。
 */
export function PopoverImageViewer({ imageId, onClose }: PopoverImageViewerProps) {
  const previewUrl = useClipPreviewImage(imageId);
  const thumbnailUrl = useClipThumbnail(imageId);
  const displayUrl = previewUrl ?? thumbnailUrl;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="图片大图"
      className="popover-image-viewer"
      onClick={onClose}
    >
      {displayUrl === null ? (
        <div className="popover-image-viewer-placeholder body-5">[图片]</div>
      ) : (
        <img src={displayUrl} alt="图片大图" className="popover-image-viewer-img" />
      )}
    </div>
  );
}
