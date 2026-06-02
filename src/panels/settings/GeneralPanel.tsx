import { useState } from "react";
import { checkForUpdates } from "../../ipc/ipc-client";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";
import SettingToggle from "./SettingToggle";
import { useGeneralSettings } from "./useGeneralSettings";

/** 检查更新操作的反馈状态。 */
interface UpdateCheckFeedback {
  msg: string | null;
  error: string | null;
}

/** 执行检查更新并返回反馈文案。*/
async function runCheckForUpdates(): Promise<UpdateCheckFeedback> {
  try {
    const result = await checkForUpdates();
    if (result.available) {
      return { msg: `发现新版本 ${result.version}，可前往下载`, error: null };
    }
    return { msg: "已是最新版本", error: null };
  } catch {
    return {
      msg: null,
      error: "检查更新失败，可能更新服务尚未配置",
    };
  }
}

/** 通用子项面板：开机自启动 / 托盘常驻 / 自动检查更新 / 立即检查更新 */
function GeneralPanel() {
  const {
    launchOnLogin,
    setLaunchOnLogin,
    stayInTray,
    setStayInTray,
    autoUpdate,
    setAutoUpdate,
  } = useGeneralSettings();

  const [checkMsg, setCheckMsg] = useState<string | null>(null);
  const [checkError, setCheckError] = useState<string | null>(null);
  const [isChecking, setIsChecking] = useState(false);

  async function handleCheckUpdate() {
    setCheckMsg(null);
    setCheckError(null);
    setIsChecking(true);
    try {
      const feedback = await runCheckForUpdates();
      setCheckMsg(feedback.msg);
      setCheckError(feedback.error);
    } finally {
      setIsChecking(false);
    }
  }

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
        <div className="set-row">
          <div className="grow">
            <div className="label">立即检查更新</div>
            <div className="desc">手动检查新版本</div>
          </div>
          <button
            className="btn"
            type="button"
            disabled={isChecking}
            onClick={() => {
              void handleCheckUpdate();
            }}
          >
            {isChecking ? "检查中…" : "检查"}
          </button>
        </div>
      </SettingGroup>

      {checkMsg !== null && (
        <div style={{ marginTop: 8, fontSize: 12, color: "var(--muted)" }}>
          {checkMsg}
        </div>
      )}
      {checkError !== null && (
        <div
          role="alert"
          style={{ color: "var(--danger)", marginTop: 8, fontSize: 12 }}
        >
          {checkError}
        </div>
      )}
    </div>
  );
}

export default GeneralPanel;
