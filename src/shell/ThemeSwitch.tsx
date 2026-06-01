import { useTheme } from "../theme/useTheme";
import type { ThemePref } from "../theme/themeStore";

interface SegOption {
  pref: ThemePref;
  title: string;
  icon: React.ReactNode;
}

const MonitorIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    <rect x="2" y="3" width="20" height="14" rx="2"/>
    <path d="M8 21h8"/><path d="M12 17v4"/>
  </svg>
);

const SunIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="4"/>
    <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>
  </svg>
);

const MoonIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/>
  </svg>
);

const OPTIONS: SegOption[] = [
  { pref: "auto", title: "跟随系统", icon: MonitorIcon },
  { pref: "light", title: "浅色", icon: SunIcon },
  { pref: "dark", title: "深色", icon: MoonIcon },
];

/** 三档主题切换按钮组，跟随 themeStore 状态，点击写入 store */
export function ThemeSwitch() {
  const { pref, setPref } = useTheme();

  return (
    <div className="theme-seg" role="group" aria-label="外观">
      {OPTIONS.map((opt) => (
        <button
          key={opt.pref}
          title={opt.title}
          aria-pressed={pref === opt.pref ? "true" : "false"}
          onClick={() => setPref(opt.pref)}
        >
          {opt.icon}
        </button>
      ))}
    </div>
  );
}
