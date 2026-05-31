import { useEffect, useState, useCallback } from "react";
import {
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
  type Provider,
} from "../../ipc/ipc-client";

/** 翻译源子项面板：列出 providers，单选切换 */
function TranslateSourcePanel() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [selectedId, setSelectedId] = useState<string>("");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);

  const fetchProviders = useCallback(async (cancelled: { current: boolean }) => {
    try {
      const [providerList, currentId] = await Promise.all([
        getTranslateProviders(),
        getSelectedProvider(),
      ]);
      if (cancelled.current) return;
      setProviders(providerList);
      setSelectedId(currentId);
      setLoadError(null);
    } catch {
      if (cancelled.current) return;
      setLoadError("翻译源加载失败，请稍后重试");
    }
  }, []);

  useEffect(() => {
    const cancelled = { current: false };
    fetchProviders(cancelled);
    return () => {
      cancelled.current = true;
    };
  }, [fetchProviders]);

  async function handleSelect(id: string) {
    try {
      await setSelectedProvider(id);
      setSelectedId(id);
      setOpError(null);
    } catch {
      setOpError("切换翻译源失败，请稍后重试");
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
      <h2 style={{ fontFamily: "var(--qq-font)", marginTop: 0 }}>翻译源</h2>
      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {providers.map((provider) => (
          <label
            key={provider.id}
            style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer", fontFamily: "var(--qq-font)" }}
          >
            <input
              type="radio"
              name="translate-provider"
              value={provider.id}
              checked={selectedId === provider.id}
              onChange={() => void handleSelect(provider.id)}
            />
            {provider.name}
          </label>
        ))}
      </div>
      {opError !== null && (
        <div role="alert" style={{ color: "var(--qq-danger, #c0392b)", marginTop: 12 }}>{opError}</div>
      )}
    </div>
  );
}

export default TranslateSourcePanel;
