import { useTheme } from "../theme/useTheme";
import type { ThemePref } from "../theme/themeStore";

interface SegOption {
  pref: ThemePref;
  label: string;
  /** 完整说明，作为 title 与读屏补充；按钮文字本身只放最短的口径 */
  title: string;
}

/**
 * 三档主题选项。
 *
 * 刻意用纯文字而不是太阳 / 月亮 / 显示器图标：
 * 「亮暗切换做成太阳月亮开关」是最典型的 AI 默认套路之一，
 * 而且在这个体量下三个图标反而比三个词更难一眼分辨。
 */
const OPTIONS: SegOption[] = [
  { pref: "auto", label: "跟随系统", title: "跟随系统" },
  { pref: "light", label: "浅色", title: "浅色" },
  { pref: "dark", label: "深色", title: "深色" },
];

/** 三档主题切换分段控件，跟随 themeStore 状态，点击写入 store */
export function ThemeSwitch() {
  const { pref, setPref } = useTheme();

  return (
    <div className="theme-seg caption-1" role="group" aria-label="外观">
      {OPTIONS.map((opt) => (
        <button
          key={opt.pref}
          type="button"
          title={opt.title}
          aria-pressed={pref === opt.pref ? "true" : "false"}
          onClick={() => setPref(opt.pref)}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}
