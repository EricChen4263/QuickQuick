import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type TopLevel } from "./main-window/nav";
import { type HotkeyTrigger } from "./shell/windowRoute";
import { AppShell } from "./shell/AppShell";
import ClipboardPage from "./panels/clipboard/ClipboardPage";
import TranslatePage from "./panels/translate/TranslatePage";
import SettingsPage from "./panels/settings/SettingsPage";
import "./theme/theme.css";

/** 热键路由 payload 类型（与后端 emit 的字符串对应） */
type RoutePayload = HotkeyTrigger;

/** 每个一级页对应的热键 hint 文案 */
const PAGE_HINTS: Record<TopLevel, string> = {
  clipboard: "⌘⇧V",
  translate: "⌘⇧T",
  settings: "设置",
};

/**
 * 将热键触发类型映射到对应的一级页。
 * history 热键对应剪贴板页；translate 热键对应翻译页。
 */
function routeToTopLevel(trigger: HotkeyTrigger): TopLevel {
  if (trigger === "translate") return "translate";
  return "clipboard";
}

/** QuickQuick 主窗口根组件：AppShell 包裹三页，路由层保持不变 */
function App() {
  const [activeTop, setActiveTop] = useState<TopLevel>("clipboard");

  // 监听后端 route 事件，切换一级页
  // cancelled flag 防止组件卸载后 Promise resolve 造成监听器泄漏
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<RoutePayload>("route", (event) => {
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
      hint={PAGE_HINTS[activeTop]}
    >
      <section
        data-testid="page-clipboard"
        style={{ display: activeTop === "clipboard" ? "block" : "none", height: "100%" }}
      >
        <ClipboardPage />
      </section>
      <section
        data-testid="page-translate"
        style={{ display: activeTop === "translate" ? "block" : "none", height: "100%" }}
      >
        <TranslatePage />
      </section>
      <section
        data-testid="page-settings"
        style={{ display: activeTop === "settings" ? "block" : "none", height: "100%" }}
      >
        <SettingsPage />
      </section>
    </AppShell>
  );
}

export default App;
