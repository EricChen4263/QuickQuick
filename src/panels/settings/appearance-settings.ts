/**
 * 外观设置的纯逻辑：默认值、合法区间、输入解析与 IPC 返回值归一。
 *
 * 这里不碰 React、不发 IPC，所有判定都能用纯函数测试：
 * 设置页的输入框、弹窗的取值、后端的读口都由同一组常量与归一函数约束，
 * 避免「设置页允许 40px、弹窗按 48px 渲染」这类两侧各写一份的漂移。
 */

import type { AppearanceSettings, HighlightStyle, SortOrder } from "../../ipc/ipc-client";
import type { SelectOption } from "../../components/Select";
import { PREVIEW_FIRST_DELAY_MS } from "../../clip-popover/preview-delay";

/**
 * 图片高度 / 首次预览延迟的合法区间与默认值。
 *
 * 与 Rust `settings::MIN/MAX/DEFAULT_IMAGE_HEIGHT_PX`、
 * `MIN/MAX/DEFAULT_PREVIEW_FIRST_DELAY_MS` 逐值对齐：前端只做输入拦截
 * （min/max + 提交前校验），真正生效的拒收在后端——输入框可以被脚本绕过，
 * 边界只有一处可信。
 */
export const MIN_IMAGE_HEIGHT_PX = 16;
export const MAX_IMAGE_HEIGHT_PX = 48;
export const DEFAULT_IMAGE_HEIGHT_PX = 24;

/**
 * 首次预览延迟（毫秒）的区间与默认值。
 *
 * 默认值直接引用 `preview-delay.ts` 的硬编码常量：弹窗在设置读取不到时用的也是它，
 * 若两处各写一个 1500，改一处就会让「设置里的默认」与「实际手感」分叉。
 */
export const MIN_PREVIEW_FIRST_DELAY_MS = 200;
export const MAX_PREVIEW_FIRST_DELAY_MS = 5000;
export const DEFAULT_PREVIEW_FIRST_DELAY_MS = PREVIEW_FIRST_DELAY_MS;

/** 默认排序方式：最近复制优先（与引入本设置之前的列表顺序一致）。 */
export const DEFAULT_SORT_ORDER: SortOrder = "recent";

/** 默认命中高亮样式：背景染色 + 加粗（与既有视觉一致）。 */
export const DEFAULT_HIGHLIGHT_STYLE: HighlightStyle = "default";

/** 全部取默认值的外观设置，供读取失败 / 首帧占位使用。 */
export const DEFAULT_APPEARANCE_SETTINGS: AppearanceSettings = {
  sortOrder: DEFAULT_SORT_ORDER,
  imageHeightPx: DEFAULT_IMAGE_HEIGHT_PX,
  previewFirstDelayMs: DEFAULT_PREVIEW_FIRST_DELAY_MS,
  highlightStyle: DEFAULT_HIGHLIGHT_STYLE,
};

/** 排序方式下拉选项（顺序即下拉里的顺序，第一项为默认档）。 */
export const SORT_ORDER_OPTIONS: SelectOption[] = [
  { value: "recent", label: "最近复制" },
  { value: "first_copied", label: "首次复制" },
];

/** 命中高亮样式下拉选项；三档只影响 <mark> 的呈现，不改变命中区间。 */
export const HIGHLIGHT_STYLE_OPTIONS: SelectOption[] = [
  { value: "default", label: "背景高亮" },
  { value: "underline", label: "下划线" },
  { value: "bold", label: "纯加粗" },
];

/**
 * 十进制整数字面量。
 *
 * 只接受纯数字：`"2.5"`、`"1e3"`、`""`、`"abc"`、全角数字一律判非法。
 * `Number.parseInt("2.5")` 会得到 2——那种「悄悄取整」会让用户以为自己设的是 2.5。
 */
const DECIMAL_INTEGER = /^\d+$/;

/** 解析取值范围内的十进制整数输入；非法（形状或区间）返回 null。 */
export function parseIntInRange(raw: string, min: number, max: number): number | null {
  const trimmed = raw.trim();
  if (!DECIMAL_INTEGER.test(trimmed)) return null;
  const value = Number(trimmed);
  if (!Number.isSafeInteger(value)) return null;
  if (value < min || value > max) return null;
  return value;
}

/** 排序方式的类型守卫：用于校验 IPC 返回值（旧后端/脏数据不得直接透传）。 */
export function isSortOrder(value: unknown): value is SortOrder {
  return value === "recent" || value === "first_copied";
}

/** 命中高亮样式的类型守卫。 */
export function isHighlightStyle(value: unknown): value is HighlightStyle {
  return value === "default" || value === "underline" || value === "bold";
}

/** 把任意输入收敛成区间内的整数；非数字回退 fallback。 */
function clampInteger(value: unknown, min: number, max: number, fallback: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, Math.round(value)));
}

/**
 * 归一 IPC 返回的外观设置。
 *
 * 为什么要归一而不是直接信任后端：弹窗是独立 webview，它拿到的可能是
 * 旧版本后端（缺字段）或被手改成非法值的 settings.json。任一字段坏掉都只回退
 * 该字段的默认值，不允许让「一个字段缺失」变成「整个弹窗崩溃」。
 */
export function normalizeAppearanceSettings(raw: unknown): AppearanceSettings {
  const source: Partial<AppearanceSettings> =
    typeof raw === "object" && raw !== null ? (raw as Partial<AppearanceSettings>) : {};

  return {
    sortOrder: isSortOrder(source.sortOrder) ? source.sortOrder : DEFAULT_SORT_ORDER,
    imageHeightPx: clampInteger(
      source.imageHeightPx,
      MIN_IMAGE_HEIGHT_PX,
      MAX_IMAGE_HEIGHT_PX,
      DEFAULT_IMAGE_HEIGHT_PX,
    ),
    previewFirstDelayMs: clampInteger(
      source.previewFirstDelayMs,
      MIN_PREVIEW_FIRST_DELAY_MS,
      MAX_PREVIEW_FIRST_DELAY_MS,
      DEFAULT_PREVIEW_FIRST_DELAY_MS,
    ),
    highlightStyle: isHighlightStyle(source.highlightStyle)
      ? source.highlightStyle
      : DEFAULT_HIGHLIGHT_STYLE,
  };
}
