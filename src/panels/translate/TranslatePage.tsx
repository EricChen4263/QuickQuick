/**
 * 翻译页（V4-F2-S08，里程碑2批次C）
 *
 * 职责：协调翻译 IPC 调用、历史取数、provider 选择、操作分发、错误处理。
 * 根布局用 .view-translate grid（1fr 工作区 + 280px 历史栏），由 translate.css 定义。
 */

import "./translate.css";
import { useEffect, useState, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  TRANSLATE_HISTORY_CHANGED_EVENT,
  PROVIDER_CONFIG_CHANGED_EVENT,
  SELECTED_PROVIDER_CHANGED_EVENT,
} from "../../ipc/events";
import {
  translateText,
  listTranslateHistory,
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
  getProviderCredentialSchema,
  getProviderCredentials,
  type TranslateResult,
  type TranslateHistoryItem,
  type Provider,
} from "../../ipc/ipc-client";
import { isProviderConfigured } from "../../ipc/credential-utils";
import { resolveTranslateAction } from "../../translate/translate-actions";
import { writeToClipboard, speakText } from "./browser-api";
import TranslateWorkspace from "./TranslateWorkspace";
import TranslateHistoryPanel from "./TranslateHistoryPanel";

/**
 * loading 态最小可见时长（毫秒）。
 * 防止翻译源秒回（如 Google 免费接口 / MyMemory 命中缓存）时，
 * setIsLoading(true)→翻译完成→setIsLoading(false) 在一两帧内走完，
 * React 提交了 loading 帧但浏览器还没绘制就被译文覆盖，导致用户看不到「翻译中」反馈。
 * 同时让翻译按钮在此时长内保持 disabled，天然抑制「频繁点击」重入。
 */
const MIN_LOADING_VISIBLE_MS = 400;

interface TranslatePageProps {
  seed?: { text: string; nonce: number } | null;
}

/** 翻译页根组件：工作区主体 + 翻译历史右栏 */
function TranslatePage({ seed }: TranslatePageProps) {
  const [inputText, setInputText] = useState("");
  const [result, setResult] = useState<TranslateResult | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [history, setHistory] = useState<TranslateHistoryItem[]>([]);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState("");
  const [sourceLang, setSourceLang] = useState("auto");
  const [targetLang, setTargetLang] = useState("zh");
  const [configuredIds, setConfiguredIds] = useState<Set<string>>(new Set());

  /**
   * 取翻译历史，带 cancelled flag 防卸载后写 state。
   * 挂载时 + 每次翻译成功后调用。
   */
  const fetchHistory = useCallback(async (cancelled: { current: boolean }) => {
    try {
      const items = await listTranslateHistory();
      if (cancelled.current) return;
      setHistory(items);
    } catch (err) {
      // 历史取数失败不阻断主翻译工作流，但记录日志便于排查
      console.error("[QuickQuick] 翻译历史取数失败:", err);
    }
  }, []);

  /**
   * 对每个 needsKey provider 并行取 schema + credentials，计算 configuredIds。
   * providerList 由调用方传入，避免重复 fetch。
   */
  const fetchConfiguredIds = useCallback(
    async (providerList: Provider[], cancelled: { current: boolean }) => {
      const needsKeyProviders = providerList.filter((p) => p.needsKey);
      if (needsKeyProviders.length === 0) return;

      const results = await Promise.all(
        needsKeyProviders.map(async (p) => {
          try {
            const [schema, credentials] = await Promise.all([
              getProviderCredentialSchema(p.id),
              getProviderCredentials(p.id),
            ]);
            return { id: p.id, configured: isProviderConfigured(schema, credentials) };
          } catch {
            return { id: p.id, configured: false };
          }
        })
      );

      if (cancelled.current) return;
      setConfiguredIds(new Set(results.filter((r) => r.configured).map((r) => r.id)));
    },
    []
  );

  /**
   * 挂载时并发 fetch：历史列表 + provider 列表 + 当前选中 provider + 凭据配置状态。
   * 用同一 cancelled ref 统一防卸载后写 state。
   */
  useEffect(() => {
    const cancelled = { current: false };

    fetchHistory(cancelled);

    Promise.all([getTranslateProviders(), getSelectedProvider()])
      .then(([providerList, currentId]) => {
        if (cancelled.current) return;
        setProviders(providerList);
        setSelectedProviderId(currentId);
        void fetchConfiguredIds(providerList, cancelled);
      })
      .catch((err) => {
        // provider 取数失败降级为空列表，不阻断翻译工作流
        console.error("[QuickQuick] provider 取数失败:", err);
      });

    return () => {
      cancelled.current = true;
    };
  }, [fetchHistory, fetchConfiguredIds]);

  // 订阅后端 translate-history-changed 事件，快捷翻译（trans-popover）写库后通知历史栏刷新。
  // 采用与 ClipboardPage 订阅 clipboard-changed 相同的 cancelled+unlisten 范式，防卸载后泄漏。
  useEffect(() => {
    const cancelled = { current: false };
    let unlisten: (() => void) | undefined;
    listen(TRANSLATE_HISTORY_CHANGED_EVENT, () => {
      void fetchHistory(cancelled);
    })
      .then((fn) => {
        if (cancelled.current) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err: unknown) => {
        console.error("[QuickQuick] translate-history-changed 监听注册失败:", err);
      });
    return () => {
      cancelled.current = true;
      unlisten?.();
    };
  }, [fetchHistory]);

  // 订阅后端 provider-config-changed 事件，设置页保存凭据后通知翻译页刷新 configuredIds。
  // 采用相同的 cancelled+unlisten 范式，防卸载后泄漏。
  useEffect(() => {
    const cancelled = { current: false };
    let unlisten: (() => void) | undefined;
    listen(PROVIDER_CONFIG_CHANGED_EVENT, () => {
      setProviders((currentProviders) => {
        void fetchConfiguredIds(currentProviders, cancelled);
        return currentProviders;
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
        console.error("[QuickQuick] provider-config-changed 监听注册失败:", err);
      });
    return () => {
      cancelled.current = true;
      unlisten?.();
    };
  }, [fetchConfiguredIds]);

  // 订阅后端 selected-provider-changed 事件：设置页改默认翻译源后，翻译页据此刷新选中项。
  // 采用相同的 cancelled+unlisten 范式，防卸载后泄漏；自发自收幂等（值相同），无需去抖。
  useEffect(() => {
    const cancelled = { current: false };
    let unlisten: (() => void) | undefined;
    listen(SELECTED_PROVIDER_CHANGED_EVENT, () => {
      void getSelectedProvider()
        .then((currentId) => {
          if (cancelled.current) return;
          setSelectedProviderId(currentId);
        })
        .catch((err: unknown) => {
          console.error("[QuickQuick] selected-provider-changed 刷新失败:", err);
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
        console.error("[QuickQuick] selected-provider-changed 监听注册失败:", err);
      });
    return () => {
      cancelled.current = true;
      unlisten?.();
    };
  }, []);

  /**
   * 执行翻译：调 IPC → 更新结果 → 刷新历史。
   * textOverride 为显式字符串时用之，否则读 inputText state。
   * typeof 守卫防止 TranslateWorkspace 翻译按钮把合成事件对象泄漏为参数。
   * source=auto 时传 undefined，后端回退自动检测。
   */
  const handleTranslate = useCallback(async (textOverride?: string) => {
    const text = typeof textOverride === "string" ? textOverride : inputText;
    if (text.trim().length === 0) return;
    setIsLoading(true);
    setError(null);
    const startedAt = Date.now();
    try {
      const sourceParam = sourceLang === "auto" ? undefined : sourceLang;
      const res = await translateText(text, targetLang, sourceParam);
      setResult(res);
      const cancelled = { current: false };
      await fetchHistory(cancelled);
    } catch (err) {
      setError(err instanceof Error ? err.message : "翻译失败，请稍后重试");
      setResult(null);
    } finally {
      // 秒回时补足最小可见时长，确保 loading 帧能被浏览器绘制后再切到结果（见 MIN_LOADING_VISIBLE_MS）。
      const remainingMs = MIN_LOADING_VISIBLE_MS - (Date.now() - startedAt);
      if (remainingMs > 0) {
        await new Promise((resolve) => setTimeout(resolve, remainingMs));
      }
      setIsLoading(false);
    }
  }, [inputText, sourceLang, targetLang, fetchHistory]);

  // 监听 seed.nonce 变化：每次新 seed 到来自动填入文本并触发翻译。
  // 依赖数组只放 seed?.nonce，确保同文本重复点击也能重新触发。
  const seedRef = useRef(seed);
  seedRef.current = seed;
  useEffect(() => {
    const current = seedRef.current;
    if (current && current.text.trim().length > 0) {
      setInputText(current.text);
      void handleTranslate(current.text);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [seed?.nonce]);

  /**
   * 分发译文操作：
   * - copy → clipboard.writeText(译文)
   * - speak → speechSynthesis.speak(SpeechSynthesisUtterance)
   * - save_history → 已自动写入，刷新历史列表即可
   */
  async function handleAction(cmd: string) {
    const action = resolveTranslateAction(cmd);
    if (action === null || result === null) return;

    try {
      if (action === "copy") {
        await writeToClipboard(result.translated);
        setError(null);
        return;
      }
      if (action === "speak") {
        speakText(result.translated);
        setError(null);
        return;
      }
      if (action === "save_history") {
        // 后端 translate_text 已自动写入历史，此处只刷新列表
        const cancelled = { current: false };
        await fetchHistory(cancelled);
      }
    } catch {
      setError("操作失败，请稍后重试");
    }
  }

  /** 切换 provider：调 IPC 持久化，并同步本地 state */
  async function handleProviderChange(id: string) {
    try {
      await setSelectedProvider(id);
      setSelectedProviderId(id);
    } catch (err) {
      // provider 切换失败不崩溃，记录日志
      console.error("[QuickQuick] 切换 provider 失败:", err);
    }
  }

  /** 点击历史条目 → 回填工作区（input + 结果区），不再发起新翻译 */
  function handleSelectHistoryItem(item: TranslateHistoryItem) {
    setInputText(item.sourceText);
    setResult({
      translated: item.translatedText,
      sourceLang: item.sourceLang,
      targetLang: item.targetLang,
    });
    setError(null);
  }

  return (
    <div className="view-translate">
      <TranslateWorkspace
        inputText={inputText}
        result={result}
        isLoading={isLoading}
        error={error}
        sourceLang={sourceLang}
        targetLang={targetLang}
        providers={providers}
        selectedProviderId={selectedProviderId}
        configuredIds={configuredIds}
        onInputChange={setInputText}
        onTranslate={handleTranslate}
        onSourceChange={setSourceLang}
        onTargetChange={setTargetLang}
        onAction={handleAction}
        onProviderChange={handleProviderChange}
      />
      <TranslateHistoryPanel
        items={history}
        onSelectItem={handleSelectHistoryItem}
      />
    </div>
  );
}

export default TranslatePage;
