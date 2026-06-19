import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * TitleBar：自绘标题栏。
 *
 * macOS（Overlay 模式）：系统红绿灯浮在 webview 左上角，webview 内容延伸到顶部，
 * 标题栏左侧留出约 106px 空间避免标题文字压住红绿灯；整条加 data-tauri-drag-region
 * 使整个标题栏区域均可拖动移窗。
 *
 * Windows：tauri.conf 无法按平台配 decorations，运行时由后端关掉原生标题栏
 * （见 setup_main_window_behavior），故前端自绘栏须自带最小化/最大化/关闭三按钮，
 * 否则用户无法控制窗口。左 padding 收回（106px 是给 macOS 红绿灯留的）。
 */

// 平台检测：模块级常量，import 时由 userAgent 求值一次。不引新依赖。
const IS_WINDOWS = navigator.userAgent.includes("Windows");

export function TitleBar() {
  const barClassName = IS_WINDOWS ? "qq-titlebar qq-titlebar--win" : "qq-titlebar";

  return (
    <div className={barClassName} data-tauri-drag-region>
      <span className="qq-titlebar-brand" data-tauri-drag-region>
        <span className="brand-accent">Quick</span>Quick
      </span>
      {IS_WINDOWS && <WindowControls />}
    </div>
  );
}

/**
 * Windows 窗口控制按钮（最小化 / 最大化 / 关闭）。
 *
 * 容器不加 data-tauri-drag-region：否则点击会被当作拖动移窗、按钮失效；
 * 拖动区域只保留在品牌文字区。
 */
function WindowControls() {
  const win = getCurrentWindow();
  return (
    <div className="qq-titlebar-controls">
      <button
        type="button"
        className="qq-titlebar-btn"
        aria-label="最小化"
        onClick={() => win.minimize().catch((e: unknown) => console.warn("[TitleBar] minimize 失败", e))}
      >
        &#xE921;
      </button>
      <button
        type="button"
        className="qq-titlebar-btn"
        aria-label="最大化"
        onClick={() => win.toggleMaximize().catch((e: unknown) => console.warn("[TitleBar] toggleMaximize 失败", e))}
      >
        &#xE922;
      </button>
      <button
        type="button"
        className="qq-titlebar-btn qq-titlebar-btn--close"
        aria-label="关闭"
        onClick={() => win.close().catch((e: unknown) => console.warn("[TitleBar] close 失败", e))}
      >
        &#xE8BB;
      </button>
    </div>
  );
}
