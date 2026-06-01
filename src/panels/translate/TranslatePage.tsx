/**
 * 翻译页（V4-F2-S08，里程碑2批次C）
 *
 * 职责：协调翻译 IPC 调用、历史取数、provider 选择、操作分发、错误处理。
 * 根布局用 .view-translate grid（1fr 工作区 + 280px 历史栏），由 translate.css 定义。
 */

import "./translate.css";
import { useEffect, useState, useCallback } from "react";
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

/** 翻译页根组件：工作区主体 + 翻译历史右栏 */
function TranslatePage() {
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

  /** 执行翻译：调 IPC → 更新结果 → 刷新历史 */
  async function handleTranslate() {
    if (inputText.trim().length === 0) return;
    setIsLoading(true);
    setError(null);
    try {
      const res = await translateText(inputText, undefined);
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
