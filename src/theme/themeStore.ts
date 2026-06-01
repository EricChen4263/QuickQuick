/**
 * 主题偏好单例（纯 TS，无 React 依赖）
 *
 * 设计决策：
 * - 模块级单例：import 即初始化，避免构造时序问题。
 * - auto 模式跟随 matchMedia，light/dark 覆盖系统偏好。
 * - dataset.theme 写入集中在此处，是唯一允许操作该 DOM 的地方。
 * - localStorage 读写封装为 readPref/writePref，里程碑3只换这两处为 IPC。
 */

export type ThemePref = "auto" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

type Listener = () => void;

const STORAGE_KEY = "qq-theme-pref";

let pref: ThemePref = "auto";
let listeners: Set<Listener> = new Set();
let mediaQuery: MediaQueryList | null = null;

function readPref(): ThemePref {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "light" || stored === "dark" || stored === "auto") {
    return stored;
  }
  return "auto";
}

function writePref(value: ThemePref): void {
  localStorage.setItem(STORAGE_KEY, value);
}

function resolveTheme(currentPref: ThemePref): ResolvedTheme {
  if (currentPref === "light") return "light";
  if (currentPref === "dark") return "dark";
  // auto：跟随系统，无 matchMedia 时回退 light
  return mediaQuery?.matches ? "dark" : "light";
}

function applyResolved(): void {
  const resolved = resolveTheme(pref);
  document.documentElement.dataset["theme"] = resolved;
  for (const listener of listeners) {
    listener();
  }
}

function handleMediaChange(): void {
  if (pref === "auto") {
    applyResolved();
  }
}

function init(): void {
  // SSR/Node 环境无 window，跳过所有 DOM/localStorage 访问
  if (typeof window === "undefined") return;

  pref = readPref();

  if (typeof window !== "undefined" && window.matchMedia) {
    mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    mediaQuery.addEventListener("change", handleMediaChange);
  }

  // 初始写入，不触发 listeners（mount 前尚无订阅者）
  document.documentElement.dataset["theme"] = resolveTheme(pref);
}

export function getPref(): ThemePref {
  return pref;
}

export function getResolved(): ResolvedTheme {
  return resolveTheme(pref);
}

export function setPref(newPref: ThemePref): void {
  pref = newPref;
  writePref(newPref);
  applyResolved();
}

export function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * 仅测试用：重置单例状态，防止测试间状态泄漏。
 * 生产代码不得调用此函数。
 */
export function _reset(): void {
  pref = "auto";
  listeners = new Set();
  localStorage.removeItem(STORAGE_KEY);
  delete document.documentElement.dataset["theme"];
  if (mediaQuery) {
    mediaQuery.removeEventListener("change", handleMediaChange);
    mediaQuery = null;
  }
}

init();
