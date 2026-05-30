import { useState } from "react";
import { resolveRoute, type HotkeyTrigger, type WindowView } from "./shell/windowRoute";

/** QuickQuick 预热窗口根组件：历史/翻译共用单窗口按路由切换视图 */
function App() {
  const [currentView, setCurrentView] = useState<WindowView>("history");

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
