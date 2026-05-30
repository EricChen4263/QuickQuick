import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { resolveRoute, type HotkeyTrigger, type WindowView } from "./shell/windowRoute";

/** Tauri `route` 事件 payload 类型（与后端 emit 的字符串对应） */
type RoutePayload = HotkeyTrigger;

/** QuickQuick 预热窗口根组件：历史/翻译共用单窗口按路由切换视图 */
function App() {
  const [currentView, setCurrentView] = useState<WindowView>("history");

  // 监听后端 route 事件，切换当前视图
  // cancelled flag 防止组件卸载后 Promise resolve 造成监听器泄漏
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<RoutePayload>("route", (event) => {
      setCurrentView(resolveRoute(event.payload));
    }).then((fn) => {
      if (cancelled) {
        // 组件已卸载，立即释放监听器
        fn();
      } else {
        unlisten = fn;
      }
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

  function handleTrigger(trigger: HotkeyTrigger) {
    setCurrentView(resolveRoute(trigger));
  }

  return (
    <div>
      <div style={{ display: currentView === "history" ? "block" : "none" }}>
        <h1>剪贴板历史</h1>
      </div>
      <div style={{ display: currentView === "translate" ? "block" : "none" }}>
        <h1>翻译</h1>
      </div>
      {/* 开发调试用按钮，生产不可见 */}
      {import.meta.env.DEV && (
        <div>
          <button onClick={() => handleTrigger("history")}>历史</button>
          <button onClick={() => handleTrigger("translate")}>翻译</button>
        </div>
      )}
    </div>
  );
}

export default App;
