import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type TopLevel } from "./main-window/nav";
import { type RoutePayload } from "./shell/windowRoute";
import { ROUTE_EVENT } from "./ipc/events";
import { AppShell } from "./shell/AppShell";
import ClipboardPage from "./panels/clipboard/ClipboardPage";
import TranslatePage from "./panels/translate/TranslatePage";
import SettingsPage from "./panels/settings/SettingsPage";
import UpdateBanner from "./components/UpdateBanner";
import "./theme/theme.css";

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

  /**
   * 剪贴板页一键翻译回调：填入 seed 并切换到翻译页。
   * nonce 自增确保相同文本重复点击也能重新触发翻译。
   */
  function handleTranslateFromClip(content: string) {
    setTranslateSeed((prev) => ({ text: content, nonce: (prev?.nonce ?? 0) + 1 }));
    setActiveTop((_prev) => "translate");
  }

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
      <UpdateBanner />
    </AppShell>
  );
}

export default App;
