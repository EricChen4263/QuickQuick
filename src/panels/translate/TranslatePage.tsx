/**
 * 翻译页（V4-F2-S08，里程碑2批次C）
 *
 * 职责：协调翻译 IPC 调用、历史取数、provider 选择、操作分发、错误处理。
 * 根布局用 .view-translate grid（1fr 工作区 + 280px 历史栏），由 translate.css 定义。
 */

import "./translate.css";
import { useEffect, useState, useCallback, useRef } from "react";
import {
  translateText,
  listTranslateHistory,
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
  type TranslateResult,
  type TranslateHistoryItem,
  type Provider,
} from "../../ipc/ipc-client";
import { resolveTranslateAction } from "../../translate/translate-actions";
import { writeToClipboard, speakText } from "./browser-api";
import TranslateWorkspace from "./TranslateWorkspace";
import TranslateHistoryPanel from "./TranslateHistoryPanel";

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
   * 挂载时并发 fetch：历史列表 + provider 列表 + 当前选中 provider。
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
      })
      .catch((err) => {
        // provider 取数失败降级为空列表，不阻断翻译工作流
        console.error("[QuickQuick] provider 取数失败:", err);
      });

    return () => {
      cancelled.current = true;
    };
  }, [fetchHistory]);

  /**
   * 执行翻译：调 IPC → 更新结果 → 刷新历史。
   * textOverride 为显式字符串时用之，否则读 inputText state。
   * typeof 守卫防止 TranslateWorkspace 翻译按钮把合成事件对象泄漏为参数。
   */
  const handleTranslate = useCallback(async (textOverride?: string) => {
    const text = typeof textOverride === "string" ? textOverride : inputText;
    if (text.trim().length === 0) return;
    setIsLoading(true);
    setError(null);
    try {
      const res = await translateText(text, undefined);
      setResult(res);
      const cancelled = { current: false };
      await fetchHistory(cancelled);
    } catch {
      setError("翻译失败，请稍后重试");
      setResult(null);
    } finally {
      setIsLoading(false);
    }
  }, [inputText, fetchHistory]);

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
   * - switch_target / switch_source_retranslate → 重新翻译（简化：直接重发当前文本）
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
      if (action === "switch_target" || action === "switch_source_retranslate") {
        await handleTranslate();
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

  /**
   * Swap：把当前译文填回输入框，以原 sourceLang 为目标语重新翻译。
   * 仅在存在明确翻译结果（非 auto sourceLang）时生效，否则无操作。
   */
  async function handleSwap() {
    if (result === null || result.sourceLang === "" || result.sourceLang === "auto") return;
    const textToTranslate = result.translated;
    const targetLang = result.sourceLang;
    setInputText(textToTranslate);
    setIsLoading(true);
    setError(null);
    try {
      const res = await translateText(textToTranslate, targetLang);
      setResult(res);
      const cancelled = { current: false };
      await fetchHistory(cancelled);
    } catch {
      setError("翻译失败，请稍后重试");
      setResult(null);
    } finally {
      setIsLoading(false);
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
        providers={providers}
        selectedProviderId={selectedProviderId}
        onInputChange={setInputText}
        onTranslate={handleTranslate}
        onSwap={handleSwap}
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
