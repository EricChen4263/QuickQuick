import type { TopLevel } from "../main-window/nav";
import { TitleBar } from "./TitleBar";
import { TopNav } from "./TopNav";

interface AppShellProps {
  activeTop: TopLevel;
  onNavigate: (top: TopLevel) => void;
  children: React.ReactNode;
}

/**
 * 应用外壳：一条 48px 顶部栏（品牌 + 一级导航 + 窗口控制）+ 内容区。
 *
 * 布局分两层：
 *   - 外层 .qq-main：纵向 grid（auto 顶部栏 + 1fr 主体），高度 100vh。
 *   - 内层 .qq-shell-body：单列 grid，min-height:0 确保不撑破外层。
 *
 * 相较于旧版去掉了 112px 常驻侧栏：一级导航并入顶部栏后内容区拿到全宽，
 * 「列表 + 预览」双栏才是这类工具真正需要横向空间的地方。
 *
 * 滚动链：顶部栏固定高度不滚；.qq-shell-body 与 main 均 overflow:hidden；
 * 各页面内部列负责 overflow-y:auto，整体窗口不产生外层滚动条。
 */
export function AppShell({ activeTop, onNavigate, children }: AppShellProps) {
  return (
    <div className="qq-main">
      <TitleBar>
        <TopNav activeTop={activeTop} onNavigate={onNavigate} />
      </TitleBar>
      <div className="qq-shell-body">
        <main style={{ minWidth: 0, minHeight: 0, height: "100%", overflow: "hidden" }}>
          {children}
        </main>
      </div>
    </div>
  );
}
