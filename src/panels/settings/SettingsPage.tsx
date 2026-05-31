import { useState } from "react";
import { settingsSections, type SettingsSection } from "../../main-window/settings/sections";
import SectionNav from "./SectionNav";
import HotkeyPanel from "./HotkeyPanel";
import TranslateSourcePanel from "./TranslateSourcePanel";
import PrivacyPanel from "./PrivacyPanel";

/** 通用子项面板（最小真实内容） */
function GeneralPanel() {
  return (
    <div style={{ padding: 24 }}>
      <h2 style={{ fontFamily: "var(--qq-font)", marginTop: 0 }}>通用</h2>
      <p style={{ fontFamily: "var(--qq-font)", color: "var(--qq-text-muted)" }}>
        通用设置将在后续版本中开放配置。
      </p>
    </div>
  );
}

/** 存储子项面板（最小真实内容） */
function StoragePanel() {
  return (
    <div style={{ padding: 24 }}>
      <h2 style={{ fontFamily: "var(--qq-font)", marginTop: 0 }}>存储</h2>
      <p style={{ fontFamily: "var(--qq-font)", color: "var(--qq-text-muted)" }}>
        存储管理（数据清理、容量限制）将在后续版本中开放配置。
      </p>
    </div>
  );
}

/** 关于子项面板 */
function AboutPanel() {
  return (
    <div style={{ padding: 24 }}>
      <h2 style={{ fontFamily: "var(--qq-font)", marginTop: 0 }}>关于</h2>
      <p style={{ fontFamily: "var(--qq-font)", fontWeight: "bold", fontSize: 18 }}>
        QuickQuick
      </p>
      <p style={{ fontFamily: "var(--qq-font)", color: "var(--qq-text-muted)" }}>
        跨平台剪贴板历史与翻译工具，基于 Tauri 2 + React 构建。
      </p>
    </div>
  );
}

/** 根据当前选中子项渲染对应内容面板 */
function SectionContent({ section }: { section: SettingsSection }) {
  if (section === "general") return <GeneralPanel />;
  if (section === "hotkey") return <HotkeyPanel />;
  if (section === "translate-source") return <TranslateSourcePanel />;
  if (section === "privacy") return <PrivacyPanel />;
  if (section === "storage") return <StoragePanel />;
  return <AboutPanel />;
}

/** 设置页根组件：左侧纵向子项栏 + 右侧内容区 */
function SettingsPage() {
  const [activeSection, setActiveSection] = useState<SettingsSection>("general");
  const sections = settingsSections();

  return (
    <div style={{ display: "flex", height: "100%", fontFamily: "var(--qq-font)" }}>
      <SectionNav
        sections={sections}
        activeSection={activeSection}
        onSelect={(section) => setActiveSection(() => section)}
      />
      <main style={{ flex: 1, overflowY: "auto" }}>
        <SectionContent section={activeSection} />
      </main>
    </div>
  );
}

export default SettingsPage;
