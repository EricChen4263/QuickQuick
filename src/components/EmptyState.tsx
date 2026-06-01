import type { ReactNode } from "react";

interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  description: string;
}

/** 通用空态展示：图标 + 标题 + 说明文字，供剪贴板/翻译历史等多处复用 */
function EmptyState({ icon, title, description }: EmptyStateProps) {
  return (
    <div className="empty">
      <div className="mark">{icon}</div>
      <h3>{title}</h3>
      <p>{description}</p>
    </div>
  );
}

export default EmptyState;
