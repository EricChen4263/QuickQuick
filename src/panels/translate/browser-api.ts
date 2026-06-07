/**
 * 浏览器 API 薄封装（V4-F2-S08）
 *
 * 把 navigator.clipboard / window.speechSynthesis / Tauri opener 隔离到独立模块，
 * 使测试可通过 vi.mock 替换，避免 jsdom secure-context 限制及 Tauri 运行时不可用导致 mock 失效。
 */

import { openUrl } from "@tauri-apps/plugin-opener";

/** 将文本写入剪贴板。 */
export async function writeToClipboard(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

/**
 * 用系统默认浏览器/邮件客户端打开外部 url（RT1-F2-S03）。
 * 富文本预览链接点击必须经此走系统外部程序，绝不让 Tauri webview 自身导航把 app 顶掉。
 * 调用方负责协议白名单校验（见 resolveRichLinkClick）。
 */
export async function openExternalUrl(url: string): Promise<void> {
  await openUrl(url);
}

/** 朗读文本（使用 Web Speech API）。 */
export function speakText(text: string): void {
  const utterance = new SpeechSynthesisUtterance(text);
  window.speechSynthesis.speak(utterance);
}
