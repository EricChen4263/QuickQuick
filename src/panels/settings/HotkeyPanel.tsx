import { useEffect, useState, useCallback } from "react";
import { getHotkeys, setHotkey, type Hotkeys, type HotkeyAction } from "../../ipc/ipc-client";
import { validateRebind } from "../../main-window/settings/rebind";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";
import SettingToggle from "./SettingToggle";

interface HotkeyRowProps {
  action: HotkeyAction;
  label: string;
  description: string;
  currentValue: string;
  occupiedValues: string[];
  onSaved: () => void;
}

/** 单行热键编辑：kbd 展示当前键 + input 改键 + 保存按钮 + 冲突校验 */
function HotkeyRow({
  action,
  label,
  description,
  currentValue,
  occupiedValues,
  onSaved,
}: HotkeyRowProps) {
  const [inputValue, setInputValue] = useState(currentValue);
  const [conflictError, setConflictError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  // 父组件刷新 currentValue 时（保存成功后重拉）同步输入框
  useEffect(() => {
    setInputValue(currentValue);
    setConflictError(null);
    setSaveError(null);
  }, [currentValue]);

  async function handleSave() {
    const result = validateRebind(inputValue, occupiedValues);
    if (!result.ok) {
      setConflictError(result.error);
      return;
    }
    setConflictError(null);
    try {
      await setHotkey(action, result.accelerator);
      setSaveError(null);
      onSaved();
    } catch (err: unknown) {
      setSaveError(err instanceof Error ? err.message : "保存失败，请稍后重试");
    }
  }

  const errorMessage = conflictError ?? saveError;

  // kbd 展示：按 + 分段，每段一个 <kbd>
  const kbdParts = currentValue.split("+");

  return (
    <div className="set-row column">
      <div className="hotkey-row-inner">
        <div className="grow">
          <div className="label">{label}</div>
          <div className="desc">{description}</div>
        </div>
        <span className="kbd-combo">
          {kbdParts.map((part) => (
            <kbd key={part}>{part}</kbd>
          ))}
        </span>
        <input
          type="text"
          className="set-input"
          value={inputValue}
          onChange={(e) => {
            setInputValue(e.target.value);
            setConflictError(null);
            setSaveError(null);
          }}
        />
        <button className="btn" onClick={() => void handleSave()}>保存</button>
      </div>
      {errorMessage !== null && (
        <div role="alert" className="hotkey-error">
          {errorMessage}
        </div>
      )}
    </div>
  );
}

/** 热键子项面板：加载热键配置，渲染 history + translate 两行编辑 */
function HotkeyPanel() {
  const [hotkeys, setHotkeys] = useState<Hotkeys | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  // 里程碑3接入 IPC 前的本地占位：回车粘贴开关
  const [enterToPaste, setEnterToPaste] = useState(true);

  const fetchHotkeys = useCallback(async (cancelled: { current: boolean }) => {
    try {
      const result = await getHotkeys();
      if (cancelled.current) return;
      setHotkeys(result);
      setLoadError(null);
    } catch {
      if (cancelled.current) return;
      setLoadError("热键加载失败，请稍后重试");
    }
  }, []);

  useEffect(() => {
    const cancelled = { current: false };
    fetchHotkeys(cancelled);
    return () => {
      cancelled.current = true;
    };
  }, [fetchHotkeys]);

  function handleSaved() {
    const cancelled = { current: false };
    fetchHotkeys(cancelled);
  }

  if (loadError !== null) {
    return <div role="alert">{loadError}</div>;
  }

  if (hotkeys === null) {
    return <div>加载中…</div>;
  }

  return (
    <div>
      <PanelHeader title="热键" subtitle="自定义全局快捷键，与其他应用冲突时可修改" />
      <SettingGroup>
        <HotkeyRow
          action="history"
          label="剪贴板历史"
          description="唤出剪贴板历史面板"
          currentValue={hotkeys.history}
          occupiedValues={[hotkeys.translate]}
          onSaved={handleSaved}
        />
        <HotkeyRow
          action="translate"
          label="翻译选中"
          description="翻译当前选中文字"
          currentValue={hotkeys.translate}
          occupiedValues={[hotkeys.history]}
          onSaved={handleSaved}
        />
      </SettingGroup>
      {/* 里程碑3接入：回车粘贴开关，当前用本地 state 占位 */}
      <SettingGroup>
        <SettingToggle
          label="回车粘贴"
          description="在历史面板按回车时自动粘贴到前台应用"
          checked={enterToPaste}
          onChange={setEnterToPaste}
        />
      </SettingGroup>
    </div>
  );
}

export default HotkeyPanel;
