import { useEffect, useState, useCallback, useRef } from "react";
import { getHotkeys, setHotkey, type Hotkeys, type HotkeyAction } from "../../ipc/ipc-client";
import { validateRebind } from "../../main-window/settings/rebind";
import { keyEventToAccelerator } from "../../main-window/settings/key-capture";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";
import { useSettingsSaveError } from "./settings-save-error";

interface HotkeyRowConfig {
  action: HotkeyAction;
  label: string;
  description: string;
  /** 出厂默认键位（与 Rust HotkeyConfig::default 逐字一致），用于 tooltip 的「默认：」 */
  defaultAccelerator: string;
}

interface HotkeyRowProps {
  action: HotkeyAction;
  label: string;
  description: string;
  /** 悬停 tooltip（含默认键位） */
  tooltip: string;
  currentValue: string;
  occupiedValues: string[];
  onSaved: () => void;
}

const HOTKEY_ROWS: HotkeyRowConfig[] = [
  {
    action: "history",
    label: "剪贴板历史",
    description: "唤出剪贴板历史面板",
    defaultAccelerator: "CmdOrCtrl+Shift+C",
  },
  {
    action: "translate",
    label: "翻译选中",
    description: "翻译当前选中文字",
    defaultAccelerator: "CmdOrCtrl+Shift+T",
  },
  {
    action: "main",
    label: "应用主界面",
    description: "打开并聚焦 QuickQuick 主窗口",
    defaultAccelerator: "CmdOrCtrl+Shift+M",
  },
];

/** 默认键位的 tooltip 文案（Maccy 的每个设置项都有 .help，我们逐项写明默认值）。 */
function hotkeyTooltip(config: HotkeyRowConfig): string {
  return `默认：${config.defaultAccelerator.replace("CmdOrCtrl", "⌘")}。修改后立即生效；与其它动作重复的键会被拒绝。`;
}

function occupiedValuesFor(action: HotkeyAction, hotkeys: Hotkeys): string[] {
  return HOTKEY_ROWS.filter((row) => row.action !== action).map(
    (row) => hotkeys[row.action]
  );
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
  tooltip,
  currentValue,
  occupiedValues,
  onSaved,
}: HotkeyRowProps) {
  const [isRecording, setIsRecording] = useState(false);
  const [captured, setCaptured] = useState<string | null>(null);
  const [conflictError, setConflictError] = useState<string | null>(null);
  // 写失败统一走页面级错误条（后端原文）；本行的提示只用于键位校验/冲突
  const { reportSaveError, clearSaveError } = useSettingsSaveError();
  const captureRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    setIsRecording(false);
    setCaptured(null);
    setConflictError(null);
  }, [currentValue]);

  function enterRecording() {
    setIsRecording(true);
    setCaptured(null);
    setConflictError(null);
    setTimeout(() => captureRef.current?.focus(), 0);
  }

  function cancelRecording() {
    setIsRecording(false);
    setCaptured(null);
    setConflictError(null);
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
      clearSaveError(`hotkey.${action}`);
      setIsRecording(false);
      setCaptured(null);
      onSaved();
    } catch (err: unknown) {
      // 键位冲突属于校验（留在本行提示），写失败统一走页面级错误条
      reportSaveError(`hotkey.${action}`, err);
    }
  }

  const errorMessage = conflictError;

  if (isRecording) {
    return (
      <div className="set-row column" title={tooltip}>
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
    <div className="set-row column" title={tooltip}>
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

/** 热键子项面板：加载热键配置，渲染所有可配置热键行 */
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
      // 读取本身失败才算「加载失败」；配置文件损坏不算——那种情况后端会连键位一起返回，
      // 并在 corrupt 里给修复指引，面板照常渲染键位行（不能整块失效）。
      setLoadError("热键加载失败，请重启应用后再试");
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
      {hotkeys.corrupt != null && (
        <div className="hotkey-corrupt" role="alert">
          <span className="hotkey-corrupt-title">热键配置文件已损坏</span>
          <span className="hotkey-corrupt-hint">{hotkeys.corrupt.hint}</span>
        </div>
      )}
      <SettingGroup>
        {HOTKEY_ROWS.map((row) => (
          <HotkeyRow
            key={row.action}
            action={row.action}
            label={row.label}
            tooltip={hotkeyTooltip(row)}
            description={row.description}
            currentValue={hotkeys[row.action]}
            occupiedValues={occupiedValuesFor(row.action, hotkeys)}
            onSaved={handleSaved}
          />
        ))}
      </SettingGroup>
    </div>
  );
}

export default HotkeyPanel;
