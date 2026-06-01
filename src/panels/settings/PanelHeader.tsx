interface PanelHeaderProps {
  title: string;
  subtitle: string;
}

/** 设置面板页标题区：大标题 + 一行说明文字 */
function PanelHeader({ title, subtitle }: PanelHeaderProps) {
  return (
    <>
      <h2 className="set-h">{title}</h2>
      <p className="set-sub">{subtitle}</p>
    </>
  );
}

export default PanelHeader;
