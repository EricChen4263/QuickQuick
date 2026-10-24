import type { ReactNode } from "react";

interface SettingRowProps {
  label: string;
  description?: string;
  /**
   * 鼠标悬停时的原生 tooltip 文案（渲染为 `title` 属性）。
   *
   * 约定：**必须写明该项的默认值**，并带上合法范围/取值——用户不该为了确认
   * 「默认是多少」去翻设置文件或改一遍再看变化。
   */
  tooltip?: string;
  children: ReactNode;
}

/** 设置行：左侧文字区（标签+可选说明）+ 右侧控件区 */
function SettingRow({ label, description, tooltip, children }: SettingRowProps) {
  return (
    <div className="set-row" title={tooltip}>
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
