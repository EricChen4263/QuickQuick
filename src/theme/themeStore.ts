/**
 * 主题偏好单例（纯 TS，无 React 依赖）
 *
 * 设计决策：
 * - 模块级单例：import 即初始化，避免构造时序问题。
 * - auto 模式跟随 matchMedia，light/dark 覆盖系统偏好。
 * - dataset.theme 写入集中在此处，是唯一允许操作该 DOM 的地方。
 * - 双轨持久化：localStorage（同步镜像，消除首屏闪烁）+ IPC（settings.json 权威）。
 *   writePref 同时写 localStorage 和 fire-and-forget IPC；
 *   hydrateFromIpc 在 init 末尾异步拉取 IPC 值，含竞争防御。
 */

import { getTheme, setTheme } from "../ipc/ipc-client";

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
  // 同步镜像，消除首屏闪烁
  localStorage.setItem(STORAGE_KEY, value);
  // fire-and-forget：IPC 持久化失败不阻断本地流程
  void setTheme(value).catch(() => {
    console.warn("[themeStore] setTheme IPC failed, localStorage remains authoritative");
  });
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

/**
 * 异步从 IPC 拉取主题值并与本地状态对比。
 * 竞争防御：记录调用时的 pref 快照，仅当 IPC 返回后 pref 未被手动修改时才应用 IPC 值。
 */
async function hydrateFromIpc(): Promise<void> {
  const initialPref = pref;
  try {
    const ipcValue = await getTheme();
    if (ipcValue !== "light" && ipcValue !== "dark" && ipcValue !== "auto") return;
    // 竞争防御：用户期间手动改了则放弃覆盖
    if (pref !== initialPref) return;
    if (ipcValue !== pref) {
      pref = ipcValue;
      localStorage.setItem(STORAGE_KEY, ipcValue);
      applyResolved();
    }
  } catch {
    console.warn("[themeStore] hydrateFromIpc failed, keeping localStorage value");
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

  // 不 await，不阻塞同步初始化；localStorage 先行，IPC 异步校正
  void hydrateFromIpc();
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
