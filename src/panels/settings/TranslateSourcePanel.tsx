import { useEffect, useState, useCallback } from "react";
import {
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
  type Provider,
} from "../../ipc/ipc-client";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";

/** provider id 取首两字作 logo 缩写，便于用等宽字体展示 */
function logoAbbr(name: string): string {
  return name.slice(0, 2).toUpperCase();
}

interface ProviderCardProps {
  provider: Provider;
  isSelected: boolean;
  onSelect: (id: string) => void;
}

/** 单个翻译源卡片行：logo 缩写 + 名称 + 状态标签 + 视觉隐藏 radio */
function ProviderCard({ provider, isSelected, onSelect }: ProviderCardProps) {
  const badgeClass = isSelected
    ? "badge default"
    : provider.needsKey
      ? "badge need"
      : "badge ok";

  const badgeLabel = isSelected
    ? "默认"
    : provider.needsKey
      ? "待配置"
      : "无需 Key";

  return (
    <div
      className="src-card"
      onClick={() => onSelect(provider.id)}
    >
      <div className="src-logo">{logoAbbr(provider.name)}</div>
      <div className="grow">
        <div className="label">{provider.name}</div>
      </div>
      <span className={badgeClass}>{badgeLabel}</span>
      {/* radio 视觉隐藏，保留可访问性——让 getByRole("radio", { name }) 能命中 */}
      <input
        type="radio"
        name="provider"
        className="sr-only"
        checked={isSelected}
        onChange={() => onSelect(provider.id)}
        aria-label={provider.name}
      />
    </div>
  );
}

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
      <div>
        <div role="alert" style={{ color: "var(--danger)" }}>{loadError}</div>
      </div>
    );
  }

  return (
    <div>
      <PanelHeader title="翻译源" subtitle="选择用于划词翻译的服务商" />
      <SettingGroup>
        {providers.map((provider) => (
          <ProviderCard
            key={provider.id}
            provider={provider}
            isSelected={selectedId === provider.id}
            onSelect={(id) => void handleSelect(id)}
          />
        ))}
      </SettingGroup>
      {opError !== null && (
        <div role="alert" style={{ color: "var(--danger)" }}>{opError}</div>
      )}
    </div>
  );
}

export default TranslateSourcePanel;
