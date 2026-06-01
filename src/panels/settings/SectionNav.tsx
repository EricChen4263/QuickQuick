import type { ReactElement } from "react";
import type { SettingsSection } from "../../main-window/settings/sections";

/** 设置子项中文标签映射（具名常量，避免魔术字符串） */
export const SECTION_LABELS: Record<SettingsSection, string> = {
  general: "通用",
  hotkey: "热键",
  "translate-source": "翻译源",
  privacy: "隐私",
  storage: "存储",
  about: "关于",
};

/** 各子项对应的内联 SVG 图标（来自设计稿，stroke-width=1.6） */
const SECTION_ICONS: Record<SettingsSection, ReactElement> = {
  general: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="3"/>
      <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>
    </svg>
  ),
  hotkey: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <rect x="2" y="6" width="20" height="12" rx="2"/>
      <path d="M6 10h0M10 10h0M14 10h0M18 10h0M6 14h12"/>
    </svg>
  ),
  "translate-source": (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="m5 8 6 6"/>
      <path d="m4 14 6-6 2-3"/>
      <path d="M2 5h12"/>
      <path d="m22 22-5-10-5 10"/>
      <path d="M14 18h6"/>
    </svg>
  ),
  privacy: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
    </svg>
  ),
  storage: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <ellipse cx="12" cy="5" rx="9" ry="3"/>
      <path d="M3 5v14a9 3 0 0 0 18 0V5"/>
      <path d="M3 12a9 3 0 0 0 18 0"/>
    </svg>
  ),
  about: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="10"/>
      <path d="M12 16v-4M12 8h.01"/>
    </svg>
  ),
};

interface SectionNavProps {
  sections: SettingsSection[];
  activeSection: SettingsSection;
  onSelect: (section: SettingsSection) => void;
}

/** 设置页左侧纵向子项导航栏：图标 + 文字，选中态用 aria-current="page" */
function SectionNav({ sections, activeSection, onSelect }: SectionNavProps) {
  return (
    <nav className="set-nav" aria-label="设置子项">
      {sections.map((section) => (
        <button
          key={section}
          className="set-nav-item"
          aria-current={activeSection === section ? "page" : undefined}
          onClick={() => onSelect(section)}
        >
          {SECTION_ICONS[section]}
          {SECTION_LABELS[section]}
        </button>
      ))}
    </nav>
  );
}

export default SectionNav;
