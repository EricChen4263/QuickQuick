/**
 * 主窗口导航路由（纯函数，无副作用）
 *
 * 一级入口固定为三项：剪贴板 / 翻译 / 设置。
 * 历史视图均归属各自模块的二级子视图（§九.3）。
 */

export type TopLevel = "clipboard" | "translate" | "settings";

export type ClipboardSub = "list" | "history";
export type TranslateSub = "workspace" | "history";
export type SettingsSub = "general" | "hotkey" | "translate-source" | "privacy" | "storage" | "about";

export type SubView = ClipboardSub | TranslateSub | SettingsSub;

export interface NavState {
  top: TopLevel;
  sub: SubView;
}

const SUB_VIEWS: Record<TopLevel, SubView[]> = {
  clipboard: ["list", "history"],
  translate: ["workspace", "history"],
  settings: ["general", "hotkey", "translate-source", "privacy", "storage", "about"],
};

const DEFAULT_SUB: Record<TopLevel, SubView> = {
  clipboard: "list",
  translate: "workspace",
  settings: "general",
};

/** 返回左侧边栏一级导航入口（固定三项，顺序不变）。 */
export function topLevelEntries(): TopLevel[] {
  return ["clipboard", "translate", "settings"];
}

/** 返回指定一级入口下的所有二级子视图列表。 */
export function subViewsOf(top: TopLevel): SubView[] {
  return SUB_VIEWS[top];
}

/**
 * 解析导航状态。
 *
 * sub 不在该 top 的合法子视图列表内时，回退到该 top 的默认子视图。
 */
export function resolveNav(top: TopLevel, sub?: string): NavState {
  const validSubs = SUB_VIEWS[top] as string[];
  const resolved = sub !== undefined && validSubs.includes(sub)
    ? (sub as SubView)
    : DEFAULT_SUB[top];

  return { top, sub: resolved };
}
