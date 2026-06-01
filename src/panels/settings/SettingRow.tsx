import type { ReactNode } from "react";

interface SettingRowProps {
  label: string;
  description?: string;
  children: ReactNode;
}

/** 设置行：左侧文字区（标签+可选说明）+ 右侧控件区 */
function SettingRow({ label, description, children }: SettingRowProps) {
  return (
    <div className="set-row">
      <div className="grow">
        <div className="label">{label}</div>
        {description !== undefined && (
          <div className="desc">{description}</div>
        )}
      </div>
      {children}
    </div>
  );
}

export default SettingRow;
