/**
 * 剪贴板列表单行：内容摘要 + 收藏标记 + 收藏切换按钮 + 删除按钮。
 * 选中态用 --qq-accent 背景色区分。
 */

import type { ClipItem } from "../../ipc/ipc-client";

/** 内容摘要最大字符数 */
const SUMMARY_MAX_LENGTH = 60;

/** 收藏状态按钮文案 */
const FAVORITE_LABEL_ON = "取消收藏";
const FAVORITE_LABEL_OFF = "收藏";

interface ClipItemRowProps {
  item: ClipItem;
  isHighlighted: boolean;
  onToggleFavorite: (item: ClipItem) => Promise<void>;
  onDelete: (item: ClipItem) => Promise<void>;
}

/** 截断文本至指定长度，超出加省略号 */
function truncateSummary(text: string): string {
  if (text.length <= SUMMARY_MAX_LENGTH) return text;
  return text.slice(0, SUMMARY_MAX_LENGTH) + "…";
}

/** 剪贴板列表单行子组件 */
export function ClipItemRow({
  item,
  isHighlighted,
  onToggleFavorite,
  onDelete,
}: ClipItemRowProps) {
  const highlightStyle = isHighlighted
    ? { backgroundColor: "var(--qq-accent)", color: "#fff" }
    : { backgroundColor: "var(--qq-surface)" };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "8px",
        padding: "8px 12px",
        borderBottom: "1px solid var(--qq-border)",
        borderRadius: "var(--qq-radius-md)",
        cursor: "default",
        ...highlightStyle,
      }}
    >
      {item.isFavorite && <span aria-label="已收藏">★</span>}
      <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {truncateSummary(item.content)}
      </span>
      <button
        aria-label={item.isFavorite ? FAVORITE_LABEL_ON : FAVORITE_LABEL_OFF}
        onClick={() => onToggleFavorite(item)}
        style={{ flexShrink: 0 }}
      >
        {item.isFavorite ? FAVORITE_LABEL_ON : FAVORITE_LABEL_OFF}
      </button>
      <button
        aria-label="删除"
        onClick={() => onDelete(item)}
        style={{ flexShrink: 0 }}
      >
        删除
      </button>
    </div>
  );
}
