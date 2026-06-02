import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("../ipc/ipc-client", () => ({
  listClipItems: vi.fn(),
  translateText: vi.fn(),
}));

vi.mock("../panels/translate/browser-api", () => ({
  speakText: vi.fn(),
  writeToClipboard: vi.fn().mockResolvedValue(undefined),
}));

import { listClipItems, translateText } from "../ipc/ipc-client";
import { writeToClipboard, speakText } from "../panels/translate/browser-api";
import TransPopoverApp from "./TransPopoverApp";

const mockListClipItems = vi.mocked(listClipItems);
const mockTranslateText = vi.mocked(translateText);
const mockWriteToClipboard = vi.mocked(writeToClipboard);
const mockSpeakText = vi.mocked(speakText);

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
    mockWriteToClipboard.mockResolvedValue(undefined);
  });

  it("剪贴板有文本 → 调 translateText 并渲染译文", async () => {
    mockListClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mockTranslateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);

    const translated = await screen.findByText("你好，世界");
    expect(translated).toBeDefined();
    expect(mockTranslateText).toHaveBeenCalledWith("Hello world");
  });

  it("剪贴板为空 → 渲染降级文案", async () => {
    mockListClipItems.mockResolvedValue([]);
    mockTranslateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);

    const fallback = await screen.findByText(/请先复制文字/);
    expect(fallback).toBeDefined();
    expect(mockTranslateText).not.toHaveBeenCalled();
  });

  it("translateText 失败 → 渲染错误文案", async () => {
    mockListClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mockTranslateText.mockRejectedValue(new Error("network error"));

    render(<TransPopoverApp />);

    const errMsg = await screen.findByText(/翻译失败/);
    expect(errMsg).toBeDefined();
  });

  it("点复制 → writeToClipboard 以译文调用", async () => {
    mockListClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mockTranslateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);
    await screen.findByText("你好，世界");

    const copyBtn = screen.getByRole("button", { name: /复制/ });
    await userEvent.click(copyBtn);

    expect(mockWriteToClipboard).toHaveBeenCalledWith("你好，世界");
  });

  it("点朗读 → speakText 以译文调用", async () => {
    mockListClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mockTranslateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);
    await screen.findByText("你好，世界");

    const speakBtn = screen.getByRole("button", { name: /朗读/ });
    await userEvent.click(speakBtn);

    expect(mockSpeakText).toHaveBeenCalledWith("你好，世界");
  });
});
