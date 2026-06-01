import type { TopLevel } from "../main-window/nav";
import { TitleBar } from "./TitleBar";
import { SideBar } from "./SideBar";

interface AppShellProps {
  activeTop: TopLevel;
  onNavigate: (top: TopLevel) => void;
  children: React.ReactNode;
}

/**
 * 应用外壳：自绘标题栏 + 侧边栏 + 内容区。
 *
 * 布局分两层：
 *   - 外层 .qq-main：纵向 grid（auto 标题栏 + 1fr 主体），高度 100vh。
 *   - 内层 .qq-shell-body：横向 grid（92px 侧栏 + 1fr 内容），min-height:0 确保不撑破外层。
 *
 * 滚动链：标题栏固定高度不滚；.qq-shell-body 及 main 均 overflow:hidden；
 * 各页面内部列负责 overflow-y:auto，整体窗口不产生外层滚动条。
 */
export function AppShell({ activeTop, onNavigate, children }: AppShellProps) {
  return (
    <div className="qq-main">
      <TitleBar />
      <div className="qq-shell-body">
        <SideBar activeTop={activeTop} onNavigate={onNavigate} />
        <main style={{ minWidth: 0, minHeight: 0, height: "100%", overflow: "hidden" }}>
          {children}
        </main>
      </div>
    </div>
  );
}
