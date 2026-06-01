import type { TopLevel } from "../main-window/nav";
import { ThemeSwitch } from "./ThemeSwitch";

interface SideBarProps {
  activeTop: TopLevel;
  onNavigate: (top: TopLevel) => void;
  hint?: string;
}

interface NavEntry {
  key: TopLevel;
  label: string;
  icon: React.ReactNode;
}

const ClipboardIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    <rect x="8" y="2" width="8" height="4" rx="1"/>
    <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
  </svg>
);

const TranslateIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    <path d="m5 8 6 6"/><path d="m4 14 6-6 2-3"/>
    <path d="M2 5h12"/><path d="M7 2h1"/>
    <path d="m22 22-5-10-5 10"/><path d="M14 18h6"/>
  </svg>
);

const SettingsIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
    <circle cx="12" cy="12" r="3"/>
  </svg>
);

const NAV_ENTRIES: NavEntry[] = [
  { key: "clipboard", label: "剪贴板", icon: ClipboardIcon },
  { key: "translate", label: "翻译", icon: TranslateIcon },
  { key: "settings", label: "设置", icon: SettingsIcon },
];

/** 主导航侧边栏：热键 hint、三个导航项、spacer、主题切换 */
export function SideBar({ activeTop, onNavigate, hint }: SideBarProps) {
  return (
    <nav aria-label="主导航" className="qq-sidebar">
      {hint && (
        <div style={{ fontSize: 11, color: "var(--muted)", padding: "6px 8px 4px", userSelect: "none" }}>
          <kbd style={{ fontFamily: "var(--mono, monospace)", fontSize: 11 }}>{hint}</kbd>
        </div>
      )}

      {NAV_ENTRIES.map((entry) => (
        <button
          key={entry.key}
          className="qq-nav-item"
          aria-current={activeTop === entry.key ? "page" : undefined}
          onClick={() => onNavigate(entry.key)}
        >
          {entry.icon}
          {entry.label}
        </button>
      ))}

      <div className="spacer" />
      <ThemeSwitch />
    </nav>
  );
}
