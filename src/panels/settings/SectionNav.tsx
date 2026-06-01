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
            // fallback 值与 theme.css 浅色 token 同值，防 CSS 未加载时选中态无背景
            background: activeSection === section ? "var(--qq-accent-bg, rgba(58, 124, 165, 0.12))" : "transparent",
            border: "none",
            cursor: "pointer",
            fontFamily: "var(--qq-font)",
            // 显式 token 避免依赖 DOM 继承，保证非选中态颜色可预测
            color: activeSection === section ? "var(--qq-accent)" : "var(--qq-text)",
          }}
        >
          {SECTION_LABELS[section]}
        </button>
      ))}
    </nav>
  );
}

export default SectionNav;
