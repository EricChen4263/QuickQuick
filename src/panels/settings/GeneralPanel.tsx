import { useState } from "react";
import {
  checkForUpdates,
  downloadAndInstallUpdate,
} from "../../ipc/ipc-client";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";
import SettingToggle from "./SettingToggle";
import { useGeneralSettings } from "./useGeneralSettings";

/** 检查更新的结果：是否有新版本及其版本号。 */
interface UpdateCheckOutcome {
  available: boolean;
  version: string;
}

/** 检查更新操作的反馈状态。 */
interface UpdateCheckFeedback {
  outcome: UpdateCheckOutcome | null;
  msg: string | null;
  error: string | null;
}

/** 执行检查更新并返回反馈状态。*/
async function runCheckForUpdates(): Promise<UpdateCheckFeedback> {
  try {
    const result = await checkForUpdates();
    if (result.available) {
      return {
        outcome: { available: true, version: result.version },
        msg: `发现新版本 ${result.version}`,
        error: null,
      };
    }
    return { outcome: { available: false, version: "" }, msg: "已是最新版本", error: null };
  } catch {
    return {
      outcome: null,
      msg: null,
      error: "检查更新失败，可能更新服务尚未配置",
    };
  }
}

/** 下载安装的成功提示与失败告警，按状态择一渲染。 */
function InstallFeedback({ doneMsg, error }: { doneMsg: string | null; error: string | null }) {
  return (
    <>
      {doneMsg !== null && (
        <div style={{ marginTop: 6, fontSize: 12, color: "var(--muted)" }}>
          {doneMsg}
        </div>
      )}
      {error !== null && (
        <div
          role="alert"
          style={{ color: "var(--danger)", marginTop: 6, fontSize: 12 }}
        >
          {error}
        </div>
      )}
    </>
  );
}

/** 发现新版后的「下载并安装」操作区：触发下载安装并反馈进度/结果。 */
function UpdateInstallAction({ version }: { version: string }) {
  const [isInstalling, setIsInstalling] = useState(false);
  const [doneMsg, setDoneMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleInstall() {
    setError(null);
    setDoneMsg(null);
    setIsInstalling(true);
    try {
      await downloadAndInstallUpdate();
      setDoneMsg("已下载，待重启生效");
    } catch {
      setError("下载安装失败，请稍后重试");
    } finally {
      setIsInstalling(false);
    }
  }

  return (
    <div style={{ marginTop: 8 }}>
      <div style={{ fontSize: 12, color: "var(--muted)" }}>
        发现新版本 {version}
      </div>
      <button
        className="btn"
        type="button"
        disabled={isInstalling}
        style={{ marginTop: 6 }}
        onClick={() => {
          void handleInstall();
        }}
      >
        {isInstalling ? "下载中…" : "下载并安装"}
      </button>
      <InstallFeedback doneMsg={doneMsg} error={error} />
    </div>
  );
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

  const [outcome, setOutcome] = useState<UpdateCheckOutcome | null>(null);
  const [checkMsg, setCheckMsg] = useState<string | null>(null);
  const [checkError, setCheckError] = useState<string | null>(null);
  const [isChecking, setIsChecking] = useState(false);

  async function handleCheckUpdate() {
    setCheckMsg(null);
    setCheckError(null);
    setOutcome(null);
    setIsChecking(true);
    try {
      const feedback = await runCheckForUpdates();
      setOutcome(feedback.outcome);
      setCheckMsg(feedback.msg);
      setCheckError(feedback.error);
    } finally {
      setIsChecking(false);
    }
  }

  const hasUpdate = outcome?.available === true;

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

      {hasUpdate ? (
        <UpdateInstallAction version={outcome.version} />
      ) : (
        checkMsg !== null && (
          <div style={{ marginTop: 8, fontSize: 12, color: "var(--muted)" }}>
            {checkMsg}
          </div>
        )
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
