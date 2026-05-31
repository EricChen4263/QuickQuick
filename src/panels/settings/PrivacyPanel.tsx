import { useEffect, useState, useCallback } from "react";
import { getExcludeList, setExcludeList } from "../../ipc/ipc-client";
import { addExcludedApp, removeExcludedApp } from "../../main-window/settings/sections";

/** 隐私子项面板：管理 App 排除名单 */
function PrivacyPanel() {
  const [excludeList, setExcludeListState] = useState<string[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);

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
      <div style={{ padding: 24 }}>
        <div role="alert" style={{ color: "var(--qq-danger, #c0392b)" }}>{loadError}</div>
      </div>
    );
  }

  return (
    <div style={{ padding: 24 }}>
      <h2 style={{ fontFamily: "var(--qq-font)", marginTop: 0 }}>隐私</h2>
      <p style={{ fontFamily: "var(--qq-font)", color: "var(--qq-text-muted)" }}>
        以下应用处于前台时，剪贴板内容不会被记录。
      </p>

      {opError !== null && (
        <div role="alert" style={{ color: "var(--qq-danger, #c0392b)", marginBottom: 12 }}>
          {opError}
        </div>
      )}

      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        <input
          type="text"
          value={inputValue}
          placeholder="应用名称"
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") void handleAdd(); }}
          style={{ fontFamily: "var(--qq-font)", padding: "4px 8px", flex: 1 }}
        />
        <button onClick={() => void handleAdd()}>添加</button>
      </div>

      <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
        {excludeList.map((app) => (
          <li
            key={app}
            style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "6px 0", fontFamily: "var(--qq-font)" }}
          >
            <span>{app}</span>
            <button
              aria-label={`删除 ${app}`}
              onClick={() => void handleRemove(app)}
              style={{ background: "none", border: "none", cursor: "pointer", color: "var(--qq-danger, #c0392b)" }}
            >
              删除
            </button>
          </li>
        ))}
      </ul>

      {excludeList.length === 0 && (
        <p style={{ fontFamily: "var(--qq-font)", color: "var(--qq-text-muted)" }}>
          暂无排除应用
        </p>
      )}
    </div>
  );
}

export default PrivacyPanel;
