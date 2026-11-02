import type { ReactNode } from "react";
import "./EmptyState.css";

interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  description: string;
  /**
   * 可选的下一步动作（按钮）。
   *
   * 空态不是「空白面板」：用户看到「没有内容」时唯一需要的是**接下来做什么**，
   * 所以支持在说明下方挂一个具体动作；不传时退化为纯说明视图，行为与旧版一致。
   */
  action?: ReactNode;
}

/** 通用空态展示：图标 + 标题 + 说明文字 + 可选下一步动作，供剪贴板/翻译历史等多处复用 */
function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="empty">
      <div className="mark">{icon}</div>
      <h3 className="title-3">{title}</h3>
      <p className="body-6">{description}</p>
      {action !== undefined && <div className="empty-action">{action}</div>}
    </div>
  );
}

export default EmptyState;
