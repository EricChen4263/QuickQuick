import type { ReactNode } from "react";

interface SettingGroupProps {
  children: ReactNode;
}

/** 设置分组容器：带边框圆角卡片，包裹若干 SettingRow */
function SettingGroup({ children }: SettingGroupProps) {
  return <div className="set-group">{children}</div>;
}

export default SettingGroup;
