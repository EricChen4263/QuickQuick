import type { ReactNode } from "react";

interface SettingGroupProps {
  /** 整组的悬停 tooltip（用于「一组成员共享一个设置项」的场景，如翻译源单选列表） */
  tooltip?: string;
  children: ReactNode;
}

/**
 * 设置分组容器。
 *
 * 刻意是「无样式容器」：分组关系由留白（组与组之间）和行分隔线（组内）表达，
 * 不再给一组设置套边框圆角卡——那会让这张表单看起来像一堆独立的模块，
 * 而它们其实只是同一页里的三段。
 */
function SettingGroup({ tooltip, children }: SettingGroupProps) {
  return (
    <div className="set-group" title={tooltip}>
      {children}
    </div>
  );
}

export default SettingGroup;
