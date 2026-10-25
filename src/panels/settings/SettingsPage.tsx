import { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import appIcon from "../../assets/app-icon.png";
import "./settings.css";
import { settingsSections, type SettingsSection } from "../../main-window/settings/sections";
import SectionNav from "./SectionNav";
import HotkeyPanel from "./HotkeyPanel";
import TranslateSourcePanel from "./TranslateSourcePanel";
import PrivacyPanel from "./PrivacyPanel";
import StoragePanel from "./StoragePanel";
import GeneralPanel from "./GeneralPanel";
import AppearancePanel from "./AppearancePanel";
import {
  SettingsSaveErrorBanner,
  SettingsSaveErrorProvider,
} from "./settings-save-error";

/** 关于子项面板：logo + 应用名 + 版本号 + 描述 */
function AboutPanel() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    const cancelled = { current: false };
    getVersion().then((v) => {
      if (!cancelled.current) setVersion(v);
    }).catch((err: unknown) => {
      console.error("getVersion failed", err);
    });
    return () => {
      cancelled.current = true;
    };
  }, []);

  const versionText = version !== null ? `v${version}` : "v…";

  return (
    <div className="about">
      <div className="logo-mark">
        <img src={appIcon} alt="QuickQuick" />
      </div>
      <h2><span className="brand-accent">Quick</span>Quick</h2>
      <div className="ver num">{versionText}</div>
      <p>托盘常驻的剪贴板历史 + 翻译小工具。本地加密存储，每台机器各一份、互相独立。</p>
    </div>
  );
}

/** 根据当前选中子项渲染对应内容面板 */
function SectionContent({ section }: { section: SettingsSection }) {
  if (section === "general") return <GeneralPanel />;
  if (section === "appearance") return <AppearancePanel />;
  if (section === "hotkey") return <HotkeyPanel />;
  if (section === "translate-source") return <TranslateSourcePanel />;
  if (section === "privacy") return <PrivacyPanel />;
  if (section === "storage") return <StoragePanel />;
  return <AboutPanel />;
}

/** 设置页根组件：左侧紧凑子项导航 + 右侧内容区 */
function SettingsPage() {
  const [activeSection, setActiveSection] = useState<SettingsSection>("general");
  const sections = settingsSections();

  return (
    <SettingsSaveErrorProvider>
      <div className="set-page">
        <SectionNav
          sections={sections}
          activeSection={activeSection}
          onSelect={(section) => setActiveSection(() => section)}
        />
        <main className="set-body">
          {/* 页面级保存失败出口：所有面板的写失败都路由到这一条，文案是后端原文 */}
          <SettingsSaveErrorBanner />
          <SectionContent section={activeSection} />
        </main>
      </div>
    </SettingsSaveErrorProvider>
  );
}

export default SettingsPage;
