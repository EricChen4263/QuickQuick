import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import TranslatePage from "./TranslatePage";

// Mock IPC：渲染测试环境无 Tauri 运行时
// getTranslateProviders / getSelectedProvider / setSelectedProvider 需补齐，
// 否则 TranslatePage 挂载时的 provider fetch 会 reject，干扰翻译/历史断言。
vi.mock("../../ipc/ipc-client", () => ({
  translateText: vi.fn(),
  listTranslateHistory: vi.fn(),
  getTranslateProviders: vi.fn(),
  getSelectedProvider: vi.fn(),
  setSelectedProvider: vi.fn(),
}));

// Mock browser-api：隔离 navigator.clipboard / speechSynthesis（jsdom secure-context 限制）
vi.mock("./browser-api", () => ({
  writeToClipboard: vi.fn().mockResolvedValue(undefined),
  speakText: vi.fn(),
}));

import {
  translateText,
  listTranslateHistory,
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
} from "../../ipc/ipc-client";
import { writeToClipboard, speakText } from "./browser-api";

const mockTranslateText = vi.mocked(translateText);
const mockListTranslateHistory = vi.mocked(listTranslateHistory);
const mockGetTranslateProviders = vi.mocked(getTranslateProviders);
const mockGetSelectedProvider = vi.mocked(getSelectedProvider);
const mockSetSelectedProvider = vi.mocked(setSelectedProvider);
const mockWriteToClipboard = vi.mocked(writeToClipboard);
const mockSpeakText = vi.mocked(speakText);

/** 测试用翻译历史数据 */
const MOCK_HISTORY = [
  {
    id: "h-1",
    sourceText: "Hello",
    translatedText: "你好",
    sourceLang: "en",
    targetLang: "zh",
    providerId: "mock",
    createdUtc: 2000,
  },
  {
    id: "h-2",
    sourceText: "Good morning",
    translatedText: "早上好",
    sourceLang: "en",
    targetLang: "zh",
    providerId: "mock",
    createdUtc: 1000,
  },
];

/** 测试用翻译结果 */
const MOCK_RESULT = {
  translated: "你好世界",
  sourceLang: "en",
  targetLang: "zh",
};

describe("translate-page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListTranslateHistory.mockResolvedValue(MOCK_HISTORY);
    // provider fetch 必须返回有效值，否则 TranslatePage 挂载时 reject 会写 console.error 干扰断言
    mockGetTranslateProviders.mockResolvedValue([
      { id: "mymemory", name: "MyMemory · 默认", needsKey: false },
    ]);
    mockGetSelectedProvider.mockResolvedValue("mymemory");
    mockSetSelectedProvider.mockResolvedValue(undefined);
    // browser-api mock 的默认实现已在 vi.mock factory 中设置，clearAllMocks 后恢复默认即可
    mockWriteToClipboard.mockResolvedValue(undefined);
  });

  it("translate-page: 输入文本点击翻译后调用 translateText 并显示译文和语言方向", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    // Act：输入文本并点击翻译
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello World");
    const translateBtn = screen.getByRole("button", { name: /翻译/ });
    await user.click(translateBtn);

    // Assert：显示译文文本
    await waitFor(() => {
      expect(screen.getByText("你好世界")).toBeInTheDocument();
    });
    // Assert：translateText 被以输入文本调用（第二参为 undefined，由实现自动检测语言）
    expect(mockTranslateText).toHaveBeenCalledWith("Hello World", undefined);
    // Assert：方向标识（sourceLang → targetLang）——DirBar lang-pill 和译文 field-label 均含此方向，
    // 用 getAllByText 避免多元素报错，断言至少一处存在即满足语义
    expect(screen.getAllByText(/en.*zh|en\s*→\s*zh/).length).toBeGreaterThanOrEqual(1);
  });

  it("translate-page: 翻译历史列表渲染——显示各历史条目的 sourceText 和 translatedText", async () => {
    // Arrange：不需要翻译，只验证历史列表
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    render(<TranslatePage />);

    // Assert：历史列表出现各条目文本
    await waitFor(() => {
      expect(screen.getByText("Hello")).toBeInTheDocument();
    });
    expect(screen.getByText("你好")).toBeInTheDocument();
    expect(screen.getByText("Good morning")).toBeInTheDocument();
    expect(screen.getByText("早上好")).toBeInTheDocument();
  });

  it("translate-page: 点击历史某项后工作区回填（input 变为该项 sourceText，结果显示其 translatedText）", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    // 等历史列表渲染
    await waitFor(() => {
      expect(screen.getByText("Hello")).toBeInTheDocument();
    });

    // Act：点击第一条历史项（通过 data-testid 精确定位）
    const historyItem = screen.getByTestId("history-item-h-1");
    await user.click(historyItem);

    // Assert：输入框回填为该项 sourceText
    await waitFor(() => {
      const textarea = screen.getByRole("textbox");
      expect((textarea as HTMLTextAreaElement).value).toBe("Hello");
    });
    // Assert：结果区至少有一处显示该项 translatedText（历史栏和结果区可能同时出现）
    expect(screen.getAllByText("你好").length).toBeGreaterThanOrEqual(1);
  });

  it("translate-page: copy 按钮调用 navigator.clipboard.writeText 并传译文", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    // 先执行翻译使译文出现
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello World");
    await user.click(screen.getByRole("button", { name: /翻译/ }));
    await waitFor(() => {
      expect(screen.getByText("你好世界")).toBeInTheDocument();
    });

    // Act：点击复制按钮
    const copyBtn = screen.getByRole("button", { name: /复制/ });
    await user.click(copyBtn);

    // Assert：以译文调用 writeToClipboard（waitFor 等待 async handleAction resolve）
    await waitFor(() => {
      expect(mockWriteToClipboard).toHaveBeenCalledWith("你好世界");
    });
  });

  it("translate-page: translateText reject 时显示错误提示（role=alert）不崩溃", async () => {
    // Arrange
    mockTranslateText.mockRejectedValue(new Error("网络错误"));
    const user = userEvent.setup();
    render(<TranslatePage />);

    // Act：输入文本并点击翻译
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello");
    await user.click(screen.getByRole("button", { name: /翻译/ }));

    // Assert：错误提示出现，不崩溃
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(screen.getByRole("alert").textContent).toMatch(/失败|错误/);
  });

  it("translate-page: 翻译成功后再次翻译失败时错误提示出现", async () => {
    // Arrange：先成功再失败
    mockTranslateText
      .mockResolvedValueOnce(MOCK_RESULT)
      .mockRejectedValueOnce(new Error("第二次失败"));
    const user = userEvent.setup();
    render(<TranslatePage />);

    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello");

    // 第一次翻译成功
    await user.click(screen.getByRole("button", { name: /翻译/ }));
    await waitFor(() => {
      expect(screen.getByText("你好世界")).toBeInTheDocument();
    });

    // 第二次翻译失败
    await user.click(screen.getByRole("button", { name: /翻译/ }));

    // Assert：错误提示（role=alert）出现
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
  });

  it("translate-page: 空输入时翻译按钮禁用，不调用 translateText", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    // Assert：初始状态输入为空，翻译按钮禁用
    const translateBtn = screen.getByRole("button", { name: /翻译/ });
    expect(translateBtn).toBeDisabled();

    // Act：点击禁用按钮（不应触发）
    await user.click(translateBtn);

    // Assert：translateText 未被调用
    expect(mockTranslateText).not.toHaveBeenCalled();
  });

  it("translate-page: 历史列表容器有 role=listbox，点击历史项后 aria-selected 动态更新", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    await waitFor(() => {
      expect(screen.getByText("Hello")).toBeInTheDocument();
    });

    // Assert：列表容器有 role=listbox
    expect(screen.getByRole("listbox", { name: "翻译历史列表" })).toBeInTheDocument();

    // 初始无选中态
    const item1 = screen.getByTestId("history-item-h-1");
    const item2 = screen.getByTestId("history-item-h-2");
    expect(item1).toHaveAttribute("aria-selected", "false");
    expect(item2).toHaveAttribute("aria-selected", "false");

    // Act：点击第一条历史项
    await user.click(item1);

    // Assert：item1 选中，item2 未选中
    expect(item1).toHaveAttribute("aria-selected", "true");
    expect(item2).toHaveAttribute("aria-selected", "false");

    // Act：点击第二条历史项
    await user.click(item2);

    // Assert：item2 选中，item1 不再选中
    expect(item2).toHaveAttribute("aria-selected", "true");
    expect(item1).toHaveAttribute("aria-selected", "false");
  });

  it("translate-page: listTranslateHistory 返回空数组时显示空历史占位文案", async () => {
    // Arrange
    mockListTranslateHistory.mockResolvedValue([]);
    render(<TranslatePage />);

    // Assert：空历史占位文案
    await waitFor(() => {
      expect(screen.getByText(/暂无翻译历史|无翻译历史/)).toBeInTheDocument();
    });
  });

  it("translate-page: speak 按钮调用 speakText 并传译文", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    // 先执行翻译
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello World");
    await user.click(screen.getByRole("button", { name: /翻译/ }));
    await waitFor(() => {
      expect(screen.getByText("你好世界")).toBeInTheDocument();
    });

    // Act：点击朗读按钮
    const speakBtn = screen.getByRole("button", { name: /朗读/ });
    await user.click(speakBtn);

    // Assert：speakText 以译文被调用
    await waitFor(() => {
      expect(mockSpeakText).toHaveBeenCalledWith("你好世界");
    });
  });

  it("translate-page: swap 按钮点击后把译文填入输入框并以 sourceLang 为目标语重新翻译", async () => {
    const swapResult = { translated: "生活", sourceLang: "en", targetLang: "zh" };
    mockTranslateText
      .mockResolvedValueOnce(MOCK_RESULT)
      .mockResolvedValueOnce(swapResult);
    const user = userEvent.setup();
    render(<TranslatePage />);

    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Life");
    await user.click(screen.getByRole("button", { name: /^翻译$/ }));
    await waitFor(() => {
      expect(screen.getByText("你好世界")).toBeInTheDocument();
    });

    const swapBtn = screen.getByRole("button", { name: "交换语言方向" });
    await user.click(swapBtn);

    await waitFor(() => {
      expect(mockTranslateText).toHaveBeenCalledWith(MOCK_RESULT.translated, MOCK_RESULT.sourceLang);
    });
    await waitFor(() => {
      expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(MOCK_RESULT.translated);
    });
  });

  it("translate-page: 无翻译结果时 swap 按钮禁用", async () => {
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    render(<TranslatePage />);

    await waitFor(() => {
      expect(screen.getByText("Hello")).toBeInTheDocument();
    });

    const swapBtn = screen.getByRole("button", { name: "交换语言方向" });
    expect(swapBtn).toBeDisabled();
  });

  it("translate-page: sourceLang 为 auto 时 swap 按钮禁用", async () => {
    const autoResult = { translated: "Life", sourceLang: "auto", targetLang: "en" };
    mockTranslateText.mockResolvedValue(autoResult);
    const user = userEvent.setup();
    render(<TranslatePage />);

    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "生活");
    await user.click(screen.getByRole("button", { name: /^翻译$/ }));
    await waitFor(() => {
      expect(screen.getByText("Life")).toBeInTheDocument();
    });

    const swapBtn = screen.getByRole("button", { name: "交换语言方向" });
    expect(swapBtn).toBeDisabled();
  });

  it("translate-page: copy 操作 reject 时显示错误提示（role=alert）", async () => {
    // Arrange：writeToClipboard 模拟失败
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    mockWriteToClipboard.mockRejectedValue(new Error("剪贴板不可用"));
    const user = userEvent.setup();
    render(<TranslatePage />);

    // 先执行翻译使译文出现
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello World");
    await user.click(screen.getByRole("button", { name: /翻译/ }));
    await waitFor(() => {
      expect(screen.getByText("你好世界")).toBeInTheDocument();
    });

    // Act：点击复制按钮（writeToClipboard 将 reject）
    const copyBtn = screen.getByRole("button", { name: /复制/ });
    await user.click(copyBtn);

    // Assert：错误提示（role=alert）出现且文案包含「失败」
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(screen.getByRole("alert").textContent).toMatch(/失败/);
  });

  it("translate-page: seed prop 传入文本后自动填入输入框并调用 translateText", async () => {
    // RED：TranslatePage 尚不接受 seed prop，此测试预期失败
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    render(<TranslatePage seed={{ text: "hello", nonce: 1 }} />);

    // Assert：translateText 被以 "hello" 调用
    await waitFor(() => {
      expect(mockTranslateText).toHaveBeenCalledWith("hello", undefined);
    });
    // Assert：输入框填入了 seed.text
    const textarea = screen.getByRole("textbox");
    expect((textarea as HTMLTextAreaElement).value).toBe("hello");
    // Assert：译文渲染出来
    await waitFor(() => {
      expect(screen.getByText("你好世界")).toBeInTheDocument();
    });
  });

  it("translate-page: seed nonce 自增时相同文本再次触发 translateText", async () => {
    // 验证：nonce 变化时，即使 text 相同，也会再次触发翻译
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const { rerender } = render(<TranslatePage seed={{ text: "hello", nonce: 1 }} />);

    await waitFor(() => {
      expect(mockTranslateText).toHaveBeenCalledTimes(1);
    });

    // Act：nonce 自增，文本不变
    rerender(<TranslatePage seed={{ text: "hello", nonce: 2 }} />);

    await waitFor(() => {
      expect(mockTranslateText).toHaveBeenCalledTimes(2);
    });
    expect(mockTranslateText).toHaveBeenNthCalledWith(2, "hello", undefined);
  });

  it("translate-page: seed 为 null 时不调用 translateText", async () => {
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    render(<TranslatePage seed={null} />);

    // 等待挂载完成（provider fetch 等）
    await waitFor(() => {
      expect(screen.getByRole("textbox")).toBeInTheDocument();
    });

    expect(mockTranslateText).not.toHaveBeenCalled();
  });

  it("translate-page: seed.text 为空字符串时不调用 translateText", async () => {
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    render(<TranslatePage seed={{ text: "   ", nonce: 1 }} />);

    await waitFor(() => {
      expect(screen.getByRole("textbox")).toBeInTheDocument();
    });

    expect(mockTranslateText).not.toHaveBeenCalled();
  });
});
