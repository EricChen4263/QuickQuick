import { useEffect, useState, useCallback } from "react";
import { getExcludeList, setExcludeList } from "../../ipc/ipc-client";
import { addExcludedApp, removeExcludedApp } from "../../main-window/settings/sections";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";
import SettingToggle from "./SettingToggle";

/** SVG × 图标，用于 chip 删除按钮 */
function CloseIcon() {
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
      <path d="M2 2l8 8M10 2l-8 8" />
    </svg>
  );
}

/** 隐私子项面板：管理 App 排除名单 + 暂停/跳过敏感开关 */
function PrivacyPanel() {
  const [excludeList, setExcludeListState] = useState<string[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);
  // 里程碑3接入 IPC 前的本地占位开关
  const [pauseCapture, setPauseCapture] = useState(false);
  const [skipSensitive, setSkipSensitive] = useState(true);

  const fetchExcludeList = useCallback(async (cancelled: { current: boolean }) => {
    try {
      const list = await getExcludeList();
      if (cancelled.current) return;
      setExcludeListState(list);
      setLoadError(null);
    } catch {
      if (cancelled.current) return;
      setLoadError("排除名单加载失败，请稍后重试");
    }
  }, []);

  useEffect(() => {
    const cancelled = { current: false };
    fetchExcludeList(cancelled);
    return () => {
      cancelled.current = true;
    };
  }, [fetchExcludeList]);

  async function handleAdd() {
    const trimmed = inputValue.trim();
    if (trimmed.length === 0) return;

    const newList = addExcludedApp(excludeList, trimmed);
    try {
      await setExcludeList(newList);
      setExcludeListState(newList);
      setInputValue("");
      setOpError(null);
    } catch {
      setOpError("添加失败，请稍后重试");
    }
  }

  async function handleRemove(app: string) {
    const newList = removeExcludedApp(excludeList, app);
    try {
      await setExcludeList(newList);
      setExcludeListState(newList);
      setOpError(null);
    } catch {
      setOpError("删除失败，请稍后重试");
    }
  }

  if (loadError !== null) {
    return (
      <div>
        <div role="alert" style={{ color: "var(--danger)" }}>{loadError}</div>
      </div>
    );
  }

  return (
    <div>
      <PanelHeader title="隐私" subtitle="以下应用处于前台时，剪贴板内容不会被记录" />

      {/* 里程碑3接入：暂停监听和跳过敏感内容，当前用本地 state 占位 */}
      <SettingGroup>
        <SettingToggle
          label="暂停剪贴板监听"
          description="临时停止记录所有剪贴板内容"
          checked={pauseCapture}
          onChange={setPauseCapture}
        />
        <SettingToggle
          label="跳过敏感内容"
          description="自动过滤密码管理器等敏感数据"
          checked={skipSensitive}
          onChange={setSkipSensitive}
        />
      </SettingGroup>

      <SettingGroup>
        <div className="set-row">
          <div className="grow">
            <div className="label">App 排除名单</div>
            <div className="desc">处于前台时不记录剪贴板的应用</div>
          </div>
          <input
            type="text"
            className="set-input"
            value={inputValue}
            placeholder="应用名称（如 1Password）"
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") void handleAdd(); }}
          />
          <button className="btn" onClick={() => void handleAdd()}>添加</button>
        </div>
        <div className="chip-row">
          {excludeList.map((app) => (
            <span key={app} className="chip">
              {app}
              <button
                type="button"
                aria-label={`删除 ${app}`}
                onClick={() => void handleRemove(app)}
              >
                <CloseIcon />
              </button>
            </span>
          ))}
          {excludeList.length === 0 && (
            <span style={{ fontSize: 12, color: "var(--muted)" }}>暂无排除应用</span>
          )}
        </div>
      </SettingGroup>

      {opError !== null && (
        <div role="alert" style={{ color: "var(--danger)", marginTop: 8, fontSize: 12 }}>
          {opError}
        </div>
      )}
    </div>
  );
}

export default PrivacyPanel;
