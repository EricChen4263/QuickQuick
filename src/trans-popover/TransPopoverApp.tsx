import React, { useEffect, useRef, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listClipItems, translateText } from "../ipc/ipc-client";
import type { TranslateResult } from "../ipc/ipc-client";
import { writeToClipboard, speakText } from "../panels/translate/browser-api";
import { pickLatestText } from "./source-text";
import { shouldRetranslate } from "./retranslate";
import MiniTranslate from "./MiniTranslate";

type Status = "loading" | "empty" | "translating" | "done" | "error";

/**
 * 翻译 popover 主体。
 *
 * 取词方案：挂载时自读 listClipItems()[0]，避免 Rust emit 事件在前端
 * 监听就绪前丢失的竞态（详见 Batch C1 设计决策）。
 * 获焦重读：窗口每次 focus 时重读剪贴板，用 shouldRetranslate 去重（Batch C2）。
 */
function TransPopoverApp(): React.ReactElement {
  const [status, setStatus] = useState<Status>("loading");
  const [result, setResult] = useState<TranslateResult | null>(null);
  const [errorMsg, setErrorMsg] = useState<string>("");
  const lastTextRef = useRef<string | null>(null);
  const translatingRef = useRef(false);

  async function runTranslateFromClipboard(cancelledRef: { value: boolean }): Promise<void> {
    if (translatingRef.current) return;
    translatingRef.current = true;

    try {
      const items = await listClipItems();
      const text = pickLatestText(items);

      if (cancelledRef.value) return;

      if (!shouldRetranslate(text, lastTextRef.current)) {
        if (text === null && lastTextRef.current === null) {
          setStatus("empty");
        }
        return;
      }

      if (text === null) {
        setStatus("empty");
        return;
      }

      setStatus("translating");
      const translated = await translateText(text);

      if (cancelledRef.value) return;
      lastTextRef.current = text;
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

    void runTranslateFromClipboard(cancelledRef);

    let unlisten: (() => void) | undefined;

    getCurrentWindow()
      .listen("tauri://focus", () => {
        if (cancelledRef.value) return;
        void runTranslateFromClipboard(cancelledRef);
      })
      .then((fn) => {
        if (!cancelledRef.value) {
          unlisten = fn;
        } else {
          fn();
        }
      })
      .catch((err: unknown) => {
        console.warn("[trans-popover] focus listen failed:", err);
      });

    return () => {
      cancelledRef.value = true;
      unlisten?.();
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
