import type { TopLevel } from "../main-window/nav";
import { SideBar } from "./SideBar";

interface AppShellProps {
  activeTop: TopLevel;
  onNavigate: (top: TopLevel) => void;
  hint?: string;
  children: React.ReactNode;
}

/**
 * 应用外壳：侧边栏 + 内容区布局容器。
 * 布局使用 CSS grid（92px 侧边栏 + 剩余内容区），与设计稿 .shell 一致。
 */
export function AppShell({ activeTop, onNavigate, hint, children }: AppShellProps) {
  return (
    <div className="qq-main">
      <SideBar activeTop={activeTop} onNavigate={onNavigate} hint={hint} />
      <main style={{ minWidth: 0, minHeight: 0 }}>
        {children}
      </main>
    </div>
  );
}
