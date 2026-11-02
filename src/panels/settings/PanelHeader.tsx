interface PanelHeaderProps {
  title: string;
  subtitle: string;
}

/**
 * 设置面板页标题区：大标题 + 一行说明。
 *
 * 字号与字重取 base.css 的命名样式（.heading-6 / .caption-2），
 * 本组件只负责语义标签与留白类，不自己堆 font-size。
 */
function PanelHeader({ title, subtitle }: PanelHeaderProps) {
  return (
    <>
      <h2 className="set-h heading-6">{title}</h2>
      <p className="set-sub caption-2">{subtitle}</p>
    </>
  );
}

export default PanelHeader;
