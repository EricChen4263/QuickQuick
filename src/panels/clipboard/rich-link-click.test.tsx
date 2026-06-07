/**
 * 富文本预览链接点击委托测试（RT1-F2-S03）。
 * 真机 bug：点富文本里的 <a> 导致 webview 自身导航把 app 顶掉。
 * 修复：点击链接走系统浏览器（openExternalUrl）并 preventDefault，非链接不拦。
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render } from "@testing-library/react";
import { ClipPreview } from "./ClipPreview";

vi.mock("../../ipc/ipc-client", () => ({
  getClipImageOriginal: vi.fn().mockResolvedValue(null),
  copyClipToClipboard: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../translate/browser-api", () => ({
  openExternalUrl: vi.fn().mockResolvedValue(undefined),
}));

import { openExternalUrl } from "../translate/browser-api";

const mockOpenExternalUrl = vi.mocked(openExternalUrl);

const RICH_ITEM = {
  id: "rich-1",
  content: "link text",
  kind: "richtext" as const,
  isFavorite: false,
  lastModifiedUtc: 1000,
  htmlContent: `<p><a href="https://x.com">x.com</a> and <b id="b">bold</b></p>`,
};

function renderRich() {
  render(
    <ClipPreview
      item={RICH_ITEM}
      onToggleFavorite={vi.fn()}
      onDelete={vi.fn()}
      onCopy={vi.fn()}
      onPasteToFront={vi.fn()}
      onTranslate={vi.fn()}
    />
  );
}

describe("富文本预览链接点击委托", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOpenExternalUrl.mockResolvedValue(undefined);
  });

  it("rich_link_click_opens_external_and_prevents_default", () => {
    renderRich();
    const anchor = document.querySelector(".preview-content a") as HTMLElement;
    expect(anchor).not.toBeNull();

    const clickEvent = new MouseEvent("click", { bubbles: true, cancelable: true });
    fireEvent(anchor, clickEvent);

    expect(mockOpenExternalUrl).toHaveBeenCalledTimes(1);
    expect(mockOpenExternalUrl).toHaveBeenCalledWith("https://x.com/");
    // preventDefault 被调用 => webview 不会自行导航
    expect(clickEvent.defaultPrevented).toBe(true);
  });

  it("rich_link_click_logs_error_when_open_rejects", async () => {
    const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    mockOpenExternalUrl.mockRejectedValueOnce(new Error("ACL denied"));
    renderRich();
    const anchor = document.querySelector(".preview-content a") as HTMLElement;

    const clickEvent = new MouseEvent("click", { bubbles: true, cancelable: true });
    fireEvent(anchor, clickEvent);
    // 等待被吞前的微任务队列：rejection 应被 .catch 捕获并记日志，而非静默
    await Promise.resolve();
    await Promise.resolve();

    expect(consoleErrorSpy).toHaveBeenCalledWith(
      "[QuickQuick] 打开外部链接失败:",
      expect.any(Error)
    );
    consoleErrorSpy.mockRestore();
  });

  it("rich_link_click_ignores_non_link_target", () => {
    renderRich();
    const bold = document.querySelector(".preview-content #b") as HTMLElement;
    expect(bold).not.toBeNull();

    const clickEvent = new MouseEvent("click", { bubbles: true, cancelable: true });
    fireEvent(bold, clickEvent);

    expect(mockOpenExternalUrl).not.toHaveBeenCalled();
    expect(clickEvent.defaultPrevented).toBe(false);
  });
});
