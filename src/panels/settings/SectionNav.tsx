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

interface SectionNavProps {
  sections: SettingsSection[];
  activeSection: SettingsSection;
  onSelect: (section: SettingsSection) => void;
}

/** 设置页左侧纵向子项导航栏 */
function SectionNav({ sections, activeSection, onSelect }: SectionNavProps) {
  return (
    <nav
      aria-label="设置子项"
      style={{ display: "flex", flexDirection: "column", width: 160, borderRight: "1px solid var(--qq-border, #e0e0e0)" }}
    >
      {sections.map((section) => (
        <button
          key={section}
          aria-current={activeSection === section ? "page" : undefined}
          onClick={() => onSelect(section)}
          style={{
            padding: "10px 16px",
            textAlign: "left",
            background: activeSection === section ? "var(--qq-accent-bg, #e8f4fd)" : "transparent",
            border: "none",
            cursor: "pointer",
            fontFamily: "var(--qq-font)",
          }}
        >
          {SECTION_LABELS[section]}
        </button>
      ))}
    </nav>
  );
}

export default SectionNav;
