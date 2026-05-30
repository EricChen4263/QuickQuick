/**
 * 预热窗口路由逻辑（纯函数，无副作用）
 *
 * 历史面板与翻译面板共用单一预热 webview，
 * 按触发热键类型路由到不同视图（§八(3)）。
 */

/** 热键触发类型：history = Cmd/Ctrl+Shift+V，translate = Cmd/Ctrl+Shift+T */
export type HotkeyTrigger = "history" | "translate";

/** 预热窗口当前显示的视图 */
export type WindowView = "history" | "translate";

/**
 * 根据热键触发类型解析应展示的窗口视图。
 *
 * @param trigger - 触发来源（history 或 translate）
 * @returns 对应的窗口视图标识
 */
export function resolveRoute(trigger: HotkeyTrigger): WindowView {
  return trigger;
}
