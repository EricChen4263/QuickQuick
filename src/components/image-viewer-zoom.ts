/**
 * 大图覆盖层的缩放数学与按键映射（纯函数，不碰 DOM）。
 *
 * 比例语义：`zoom` 是相对图片原始像素的倍率，1 即「原比例」（一个图片像素对应一个 CSS 像素）。
 * 「适应窗口」不是固定倍率，而是 `null`，渲染时由容器尺寸决定。
 */

/** 每次放大/缩小的倍率步长 */
export const ZOOM_STEP = 1.25;
/** 最小比例：再小对看图没有意义 */
export const ZOOM_MIN = 0.1;
/** 最大比例：超过后像素放大失真严重 */
export const ZOOM_MAX = 8;

export type ZoomDirection = "in" | "out";

/** 覆盖层可执行的操作；键盘与工具栏共用这一套 */
export type ViewerAction = "close" | "zoom-in" | "zoom-out" | "fit" | "actual";

/** 把比例夹在 [ZOOM_MIN, ZOOM_MAX] 之间 */
export function clampZoom(zoom: number): number {
  return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom));
}

/** 按方向前进一步，结果夹在合法范围内 */
export function stepZoom(current: number, direction: ZoomDirection): number {
  return clampZoom(direction === "in" ? current * ZOOM_STEP : current / ZOOM_STEP);
}

/**
 * 「适应窗口」时的实际比例：渲染宽度 / 原始宽度。
 * 任一尺寸无效（图片未加载、布局尚未发生）时按原比例 1 处理，避免除零或比例失真。
 */
export function fitScale(renderedWidth: number, naturalWidth: number): number {
  if (renderedWidth <= 0 || naturalWidth <= 0) return 1;
  return renderedWidth / naturalWidth;
}

/** 当前有效比例：显式比例直接用，适应窗口时按渲染结果反推 */
export function effectiveScale(
  zoom: number | null,
  renderedWidth: number,
  naturalWidth: number,
): number {
  return zoom ?? fitScale(renderedWidth, naturalWidth);
}

/** 比例显示文案，如 1.25 → "125%" */
export function formatZoomPercent(zoom: number): string {
  return `${Math.round(zoom * 100)}%`;
}

/**
 * 按键到覆盖层操作的映射；不认识的键返回 null，由调用方放行。
 * `+` 与 `=` 同键（无 Shift 时为 `=`），`-` 与 `_` 同键。
 */
export function viewerKeyAction(key: string): ViewerAction | null {
  switch (key) {
    case "Escape":
      return "close";
    case "+":
    case "=":
      return "zoom-in";
    case "-":
    case "_":
      return "zoom-out";
    case "0":
      return "fit";
    case "1":
      return "actual";
    default:
      return null;
  }
}
