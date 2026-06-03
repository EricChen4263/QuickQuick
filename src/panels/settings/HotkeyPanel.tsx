import { useEffect, useState, useCallback, useRef } from "react";
import { getHotkeys, setHotkey, type Hotkeys, type HotkeyAction } from "../../ipc/ipc-client";
import { validateRebind } from "../../main-window/settings/rebind";
import { keyEventToAccelerator } from "../../main-window/settings/key-capture";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";

interface HotkeyRowProps {
  action: HotkeyAction;
  label: string;
  description: string;
  currentValue: string;
  occupiedValues: string[];
  onSaved: () => void;
}

/** kbd 芯片组：按 + 分段渲染每段为一个 <kbd> */
function KbdCombo({ accelerator }: { accelerator: string }) {
  return (
    <span className="kbd-combo">
      {accelerator.split("+").map((part) => (
        <kbd key={part}>{part}</kbd>
      ))}
    </span>
  );
}

/** 单行热键编辑：kbd 展示当前键 + 录制模式捕获 + 保存/取消 */
function HotkeyRow({
  action,
  label,
  description,
  currentValue,
  occupiedValues,
  onSaved,
}: HotkeyRowProps) {
  const [isRecording, setIsRecording] = useState(false);
  const [captured, setCaptured] = useState<string | null>(null);
  const [conflictError, setConflictError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const captureRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    setIsRecording(false);
    setCaptured(null);
    setConflictError(null);
    setSaveError(null);
  }, [currentValue]);

  function enterRecording() {
    setIsRecording(true);
    setCaptured(null);
    setConflictError(null);
    setSaveError(null);
    setTimeout(() => captureRef.current?.focus(), 0);
  }

  function cancelRecording() {
    setIsRecording(false);
    setCaptured(null);
    setConflictError(null);
    setSaveError(null);
  }

  function handleCaptureKeyDown(e: React.KeyboardEvent<HTMLButtonElement>) {
    e.preventDefault();

    if (e.code === "Escape") {
      cancelRecording();
      return;
    }

    const accel = keyEventToAccelerator({
      metaKey: e.metaKey,
      ctrlKey: e.ctrlKey,
      altKey: e.altKey,
      shiftKey: e.shiftKey,
      code: e.code,
    });

    if (accel !== null) {
      setCaptured(accel);
    }
  }

  async function handleSave() {
    if (captured === null) return;

    const result = validateRebind(captured, occupiedValues);
    if (!result.ok) {
      setConflictError(result.error);
      return;
    }
    setConflictError(null);
    try {
      await setHotkey(action, result.accelerator);
      setSaveError(null);
      setIsRecording(false);
      setCaptured(null);
      onSaved();
    } catch (err: unknown) {
      setSaveError(err instanceof Error ? err.message : "保存失败，请稍后重试");
    }
  }

  const errorMessage = conflictError ?? saveError;

  if (isRecording) {
    return (
      <div className="set-row column">
        <div className="hotkey-row-inner">
          <div className="grow">
            <div className="label">{label}</div>
            <div className="desc">{description}</div>
          </div>
          {captured !== null ? (
            <KbdCombo accelerator={captured} />
          ) : null}
          <button
            ref={captureRef}
            className="btn capture-area"
            aria-label="录制中…请按下快捷键"
            onKeyDown={handleCaptureKeyDown}
          >
            录制中…请按下快捷键
          </button>
          <button
            className="btn btn-primary"
            disabled={captured === null}
            onClick={() => void handleSave()}
          >
            保存
          </button>
          <button className="btn" onClick={cancelRecording}>
            取消
          </button>
        </div>
        {errorMessage !== null && (
          <div role="alert" className="hotkey-error">
            {errorMessage}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="set-row column">
      <div className="hotkey-row-inner">
        <div className="grow">
          <div className="label">{label}</div>
          <div className="desc">{description}</div>
        </div>
        <KbdCombo accelerator={currentValue} />
        <button className="btn" onClick={enterRecording}>
          修改
        </button>
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
    </div>
  );
}

export default HotkeyPanel;
