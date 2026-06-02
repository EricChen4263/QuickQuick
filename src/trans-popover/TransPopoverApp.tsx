import React, { useEffect, useState } from "react";
import { listClipItems, translateText } from "../ipc/ipc-client";
import type { TranslateResult } from "../ipc/ipc-client";
import { writeToClipboard, speakText } from "../panels/translate/browser-api";
import { pickLatestText } from "./source-text";
import MiniTranslate from "./MiniTranslate";

type Status = "loading" | "empty" | "translating" | "done" | "error";

/**
 * 翻译 popover 主体。
 *
 * 取词方案：挂载时自读 listClipItems()[0]，避免 Rust emit 事件在前端
 * 监听就绪前丢失的竞态（详见 Batch C1 设计决策）。
 * 获焦重读见 Batch C2。
 */
function TransPopoverApp(): React.ReactElement {
  const [status, setStatus] = useState<Status>("loading");
  const [result, setResult] = useState<TranslateResult | null>(null);
  const [errorMsg, setErrorMsg] = useState<string>("");

  useEffect(() => {
    let cancelled = false;

    async function run(): Promise<void> {
      try {
        const items = await listClipItems();
        const text = pickLatestText(items);

        if (cancelled) return;

        if (text === null) {
          setStatus("empty");
          return;
        }

        setStatus("translating");
        const translated = await translateText(text);

        if (cancelled) return;
        setResult(translated);
        setStatus("done");
      } catch {
        if (cancelled) return;
        setErrorMsg("翻译失败，请稍后重试");
        setStatus("error");
      }
    }

    void run();

    return () => {
      cancelled = true;
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
      onExpand={() => {
        /* 展开跳转 main 见 Batch C2 */
      }}
    />
  );
}

export default TransPopoverApp;
