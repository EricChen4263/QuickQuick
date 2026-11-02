import type { TopLevel } from "../main-window/nav";

interface TopNavProps {
  activeTop: TopLevel;
  onNavigate: (top: TopLevel) => void;
}

interface NavEntry {
  key: TopLevel;
  label: string;
  icon: React.ReactNode;
}

/**
 * 图标统一走 Phosphor 描边语汇：24 网格、stroke 1.5、圆头圆角。
 * 全部内联 SVG，不引图标库，也不挂 Lucide / Feather / Material 的形状。
 */
const NAV_ENTRIES: NavEntry[] = [
  {
    key: "clipboard",
    label: "剪贴板",
    icon: (
      <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M9 4H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2h-2" />
        <rect x="9" y="2.5" width="6" height="3.5" rx="1" />
      </svg>
    ),
  },
  {
    key: "translate",
    label: "翻译",
    icon: (
      <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M4 6h9M8 3v3M12 6c0 4.5-3.2 7.6-8 9" />
        <path d="m13.5 20 4-9 4 9M15.2 17h4.6" />
      </svg>
    ),
  },
  {
    key: "settings",
    label: "设置",
    icon: (
      <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M4 6.5h4M12 6.5h8M4 12h8M16 12h4M4 17.5h2M10 17.5h10" />
        <circle cx="10" cy="6.5" r="1.8" />
        <circle cx="14" cy="12" r="1.8" />
        <circle cx="8" cy="17.5" r="1.8" />
      </svg>
    ),
  },
];

/**
 * 一级导航：横向文字导航 + 下划线指示，嵌在顶部标题栏的中部插槽里。
 *
 * 选中态只用「加重字色 + 2px 下划线」表达，不用强调色底色药丸——
 * 那是这类应用最泛化的做法，也会让顶栏出现三块色斑。
 */
export function TopNav({ activeTop, onNavigate }: TopNavProps) {
  return (
    <nav aria-label="主导航" className="qq-topnav">
      {NAV_ENTRIES.map((entry) => (
        <button
          key={entry.key}
          type="button"
          className="qq-nav-item body-5"
          aria-current={activeTop === entry.key ? "page" : undefined}
          onClick={() => onNavigate(entry.key)}
        >
          {entry.icon}
          {entry.label}
        </button>
      ))}
    </nav>
  );
}
