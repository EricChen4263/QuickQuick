import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const mocks = vi.hoisted(() => ({
  listClipItems: vi.fn(),
  translateText: vi.fn(),
  speakText: vi.fn(),
  writeToClipboard: vi.fn().mockResolvedValue(undefined),
  hide: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => undefined),
  emit: vi.fn().mockResolvedValue(undefined),
  mainShow: vi.fn().mockResolvedValue(undefined),
  mainSetFocus: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../ipc/ipc-client", () => ({
  listClipItems: mocks.listClipItems,
  translateText: mocks.translateText,
}));

vi.mock("../panels/translate/browser-api", () => ({
  speakText: mocks.speakText,
  writeToClipboard: mocks.writeToClipboard,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    hide: mocks.hide,
    listen: mocks.listen,
  })),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: {
    getByLabel: vi.fn().mockImplementation(() =>
      Promise.resolve({
        show: mocks.mainShow,
        setFocus: mocks.mainSetFocus,
      }),
    ),
  },
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: mocks.emit,
}));

import TransPopoverApp from "./TransPopoverApp";

const MOCK_TEXT_ITEM = {
  id: "clip-1",
  content: "Hello world",
  kind: "text" as const,
  isFavorite: false,
  lastModifiedUtc: 1000,
};

const MOCK_RESULT = {
  translated: "你好，世界",
  sourceLang: "en",
  targetLang: "zh",
};

describe("TransPopoverApp", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.writeToClipboard.mockResolvedValue(undefined);
    mocks.hide.mockResolvedValue(undefined);
    mocks.listen.mockResolvedValue(() => undefined);
    mocks.emit.mockResolvedValue(undefined);
    mocks.mainShow.mockResolvedValue(undefined);
    mocks.mainSetFocus.mockResolvedValue(undefined);
  });

  it("剪贴板有文本 → 调 translateText 并渲染译文", async () => {
    mocks.listClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mocks.translateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);

    const translated = await screen.findByText("你好，世界");
    expect(translated).toBeDefined();
    expect(mocks.translateText).toHaveBeenCalledWith("Hello world");
  });

  it("剪贴板为空 → 渲染降级文案", async () => {
    mocks.listClipItems.mockResolvedValue([]);
    mocks.translateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);

    const fallback = await screen.findByText(/请先复制文字/);
    expect(fallback).toBeDefined();
    expect(mocks.translateText).not.toHaveBeenCalled();
  });

  it("translateText 失败 → 渲染错误文案", async () => {
    mocks.listClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mocks.translateText.mockRejectedValue(new Error("network error"));

    render(<TransPopoverApp />);

    const errMsg = await screen.findByText(/翻译失败/);
    expect(errMsg).toBeDefined();
  });

  it("点复制 → writeToClipboard 以译文调用", async () => {
    mocks.listClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mocks.translateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);
    await screen.findByText("你好，世界");

    const copyBtn = screen.getByRole("button", { name: /复制/ });
    await userEvent.click(copyBtn);

    expect(mocks.writeToClipboard).toHaveBeenCalledWith("你好，世界");
  });

  it("点朗读 → speakText 以译文调用", async () => {
    mocks.listClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mocks.translateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);
    await screen.findByText("你好，世界");

    const speakBtn = screen.getByRole("button", { name: /朗读/ });
    await userEvent.click(speakBtn);

    expect(mocks.speakText).toHaveBeenCalledWith("你好，世界");
  });

  it("点展开 → emit('route','translate')、main.show、main.setFocus、当前窗口 hide 依序调用", async () => {
    mocks.listClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mocks.translateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);
    await screen.findByText("你好，世界");

    const expandBtn = screen.getByRole("button", { name: /展开/ });
    await userEvent.click(expandBtn);

    expect(mocks.emit).toHaveBeenCalledWith("route", "translate");
    expect(mocks.mainShow).toHaveBeenCalled();
    expect(mocks.mainSetFocus).toHaveBeenCalled();
    expect(mocks.hide).toHaveBeenCalled();
  });
});
