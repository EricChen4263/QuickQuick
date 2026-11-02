import type { ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * TitleBar：自绘顶部栏的容器。
 *
 * 结构简化史：原来是「44px 标题栏 + 112px 常驻侧栏」，左侧一级导航长期吃掉
 * 横向宽度。现在标题栏与一级导航合并成一条 48px 顶栏——剪贴板/翻译都是
 * 「列表 + 预览」双栏，横向空间是这类工具最稀缺的资源。
 *
 * macOS（Overlay 模式）：系统红绿灯浮在 webview 左上角，webview 内容延伸到顶部，
 * 故左 padding 留出 106px 避开红绿灯；整条加 data-tauri-drag-region 可拖动移窗。
 *
 * Windows：原生标题栏由后端关掉，前端须自带最小化/最大化/关闭三按钮，
 * 左 padding 收回，三按钮贴右。
 */

// 平台检测：模块级常量，import 时由 userAgent 求值一次。不引新依赖。
const IS_WINDOWS = navigator.userAgent.includes("Windows");

interface TitleBarProps {
  /** 中部插槽：一级导航。由应用外壳传入，标题栏本身不关心导航内容。 */
  children?: ReactNode;
}

export function TitleBar({ children }: TitleBarProps) {
  const barClassName = IS_WINDOWS ? "qq-titlebar qq-titlebar--win" : "qq-titlebar";

  return (
    <div className={barClassName} data-tauri-drag-region>
      {/* 可见品牌即文档 h1：应用外壳没有别的标题，单独加一个 sr-only 的
         同名 h1 会让读屏把「QuickQuick」念两遍。 */}
      <h1 className="qq-titlebar-brand title-3" data-tauri-drag-region>
        <span className="brand-accent">Quick</span>Quick
      </h1>
      {children}
      <div className="qq-titlebar-trail">{IS_WINDOWS && <WindowControls />}</div>
    </div>
  );
}

/**
 * Windows 窗口控制按钮（最小化 / 最大化 / 关闭）。
 *
 * 容器不加 data-tauri-drag-region：否则点击会被当作拖动移窗、按钮失效。
 * 图标用内联 SVG，不再用 Segoe MDL2 图标字体——全站描边语汇必须一致。
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
        <svg className="ico ico-14" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M5 12h14" />
        </svg>
      </button>
      <button
        type="button"
        className="qq-titlebar-btn"
        aria-label="最大化"
        onClick={() => win.toggleMaximize().catch((e: unknown) => console.warn("[TitleBar] toggleMaximize 失败", e))}
      >
        <svg className="ico ico-14" viewBox="0 0 24 24" aria-hidden="true">
          <rect x="5" y="5" width="14" height="14" rx="1.5" />
        </svg>
      </button>
      <button
        type="button"
        className="qq-titlebar-btn qq-titlebar-btn--close"
        aria-label="关闭"
        onClick={() => win.close().catch((e: unknown) => console.warn("[TitleBar] close 失败", e))}
      >
        <svg className="ico ico-14" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M6 6l12 12M18 6 6 18" />
        </svg>
      </button>
    </div>
  );
}
