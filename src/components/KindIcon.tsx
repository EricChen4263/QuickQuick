import type { ClipItem } from "../ipc/ipc-client";

/**
 * 类型图标：根据 kind 渲染对应 SVG。
 * richtext 用 T 字标（格式/排版语义），image 用图片框，text 用参差段落线。
 * 从 ClipItemRow 提取为共享组件，供列表行与 popover 复用。
 *
 * 描边语汇统一走 base.css 的 `.ico`（24 网格 / stroke-width 1.5 / 圆头圆角），
 * 这里只用尺寸类 `.ico-16` 决定大小，不各自重复描边参数。
 */
export function KindIcon({ kind }: { kind: ClipItem["kind"] }) {
  if (kind === "image") {
    return (
      <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
        <rect x="3.5" y="4.5" width="17" height="15" rx="2.5" />
        <circle cx="9" cy="10" r="1.6" />
        <path d="m4.5 17.5 4.5-4.2 3.6 3.2 2.9-2.4 4 3.4" />
      </svg>
    );
  }
  if (kind === "richtext") {
    return (
      <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M5 7V5h14v2" />
        <path d="M12 5v14" />
        <path d="M9 19h6" />
      </svg>
    );
  }
  return (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4 6h16" />
      <path d="M4 12h12" />
      <path d="M4 18h8" />
    </svg>
  );
}
