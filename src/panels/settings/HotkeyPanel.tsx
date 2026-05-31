import { useEffect, useState, useCallback } from "react";
import { getHotkeys, setHotkey, type Hotkeys, type HotkeyAction } from "../../ipc/ipc-client";
import { validateRebind } from "../../main-window/settings/rebind";

interface HotkeyRowProps {
  action: HotkeyAction;
  label: string;
  currentValue: string;
  occupiedValues: string[];
  onSaved: () => void;
}

/** 单行热键编辑组件：输入 + 实时冲突校验 + 保存 */
function HotkeyRow({ action, label, currentValue, occupiedValues, onSaved }: HotkeyRowProps) {
  const [inputValue, setInputValue] = useState(currentValue);
  const [conflictError, setConflictError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  // 当父组件刷新 currentValue 时同步输入框
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

  return (
    <div style={{ marginBottom: 16 }}>
      <label style={{ display: "block", marginBottom: 4, fontFamily: "var(--qq-font)" }}>
        {label}
      </label>
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <input
          type="text"
          value={inputValue}
          onChange={(e) => {
            setInputValue(e.target.value);
            setConflictError(null);
            setSaveError(null);
          }}
          style={{ fontFamily: "var(--qq-font)", padding: "4px 8px" }}
        />
        <button onClick={() => void handleSave()}>保存</button>
      </div>
      {errorMessage !== null && (
        <div
          role="alert"
          style={{ color: "var(--qq-danger, #c0392b)", marginTop: 4, fontSize: 13 }}
        >
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
    <div style={{ padding: 24 }}>
      <h2 style={{ fontFamily: "var(--qq-font)", marginTop: 0 }}>热键</h2>
      <HotkeyRow
        action="history"
        label="剪贴板历史"
        currentValue={hotkeys.history}
        occupiedValues={[hotkeys.translate]}
        onSaved={handleSaved}
      />
      <HotkeyRow
        action="translate"
        label="翻译"
        currentValue={hotkeys.translate}
        occupiedValues={[hotkeys.history]}
        onSaved={handleSaved}
      />
    </div>
  );
}

export default HotkeyPanel;
