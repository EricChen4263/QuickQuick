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

/**
 * 关于子项面板：logo + 应用名 + 版本号 + 一句定位说明。
 *
 * 这里是全页唯一「非表单」的区块，所以不给它套设置行，而是一块居中的品牌区。
 */
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
      <h2 className="heading-5"><span className="brand-accent">Quick</span>Quick</h2>
      <div className="ver num caption-2 mono">{versionText}</div>
      <p className="body-6">
        托盘常驻的剪贴板历史 + 翻译小工具。本地加密存储，每台机器各一份、互相独立。
      </p>
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

/**
 * 设置页根组件：左侧二级导航（160px 固定）+ 右侧内容区。
 *
 * 布局分工：本页只有内容区滚动，导航固定不动；内容列宽由 settings.css 限制在
 * 阅读宽度内，右侧留白是有意留的，不是没铺满。
 */
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
