/**
 * 富文本预览链接点击解析（RT1-F2-S03）。
 *
 * 真机 bug：富文本预览里的 <a href> 被点击时 Tauri webview 直接导航到目标地址，
 * 把 app 顶掉退不回来。修复策略：点击时拦截、用系统默认浏览器打开外部链接。
 * 本模块只负责「从点击目标解析出应外部打开的 url」这一纯逻辑，便于单测；
 * 实际打开与 preventDefault 由调用方（ClipPreview / PopoverPreview）处理。
 */

import type { MouseEvent as ReactMouseEvent } from "react";
import { openExternalUrl } from "../translate/browser-api";

/** 允许走外部浏览器/邮件客户端打开的协议白名单（javascript: 已被 DOMPurify 剥离，file:// 等不放行）。 */
const EXTERNAL_LINK_SCHEMES = new Set(["http:", "https:", "mailto:"]);

/**
 * 从点击目标向上找最近的 <a>，校验其 href 协议。
 * 命中白名单协议返回规范化后的 url，否则（无 href / 非白名单协议 / 非链接）返回 null。
 */
export function resolveRichLinkClick(target: HTMLElement | null): string | null {
  const anchor = target?.closest("a");
  if (anchor === null || anchor === undefined) {
    return null;
  }

  // 用 getAttribute 而非 anchor.href：后者对裸 <a>（无 href）会返回 jsdom 的 base url，造成误判。
  const rawHref = anchor.getAttribute("href");
  if (rawHref === null || rawHref.trim() === "") {
    return null;
  }

  let parsed: URL;
  try {
    parsed = new URL(rawHref, window.location.href);
  } catch {
    return null;
  }

  if (!EXTERNAL_LINK_SCHEMES.has(parsed.protocol)) {
    return null;
  }
  return parsed.href;
}

/**
 * 富文本渲染容器的 onClick 委托处理器（ClipPreview / PopoverPreview 共用，避免两处重复）。
 * 命中外部链接时阻止 webview 默认导航并交系统浏览器打开；点的不是链接则放行不拦。
 */
export function handleRichLinkClick(event: ReactMouseEvent<HTMLElement>): void {
  const url = resolveRichLinkClick(event.target as HTMLElement);
  if (url === null) {
    return;
  }
  event.preventDefault();
  // openUrl 的 rejection（如 ACL 拒绝）必须显式记日志，不可静默吞，否则真机点链接"没反应"无从诊断。
  openExternalUrl(url).catch((err: unknown) => {
    console.error("[QuickQuick] 打开外部链接失败:", err);
  });
}
