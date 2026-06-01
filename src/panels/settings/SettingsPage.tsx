import { useState } from "react";
import "./settings.css";
import { settingsSections, type SettingsSection } from "../../main-window/settings/sections";
import SectionNav from "./SectionNav";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";
import SettingToggle from "./SettingToggle";
import HotkeyPanel from "./HotkeyPanel";
import TranslateSourcePanel from "./TranslateSourcePanel";
import PrivacyPanel from "./PrivacyPanel";
import { useGeneralSettings } from "./useGeneralSettings";

/** 通用子项面板：开机自启动 / 托盘常驻 / 自动检查更新 */
function GeneralPanel() {
  const {
    launchOnLogin, setLaunchOnLogin,
    stayInTray, setStayInTray,
    autoUpdate, setAutoUpdate,
  } = useGeneralSettings();

  return (
    <div>
      <PanelHeader title="通用" subtitle="启动方式与基础行为。" />
      <SettingGroup>
        <SettingToggle
          label="开机自启动"
          description="登录系统时在后台启动 QuickQuick"
          checked={launchOnLogin}
          onChange={setLaunchOnLogin}
        />
        <SettingToggle
          label="托盘常驻"
          description="关闭窗口后保留托盘图标与剪贴板监听"
          checked={stayInTray}
          onChange={setStayInTray}
        />
        <SettingToggle
          label="自动检查更新"
          description="通过签名清单静默拉取新版本"
          checked={autoUpdate}
          onChange={setAutoUpdate}
        />
      </SettingGroup>
    </div>
  );
}

/** 存储子项面板：库体积进度条 + 上限/阈值展示 + 立即清理按钮（里程碑3 接 IPC） */
function StoragePanel() {
  return (
    <div>
      <PanelHeader title="存储" subtitle="加密库 quickquick.db · 分级清理，收藏永远豁免。" />
      <SettingGroup>
        <div className="set-row">
          <div className="grow">
            <div className="label">库体积</div>
            {/* 静态占位，里程碑3 替换为 IPC 读取实际值 */}
            <div className="meter" aria-hidden="true">
              <i style={{ width: "47%" }} />
            </div>
            <div className="meter-legend">
              <span>234 MB 已用</span>
              <span>上限 500 MB</span>
            </div>
          </div>
        </div>
        <div className="set-row">
          <div className="grow">
            <div className="label">总量上限</div>
            <div className="desc">超出按分级清理：先删大图原图保缩略图，再整条清最旧非收藏</div>
          </div>
          <span className="kbd-combo num">
            <kbd>500 MB</kbd>
          </span>
        </div>
        <div className="set-row">
          <div className="grow">
            <div className="label">单张图片阈值</div>
            <div className="desc">超过阈值默认只留缩略图并标「原图过大未存」</div>
          </div>
          <span className="kbd-combo num">
            <kbd>20 MB</kbd>
          </span>
        </div>
        <div className="set-row">
          <div className="grow">
            <div className="label">立即清理非收藏历史</div>
            <div className="desc">物理清除已软删条目（墓碑 GC）</div>
          </div>
          {/* 里程碑3 接 IPC clear_non_favorite 命令，点击 noop */}
          <button className="btn" type="button" onClick={() => undefined}>
            清理…
          </button>
        </div>
      </SettingGroup>
    </div>
  );
}

/** 关于子项面板：logo + 应用名 + 版本号 + 描述 */
function AboutPanel() {
  return (
    <div className="about">
      <div className="logo-mark">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <rect x="8" y="2" width="8" height="4" rx="1"/>
          <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
          <path d="m9 14 2 2 4-4"/>
        </svg>
      </div>
      <h2>QuickQuick</h2>
      <div className="ver num">v1.0.0 · Tauri 2.0</div>
      <p>托盘常驻的剪贴板历史 + 翻译小工具。本地加密存储，每台机器各一份、互相独立。</p>
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

/** 设置页根组件：左侧 184px 子项导航 + 右侧内容区 */
function SettingsPage() {
  const [activeSection, setActiveSection] = useState<SettingsSection>("general");
  const sections = settingsSections();

  return (
    <div className="set-page">
      <SectionNav
        sections={sections}
        activeSection={activeSection}
        onSelect={(section) => setActiveSection(() => section)}
      />
      <main className="set-body">
        <SectionContent section={activeSection} />
      </main>
    </div>
  );
}

export default SettingsPage;
