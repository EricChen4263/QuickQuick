import { useEffect, useState, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
  getProviderCredentialSchema,
  getProviderCredentials,
  type Provider,
  type CredentialField,
  type CredentialValue,
} from "../../ipc/ipc-client";
import { SELECTED_PROVIDER_CHANGED_EVENT } from "../../ipc/events";
import { isProviderConfigured } from "../../ipc/credential-utils";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";
import CredentialForm from "./CredentialForm";

/** provider id 取首两字作 logo 缩写，便于用等宽字体展示 */
function logoAbbr(name: string): string {
  return name.slice(0, 2).toUpperCase();
}

const GearIcon = () => (
  <svg
    width="15"
    height="15"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
  </svg>
);

interface ProviderCardProps {
  provider: Provider;
  isSelected: boolean;
  isConfigured: boolean;
  isExpanded: boolean;
  onSelect: (id: string) => void;
  onConfigure: (id: string) => void;
}

/** 单个翻译源卡片行：左侧「设默认」热区 + 右侧可选「配置」按钮（变体 A） */
function ProviderCard({
  provider,
  isSelected,
  isConfigured,
  isExpanded,
  onSelect,
  onConfigure,
}: ProviderCardProps) {
  // 判据为「是否可配置」（needsConfig）而非「是否要 key」：Ollama 无 key 但有必填字段也需配置。
  const badgeClass = isSelected
    ? "badge default"
    : provider.needsConfig
      ? isConfigured ? "badge ok" : "badge need"
      : "badge ok";

  const badgeLabel = isSelected
    ? "默认"
    : provider.needsConfig
      ? isConfigured ? "已配置" : "待配置"
      : "无需配置";

  return (
    <div className="src-card">
      <div
        className="src-select"
        onClick={() => onSelect(provider.id)}
      >
        <div className={`src-radio${isSelected ? " sel" : ""}`} />
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
      {provider.needsConfig && (
        <button
          type="button"
          className={`src-cfg-btn${isExpanded ? " open" : ""}`}
          onClick={() => onConfigure(provider.id)}
          aria-label={`配置 ${provider.name}`}
        >
          <GearIcon />
          配置
        </button>
      )}
    </div>
  );
}

/** 翻译源子项面板：列出 providers，单选切换；配置按钮独立控制 key 表单展开 */
function TranslateSourcePanel() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [selectedId, setSelectedId] = useState<string>("");
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [schemaCache, setSchemaCache] = useState<Record<string, CredentialField[]>>({});
  const [configuredIds, setConfiguredIds] = useState<Set<string>>(new Set());
  const [loadError, setLoadError] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);
  const isMounted = useRef(true);

  useEffect(() => {
    isMounted.current = true;
    return () => {
      isMounted.current = false;
    };
  }, []);

  const refreshConfiguredState = useCallback(
    async (
      providerList: Provider[],
      schemas: Record<string, CredentialField[]>,
      cancelled: { current: boolean }
    ) => {
      const configurableProviders = providerList.filter((p) => p.needsConfig);

      const results = await Promise.all(
        configurableProviders.map(async (p) => {
          try {
            const credentials: CredentialValue[] = await getProviderCredentials(p.id);
            const schema = schemas[p.id] ?? [];
            return { id: p.id, configured: isProviderConfigured(schema, credentials) };
          } catch {
            return { id: p.id, configured: false };
          }
        })
      );

      if (cancelled.current) return;

      setConfiguredIds(
        new Set(results.filter((r) => r.configured).map((r) => r.id))
      );
    },
    []
  );

  const fetchProviders = useCallback(async (cancelled: { current: boolean }) => {
    try {
      const [providerList, currentId] = await Promise.all([
        getTranslateProviders(),
        getSelectedProvider(),
      ]);
      if (cancelled.current) return;

      const configurableProviders = providerList.filter((p) => p.needsConfig);
      const schemaEntries = await Promise.all(
        configurableProviders.map(async (p) => {
          try {
            const schema = await getProviderCredentialSchema(p.id);
            return [p.id, schema] as [string, CredentialField[]];
          } catch {
            return [p.id, []] as [string, CredentialField[]];
          }
        })
      );

      if (cancelled.current) return;

      const schemas = Object.fromEntries(schemaEntries);

      setProviders(providerList);
      setSelectedId(currentId);
      setSchemaCache(schemas);
      setLoadError(null);

      await refreshConfiguredState(providerList, schemas, cancelled);
    } catch {
      if (cancelled.current) return;
      setLoadError("翻译源加载失败，请稍后重试");
    }
  }, [refreshConfiguredState]);

  useEffect(() => {
    const cancelled = { current: false };
    fetchProviders(cancelled);
    return () => {
      cancelled.current = true;
    };
  }, [fetchProviders]);

  // 订阅后端 selected-provider-changed 事件：翻译页改默认源后，设置页据此刷新选中徽标。
  // 采用相同的 cancelled+unlisten 范式，防卸载后泄漏；自发自收幂等（值相同），无需去抖。
  useEffect(() => {
    const cancelled = { current: false };
    let unlisten: (() => void) | undefined;
    listen(SELECTED_PROVIDER_CHANGED_EVENT, () => {
      void getSelectedProvider()
        .then((currentId) => {
          if (cancelled.current) return;
          setSelectedId(currentId);
        })
        .catch((err: unknown) => {
          console.error("[TranslateSourcePanel] selected-provider-changed 刷新失败:", err);
        });
    })
      .then((fn) => {
        if (cancelled.current) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err: unknown) => {
        console.error("[TranslateSourcePanel] selected-provider-changed 监听注册失败:", err);
      });
    return () => {
      cancelled.current = true;
      unlisten?.();
    };
  }, []);

  async function handleSelect(id: string) {
    try {
      await setSelectedProvider(id);
      setSelectedId(id);
      setOpError(null);
    } catch {
      setOpError("切换翻译源失败，请稍后重试");
    }
  }

  function handleToggleConfigure(id: string) {
    setExpandedId((prev) => (prev === id ? null : id));
  }

  function handleCredentialSaved(providerId: string) {
    void getProviderCredentials(providerId)
      .then((credentials) => {
        if (!isMounted.current) return;
        const schema = schemaCache[providerId] ?? [];
        const configured = isProviderConfigured(schema, credentials);
        setConfiguredIds((prev) => {
          const next = new Set(prev);
          if (configured) {
            next.add(providerId);
          } else {
            next.delete(providerId);
          }
          return next;
        });
        if (configured) {
          setExpandedId(null);
        }
      })
      .catch((err) => {
        // 复核请求失败时无法确认配置态，保守回退为「待配置」（移除该 id），
        // 避免清除/保存后徽标与实际状态矛盾。
        if (!isMounted.current) return;
        console.error("[TranslateSourcePanel] 复核凭据状态失败，回退为待配置:", err);
        setConfiguredIds((prev) => {
          const next = new Set(prev);
          next.delete(providerId);
          return next;
        });
      });
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
          <div key={provider.id}>
            <ProviderCard
              provider={provider}
              isSelected={selectedId === provider.id}
              isConfigured={configuredIds.has(provider.id)}
              isExpanded={expandedId === provider.id}
              onSelect={(id) => void handleSelect(id)}
              onConfigure={handleToggleConfigure}
            />
            {expandedId === provider.id && (
              <CredentialForm
                providerId={provider.id}
                schema={schemaCache[provider.id] ?? []}
                onSaved={() => handleCredentialSaved(provider.id)}
                isConfigured={configuredIds.has(provider.id)}
              />
            )}
          </div>
        ))}
      </SettingGroup>
      {opError !== null && (
        <div role="alert" style={{ color: "var(--danger)" }}>{opError}</div>
      )}
    </div>
  );
}

export default TranslateSourcePanel;
