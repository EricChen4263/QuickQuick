import type { ReactNode } from "react";

interface SettingGroupProps {
  /** 整组的悬停 tooltip（用于「一组成员共享一个设置项」的场景，如翻译源单选列表） */
  tooltip?: string;
  children: ReactNode;
}

/** 设置分组容器：带边框圆角卡片，包裹若干 SettingRow */
function SettingGroup({ tooltip, children }: SettingGroupProps) {
  return (
    <div className="set-group" title={tooltip}>
      {children}
    </div>
  );
}

export default SettingGroup;
