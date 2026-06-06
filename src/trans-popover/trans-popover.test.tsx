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
  kind: "plain" as const,
  translated: "你好，世界",
  sourceLang: "en",
  targetLang: "zh",
};

const RESULT_B = {
  kind: "plain" as const,
  translated: "第二段译文",
  sourceLang: "en",
  targetLang: "zh",
};

/**
 * 按事件名分发的 listen mock：组件用 getCurrentWindow().listen 分别订阅
 * "trans-source" 与 "tauri://blur"，此 helper 把各自回调缓存供测试触发。
 */
function installListenDispatcher(): {
  fireTransSource: (payload: string | null) => void;
  fireBlur: () => void;
} {
  const handlers: Record<string, (e: { payload: unknown }) => void> = {};
  mocks.listen.mockImplementation(
    (event: string, cb: (e: { payload: unknown }) => void) => {
      handlers[event] = cb;
      return Promise.resolve(() => undefined);
    },
  );
  return {
    fireTransSource: (payload: string | null) =>
      handlers["trans-source"]?.({ payload }),
    fireBlur: () => handlers["tauri://blur"]?.({ payload: null }),
  };
}

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

  it("收到 trans-source 事件（新文本）→ 调 translateText 并渲染新译文", async () => {
    const dispatcher = installListenDispatcher();
    mocks.listClipItems.mockResolvedValue([]);
    mocks.translateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);
    await screen.findByText(/请先复制文字/);

    dispatcher.fireTransSource("Hello world");

    const translated = await screen.findByText("你好，世界");
    expect(translated).toBeDefined();
    expect(mocks.translateText).toHaveBeenCalledWith("Hello world");
  });

  it("残影修复：blur 复位后再收 trans-source 新文本 pending 时查不到旧结果A", async () => {
    const dispatcher = installListenDispatcher();
    mocks.listClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    // 首译立即返回结果A；第二段译文挂起（不 resolve），维持 pending 态
    mocks.translateText.mockResolvedValueOnce(MOCK_RESULT);
    let resolveSecond: ((v: typeof RESULT_B) => void) | undefined;
    mocks.translateText.mockReturnValueOnce(
      new Promise<typeof RESULT_B>((resolve) => {
        resolveSecond = resolve;
      }),
    );

    render(<TransPopoverApp />);
    await screen.findByText("你好，世界");

    // 仅触发 blur、尚未发新 trans-source：独立验证 blur 复位本身已清旧译文、
    // 切到「翻译中…」。此中间态断言保证「blur 复位」逻辑被删时用例必红
    // （否则新文本分支自身的 setResult(null) 会掩盖差异，导致假覆盖）。
    dispatcher.fireBlur();
    await screen.findByText("翻译中…");
    expect(screen.queryByText("你好，世界")).toBeNull();

    // 再发新文本（pending），沿用原有断言确认仍维持「翻译中…」、旧译文不回归
    dispatcher.fireTransSource("Second source");
    await screen.findByText("翻译中…");
    expect(screen.queryByText("你好，世界")).toBeNull();

    resolveSecond?.(RESULT_B);
  });

  it("同文本秒级还原：再收 trans-source 同文本 → 不再调 translateText 直接显示原译文", async () => {
    const dispatcher = installListenDispatcher();
    mocks.listClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mocks.translateText.mockResolvedValue(MOCK_RESULT);

    render(<TransPopoverApp />);
    await screen.findByText("你好，世界");
    expect(mocks.translateText).toHaveBeenCalledTimes(1);

    dispatcher.fireBlur();
    dispatcher.fireTransSource("Hello world");

    const restored = await screen.findByText("你好，世界");
    expect(restored).toBeDefined();
    expect(mocks.translateText).toHaveBeenCalledTimes(1);
  });

  it("M-1 回归：translateText 失败后 trans-source 同文本 → translateText 被再次调用（不被去重跳过）", async () => {
    const dispatcher = installListenDispatcher();

    let callCount = 0;
    mocks.listClipItems.mockResolvedValue([MOCK_TEXT_ITEM]);
    mocks.translateText.mockImplementation(() => {
      callCount += 1;
      if (callCount === 1) return Promise.reject(new Error("network error"));
      return Promise.resolve(MOCK_RESULT);
    });

    render(<TransPopoverApp />);
    await screen.findByText(/翻译失败/);

    expect(mocks.translateText).toHaveBeenCalledTimes(1);

    dispatcher.fireTransSource("Hello world");
    await screen.findByText("你好，世界");

    expect(mocks.translateText).toHaveBeenCalledTimes(2);
  });
});
