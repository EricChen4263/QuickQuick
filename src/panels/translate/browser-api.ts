/**
 * 浏览器 API 薄封装（V4-F2-S08）
 *
 * 把 navigator.clipboard 和 window.speechSynthesis 隔离到独立模块，
 * 使测试可通过 vi.mock 替换，避免 jsdom secure-context 限制导致 mock 失效。
 */

/** 将文本写入剪贴板。 */
export async function writeToClipboard(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

/** 朗读文本（使用 Web Speech API）。 */
export function speakText(text: string): void {
  const utterance = new SpeechSynthesisUtterance(text);
  window.speechSynthesis.speak(utterance);
}
