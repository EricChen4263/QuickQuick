/**
 * 右侧预览区：展示当前高亮剪贴板条目的完整内容。
 * 无高亮时显示占位文案。
 */

import type { ClipItem } from "../../ipc/ipc-client";

/** 无选中条目时的占位文案 */
const EMPTY_PREVIEW_PLACEHOLDER = "选择条目以预览内容";

interface ClipPreviewProps {
  item: ClipItem | null;
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
      ) : (
        <p>{item.content}</p>
      )}
    </div>
  );
}
