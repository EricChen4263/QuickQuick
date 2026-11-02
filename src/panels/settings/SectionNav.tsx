import type { ReactElement } from "react";
import type { SettingsSection } from "../../main-window/settings/sections";

/** 设置子项中文标签映射（具名常量，避免魔术字符串） */
export const SECTION_LABELS: Record<SettingsSection, string> = {
  general: "通用",
  appearance: "外观",
  hotkey: "热键",
  "translate-source": "翻译源",
  privacy: "隐私",
  storage: "存储",
  about: "关于",
};

/**
 * 各子项对应的内联 SVG 图标。
 *
 * 全部走 .ico 基类（24 网格 / stroke 1.5 / 圆头圆角，Phosphor 语汇），
 * 不在各处重复写 stroke-width；尺寸交给 .ico-16。形状各自扣题，
 * 不用通用齿轮堆满整列。
 */
const SECTION_ICONS: Record<SettingsSection, ReactElement> = {
  general: (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M10.55 2.81 13.45 2.81 13.68 5 15.76 5.86 17.47 4.48 19.52 6.53 18.14 8.24 19 10.32 21.19 10.55 21.19 13.45 19 13.68 18.14 15.76 19.52 17.47 17.47 19.52 15.76 18.14 13.68 19 13.45 21.19 10.55 21.19 10.32 19 8.24 18.14 6.53 19.52 4.48 17.47 5.86 15.76 5 13.68 2.81 13.45 2.81 10.55 5 10.32 5.86 8.24 4.48 6.53 6.53 4.48 8.24 5.86 10.32 5Z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  ),
  appearance: (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 3a9 9 0 0 1 0 18Z" fill="currentColor" stroke="none" />
    </svg>
  ),
  hotkey: (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <rect x="2.5" y="6" width="19" height="12" rx="2.5" />
      <path d="M6.5 10h.01M10 10h.01M13.5 10h.01M17 10h.01M8 14h8" />
    </svg>
  ),
  "translate-source": (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M3.5 6h9M8 6c0 4.2-1.6 7.4-4.5 9.4" />
      <path d="M6 10.5c1.3 2.1 3.2 3.6 5.6 4.4" />
      <path d="m12.5 20.5 4.2-9.6 4.2 9.6M14.2 17.2h5" />
    </svg>
  ),
  privacy: (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 2.8 19 5.5v5.6c0 4.5-2.9 8.4-7 9.6-4.1-1.2-7-5.1-7-9.6V5.5Z" />
      <path d="m9 11.8 2.2 2.2 4.2-4.4" />
    </svg>
  ),
  storage: (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <ellipse cx="12" cy="5.5" rx="7.5" ry="2.8" />
      <path d="M4.5 5.5v13c0 1.5 3.4 2.8 7.5 2.8s7.5-1.3 7.5-2.8v-13" />
      <path d="M4.5 12c0 1.5 3.4 2.8 7.5 2.8s7.5-1.3 7.5-2.8" />
    </svg>
  ),
  about: (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 11.5v5" />
      <path d="M12 8h.01" />
    </svg>
  ),
};

interface SectionNavProps {
  sections: SettingsSection[];
  activeSection: SettingsSection;
  onSelect: (section: SettingsSection) => void;
}

/**
 * 设置页左侧纵向子项导航。
 *
 * 选中态用 aria-current="page" 表达，视觉上只做两件事：左侧 2px 强调色竖条
 * + 标签加重字重。刻意不做「强调色底色药丸」——那会把导航变成一排按钮，
 * 让 7 个平级入口看起来像 7 个操作。
 */
function SectionNav({ sections, activeSection, onSelect }: SectionNavProps) {
  return (
    <nav className="set-nav" aria-label="设置子项">
      {sections.map((section) => {
        const isActive = activeSection === section;
        return (
          <button
            key={section}
            type="button"
            className="set-nav-item"
            aria-current={isActive ? "page" : undefined}
            onClick={() => onSelect(section)}
          >
            {SECTION_ICONS[section]}
            <span className={isActive ? "body-4" : "caption-1"}>
              {SECTION_LABELS[section]}
            </span>
          </button>
        );
      })}
    </nav>
  );
}

export default SectionNav;
