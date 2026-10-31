import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type TopLevel } from "./main-window/nav";
import { type RoutePayload } from "./shell/windowRoute";
import { ROUTE_EVENT, TRANSLATE_SEED_EVENT, TRAY_CLEAR_UNPINNED_REQUEST_EVENT } from "./ipc/events";
import { clearUnpinnedHistory } from "./ipc/ipc-client";
import { AppShell } from "./shell/AppShell";
import ClipboardPage from "./panels/clipboard/ClipboardPage";
import TranslatePage from "./panels/translate/TranslatePage";
import SettingsPage from "./panels/settings/SettingsPage";
import UpdateBanner from "./components/UpdateBanner";
import "./theme/theme.css";

/**
 * 清空未固定的二次确认文案。
 *
 * 必须写明「收藏项会保留」：用户看到破坏性确认时唯一想知道的就是代价边界，
 * 只说「不可撤销」等于让人盲选。文案里的结论与后端行为一致——收藏项由
 * `cleanup_keep_recent` 的 `is_favorite = 0` 条件豁免。
 */
const CLEAR_UNPINNED_CONFIRM_TEXT = "清空未固定历史？收藏项会保留，此操作不可撤销。";

/** 结果提示的停留时长：够读完一句话，又不长期占用右下角。 */
const CLEAR_UNPINNED_NOTICE_MS = 4000;

/**
 * 将路由 payload 映射到对应的一级页。
 * history 热键对应剪贴板页；translate 热键对应翻译页；settings 来自托盘「打开设置」。
 */
function routeToTopLevel(payload: RoutePayload): TopLevel {
  if (payload === "translate") return "translate";
  if (payload === "settings") return "settings";
  return "clipboard";
}

/** QuickQuick 主窗口根组件：AppShell 包裹三页，路由层保持不变 */
function App() {
  const [activeTop, setActiveTop] = useState<TopLevel>("clipboard");
  const [translateSeed, setTranslateSeed] = useState<{ text: string; nonce: number } | null>(null);
  const [clearNotice, setClearNotice] = useState<string | null>(null);

  /**
   * 剪贴板页一键翻译回调：填入 seed 并切换到翻译页。
   * nonce 自增确保相同文本重复点击也能重新触发翻译。
   */
  function handleTranslateFromClip(content: string) {
    setTranslateSeed((prev) => ({ text: content, nonce: (prev?.nonce ?? 0) + 1 }));
    setActiveTop((_prev) => "translate");
  }

  // 浮窗「展开」带来的待译文本：与剪贴板页一键翻译走同一条预填路径（填入 seed 并切到翻译页）。
  // 监听取消标记同 route 监听，防止卸载后 Promise 回写造成监听泄漏。
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<{ text: string }>(TRANSLATE_SEED_EVENT, (event) => {
      const text = event.payload?.text;
      if (typeof text === "string" && text.trim() !== "") {
        handleTranslateFromClip(text);
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    }).catch((err: unknown) => {
      console.error("[QuickQuick] translate-seed 监听注册失败:", err);
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // 监听后端路由事件，切换一级页（热键 history/translate + 托盘「打开设置」）
  // cancelled flag 防止组件卸载后 Promise resolve 造成监听器泄漏
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<RoutePayload>(ROUTE_EVENT, (event) => {
      setActiveTop((_prev) => routeToTopLevel(event.payload));
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    }).catch((err: unknown) => {
      console.error("[QuickQuick] route 监听注册失败:", err);
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // 托盘「清空未固定…」：确认与执行都在前端完成。
  // 托盘侧只发请求事件（原生菜单承载不了确认框），删除动作串行在用户确认之后：
  // 用户取消时**不得**调用后端命令——不可撤销的操作不能只靠文案兜底。
  // 与 route 监听同理用 cancelled flag 防卸载后 resolve 造成监听器泄漏与越界 setState。
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen(TRAY_CLEAR_UNPINNED_REQUEST_EVENT, () => {
      if (!window.confirm(CLEAR_UNPINNED_CONFIRM_TEXT)) return;
      clearUnpinnedHistory()
        .then((result) => {
          if (!cancelled) setClearNotice(`已清空 ${String(result.softDeleted)} 条非收藏项`);
        })
        .catch((err: unknown) => {
          if (cancelled) return;
          const reason = err instanceof Error ? err.message : String(err);
          setClearNotice(`清空未固定失败：${reason}`);
          console.error("[QuickQuick] 清空未固定失败:", err);
        });
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    }).catch((err: unknown) => {
      console.error("[QuickQuick] 清空未固定请求监听注册失败:", err);
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // 结果提示到点自动消失：不让用户手动关，也不长期占住右下角
  useEffect(() => {
    if (clearNotice === null) return;
    const timer = window.setTimeout(() => setClearNotice(null), CLEAR_UNPINNED_NOTICE_MS);
    return () => window.clearTimeout(timer);
  }, [clearNotice]);

  // Esc 键隐藏窗口
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        getCurrentWindow().hide().catch((err: unknown) => {
          console.error("[QuickQuick] 隐藏窗口失败:", err);
        });
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  return (
    <AppShell
      activeTop={activeTop}
      onNavigate={(top) => setActiveTop((_prev) => top)}

    >
      <section
        data-testid="page-clipboard"
        style={{ display: activeTop === "clipboard" ? "block" : "none", height: "100%" }}
      >
        <ClipboardPage onTranslateItem={handleTranslateFromClip} />
      </section>
      <section
        data-testid="page-translate"
        style={{ display: activeTop === "translate" ? "block" : "none", height: "100%" }}
      >
        <TranslatePage seed={translateSeed} />
      </section>
      <section
        data-testid="page-settings"
        style={{ display: activeTop === "settings" ? "block" : "none", height: "100%" }}
      >
        <SettingsPage />
      </section>
      {/* 清空未固定的结果回执：复用更新提示条的非打扰样式（右下角浮层），
          不再新造一套提示视觉。role="status" 让读屏用户也能听到结论。 */}
      {clearNotice !== null && (
        <div className="update-banner" role="status" data-testid="clear-unpinned-result">
          <span className="update-banner-text">{clearNotice}</span>
        </div>
      )}
      <UpdateBanner />
    </AppShell>
  );
}

export default App;
