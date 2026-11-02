import React, { useEffect, useRef, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  getTransPopoverSource,
  listClipItems,
  translateText,
} from "../ipc/ipc-client";
import type { TranslateResult, TransSourcePayload } from "../ipc/ipc-client";
import { writeToClipboard, speakText } from "../panels/translate/browser-api";
import { pickLatestText } from "./source-text";
import { asTransSourcePayload } from "./trans-source-payload";
import { ROUTE_EVENT, TRANS_SOURCE_EVENT, TRANSLATE_SEED_EVENT } from "../ipc/events";
import MiniTranslate, { fallbackHint } from "./MiniTranslate";

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
 * 取词方案（取词全在 Rust 侧、窗口显示之前完成——本进程一旦激活，源应用的选区就没了）：
 * - 冷启动：挂载时用 `getTransPopoverSource()` 补读 Rust 存的来源快照。窗口首次挂载晚于
 *   Rust 的推送事件，自己查剪贴板补位会把「选中」错标成「剪贴板」。
 * - 暖触发：热键再次唤起时由 Rust 在 window.show() 后推送 trans-source 事件，payload 与
 *   快照同源（文本 + 来源 + 降级原因），前端据此直接重译，无需 listClipItems 往返。
 * - 兜底：快照为空（非热键路径直接打开本窗口）时才回退 listClipItems，并如实标「剪贴板」。
 * - blur 复位：窗口隐藏时把视图复位为中性「翻译中…」，确保下次 show 画的是干净
 *   帧而非上次旧译文残影（Bug 2）；缓存的 lastText/lastResult 保留供秒级还原。
 */
function TransPopoverApp(): React.ReactElement {
  const [status, setStatus] = useState<Status>("loading");
  const [result, setResult] = useState<TranslateResult | null>(null);
  const [sourceInfo, setSourceInfo] = useState<TransSourcePayload | null>(null);
  const [errorMsg, setErrorMsg] = useState<string>("");
  const lastTextRef = useRef<string | null>(null);
  const lastResultRef = useRef<TranslateResult | null>(null);
  const translatingRef = useRef(false);

  /**
   * 处理一次「待译文本 + 来源」输入：text 为 null=无可译内容；与上次同文本且有缓存则
   * 秒级还原；新文本则先清旧结果再联网翻译。翻译中再来输入直接忽略（互斥防重入）。
   *
   * 失败后不更新 lastText/lastResult，保证同文本可重试（保住 M-1 回归意图）。
   */
  async function handleSource(
    payload: TransSourcePayload,
    cancelledRef: { value: boolean },
  ): Promise<void> {
    if (translatingRef.current) return;

    setSourceInfo(payload);
    const text = payload.text;

    if (text === null) {
      setResult(null);
      setStatus("empty");
      return;
    }

    // 同文本不重复联网：直接还原上次成功译文
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

    // 冷启动补读 Rust 的来源快照（含选中文字与来源标签），读不到再退回剪贴板自读
    void (async () => {
      try {
        const stored = await getTransPopoverSource();
        if (cancelledRef.value) return;
        if (stored !== null) {
          await handleSource(stored, cancelledRef);
          return;
        }

        const items = await listClipItems();
        if (cancelledRef.value) return;
        await handleSource(
          {
            text: pickLatestText(items),
            source: "clipboard",
            fallbackReason: null,
          },
          cancelledRef,
        );
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
        void handleSource(asTransSourcePayload(e.payload), cancelledRef);
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

  // 展开：把当前原文带到主窗翻译页预填并自动翻译，再切到翻译页。
  // 先 emit seed 再 emit route：主窗两个监听都在同一轮事件里处理，顺序与切页一致。
  async function handleExpand(): Promise<void> {
    try {
      const text = sourceInfo?.text ?? null;
      if (text) {
        await emit(TRANSLATE_SEED_EVENT, { text });
      }
      await emit(ROUTE_EVENT, "translate");
      const main = await WebviewWindow.getByLabel("main");
      await main?.show();
      await main?.setFocus();
      await getCurrentWindow().hide();
    } catch (err: unknown) {
      console.error("[trans-popover] expand failed:", err);
    }
  }

  if (status === "loading" || status === "translating") {
    // 骨架而不是 spinner：三条递减的占位块能预告「译文大概占几行」，
    // 转圈只能说「在等」。状态文案占住方向行的位置，屏幕阅读器与测试都靠它。
    return (
      <div className="mini-status mini-loading">
        <span className="mini-hint caption-5">翻译中…</span>
        <div className="mini-skeleton" aria-hidden="true">
          <span className="skeleton mini-skel" />
          <span className="skeleton mini-skel mini-skel--md" />
          <span className="skeleton mini-skel mini-skel--sm" />
        </div>
      </div>
    );
  }

  if (status === "empty") {
    // 取词通道不可用（未授权 / 平台不支持）时也要说清原因，否则用户会以为是自己没选中
    const degradedHint = fallbackHint(sourceInfo?.fallbackReason ?? null);
    return (
      <div className="mini-status mini-empty">
        <span className="mini-hint body-6">请先选中或复制文字再按 ⌘⇧T</span>
        {degradedHint !== undefined && (
          <span className="mini-degrade caption-5">{degradedHint}</span>
        )}
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="mini-status">
        <span className="mini-hint mini-error body-6">{errorMsg}</span>
      </div>
    );
  }

  return (
    <MiniTranslate
      result={result!}
      source={sourceInfo?.source ?? "clipboard"}
      fallbackReason={sourceInfo?.fallbackReason ?? null}
      onCopy={handleCopy}
      onSpeak={handleSpeak}
      onExpand={() => void handleExpand()}
    />
  );
}

export default TransPopoverApp;
