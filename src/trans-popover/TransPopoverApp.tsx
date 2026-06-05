import React, { useEffect, useRef, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listClipItems, translateText } from "../ipc/ipc-client";
import type { TranslateResult } from "../ipc/ipc-client";
import { writeToClipboard, speakText } from "../panels/translate/browser-api";
import { pickLatestText } from "./source-text";
import { TRANS_SOURCE_EVENT } from "../ipc/events";
import MiniTranslate from "./MiniTranslate";

type Status = "loading" | "empty" | "translating" | "done" | "error";

/**
 * 收纳一个 listen 的解绑函数：若组件已卸载（cancelled）则立即解绑，
 * 否则存入 unlisteners 待 cleanup 统一调用，避免监听泄漏。
 */
function collectUnlisten(
  fn: () => void,
  cancelledRef: { value: boolean },
  unlisteners: Array<() => void>,
): void {
  if (cancelledRef.value) {
    fn();
    return;
  }
  unlisteners.push(fn);
}

/**
 * 翻译 popover 主体。
 *
 * 取词方案：
 * - 冷启动：挂载时自读 listClipItems()[0]，避免 Rust emit 事件在前端监听
 *   就绪前丢失的竞态（沿用 Batch C1 决策）。
 * - 暖触发：热键再次唤起时由 Rust 在 window.show() 后推送 trans-source 事件
 *   携带待译文本，前端无需再 listClipItems() 往返查加密 DB（消除可见延迟 Bug 1）。
 * - blur 复位：窗口隐藏时把视图复位为中性「翻译中…」，确保下次 show 画的是干净
 *   帧而非上次旧译文残影（Bug 2）；缓存的 lastText/lastResult 保留供秒级还原。
 */
function TransPopoverApp(): React.ReactElement {
  const [status, setStatus] = useState<Status>("loading");
  const [result, setResult] = useState<TranslateResult | null>(null);
  const [errorMsg, setErrorMsg] = useState<string>("");
  const lastTextRef = useRef<string | null>(null);
  const lastResultRef = useRef<TranslateResult | null>(null);
  const translatingRef = useRef(false);

  /**
   * 处理一次「待译文本」输入：null=无可译内容；与上次同文本且有缓存则秒级还原；
   * 新文本则先清旧结果再联网翻译。翻译中再来事件直接忽略（互斥防重入）。
   *
   * 失败后不更新 lastText/lastResult，保证同文本可重试（保住 M-1 回归意图）。
   */
  async function handleSource(
    text: string | null,
    cancelledRef: { value: boolean },
  ): Promise<void> {
    if (translatingRef.current) return;

    if (text === null) {
      setResult(null);
      setStatus("empty");
      return;
    }

    // 同剪贴板不重复联网：直接还原上次成功译文
    if (text === lastTextRef.current && lastResultRef.current !== null) {
      setResult(lastResultRef.current);
      setStatus("done");
      return;
    }

    translatingRef.current = true;
    setResult(null);
    setStatus("translating");

    try {
      const translated = await translateText(text);
      if (cancelledRef.value) return;
      lastTextRef.current = text;
      lastResultRef.current = translated;
      setResult(translated);
      setStatus("done");
    } catch {
      if (cancelledRef.value) return;
      setErrorMsg("翻译失败，请稍后重试");
      setStatus("error");
    } finally {
      translatingRef.current = false;
    }
  }

  useEffect(() => {
    const cancelledRef = { value: false };
    const unlisteners: Array<() => void> = [];

    // 冷启动自读（沿用 C1：避免推送事件早于监听就绪丢失）
    void (async () => {
      try {
        const items = await listClipItems();
        if (cancelledRef.value) return;
        await handleSource(pickLatestText(items), cancelledRef);
      } catch {
        if (!cancelledRef.value) {
          setErrorMsg("翻译失败，请稍后重试");
          setStatus("error");
        }
      }
    })();

    const win = getCurrentWindow();

    win
      .listen(TRANS_SOURCE_EVENT, (e) => {
        if (cancelledRef.value) return;
        void handleSource(e.payload as string | null, cancelledRef);
      })
      .then((fn) => collectUnlisten(fn, cancelledRef, unlisteners))
      .catch((err: unknown) => {
        console.warn("[trans-popover] trans-source listen failed:", err);
      });

    // blur 复位：隐藏时清掉旧译文帧，下次 show 画干净「翻译中…」（消残影）
    win
      .listen("tauri://blur", () => {
        if (cancelledRef.value || translatingRef.current) return;
        setResult(null);
        setStatus("loading");
      })
      .then((fn) => collectUnlisten(fn, cancelledRef, unlisteners))
      .catch((err: unknown) => {
        console.warn("[trans-popover] blur listen failed:", err);
      });

    return () => {
      cancelledRef.value = true;
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  function handleCopy(): void {
    if (result === null) return;
    writeToClipboard(result.translated).catch((err: unknown) => {
      console.warn("[trans-popover] copy failed:", err);
    });
  }

  function handleSpeak(): void {
    if (result === null) return;
    speakText(result.translated);
  }

  // v1 仅跳转翻译页，不预填文本（主窗翻译输入预填为后续增强）
  async function handleExpand(): Promise<void> {
    try {
      await emit("route", "translate");
      const main = await WebviewWindow.getByLabel("main");
      await main?.show();
      await main?.setFocus();
      await getCurrentWindow().hide();
    } catch (err: unknown) {
      console.error("[trans-popover] expand failed:", err);
    }
  }

  if (status === "loading" || status === "translating") {
    return (
      <div className="mini-status">
        <span className="mini-hint">翻译中…</span>
      </div>
    );
  }

  if (status === "empty") {
    return (
      <div className="mini-status">
        <span className="mini-hint">请先复制文字再按 ⌘⇧T</span>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="mini-status">
        <span className="mini-hint mini-error">{errorMsg}</span>
      </div>
    );
  }

  return (
    <MiniTranslate
      result={result!}
      onCopy={handleCopy}
      onSpeak={handleSpeak}
      onExpand={() => void handleExpand()}
    />
  );
}

export default TransPopoverApp;
