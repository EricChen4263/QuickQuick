import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import TranslatePage from "./TranslatePage";

// Mock Tauri event API：渲染测试环境无 Tauri 运行时
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock IPC：渲染测试环境无 Tauri 运行时
// getTranslateProviders / getSelectedProvider / setSelectedProvider 需补齐，
// 否则 TranslatePage 挂载时的 provider fetch 会 reject，干扰翻译/历史断言。
vi.mock("../../ipc/ipc-client", () => ({
  translateText: vi.fn(),
  listTranslateHistory: vi.fn(),
  getTranslateProviders: vi.fn(),
  getSelectedProvider: vi.fn(),
  setSelectedProvider: vi.fn(),
  getProviderCredentialSchema: vi.fn(),
  getProviderCredentials: vi.fn(),
}));

// Mock browser-api：隔离 navigator.clipboard / speechSynthesis（jsdom secure-context 限制）
vi.mock("./browser-api", () => ({
  writeToClipboard: vi.fn().mockResolvedValue(undefined),
  speakText: vi.fn(),
}));

import { listen } from "@tauri-apps/api/event";
import { TRANSLATE_HISTORY_CHANGED_EVENT } from "../../ipc/events";
import {
  translateText,
  listTranslateHistory,
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
  getProviderCredentialSchema,
  getProviderCredentials,
  type TranslateResult,
} from "../../ipc/ipc-client";
import { PROVIDER_CONFIG_CHANGED_EVENT } from "../../ipc/events";
import { writeToClipboard, speakText } from "./browser-api";

const mockListen = vi.mocked(listen);
const mockTranslateText = vi.mocked(translateText);
const mockListTranslateHistory = vi.mocked(listTranslateHistory);
const mockGetTranslateProviders = vi.mocked(getTranslateProviders);
const mockGetSelectedProvider = vi.mocked(getSelectedProvider);
const mockSetSelectedProvider = vi.mocked(setSelectedProvider);
const mockGetProviderCredentialSchema = vi.mocked(getProviderCredentialSchema);
const mockGetProviderCredentials = vi.mocked(getProviderCredentials);
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
const MOCK_RESULT: TranslateResult = {
  kind: "plain",
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
      { id: "lingva", name: "Lingva · 默认", needsKey: false, needsConfig: false, isUnofficial: true },
      { id: "baidu", name: "百度翻译", needsKey: true, needsConfig: true, isUnofficial: false },
    ]);
    mockGetSelectedProvider.mockResolvedValue("lingva");
    mockSetSelectedProvider.mockResolvedValue(undefined);
    // 凭据相关 mock 默认返回空（未配置）
    mockGetProviderCredentialSchema.mockResolvedValue([
      { key: "app_id", label: "App ID", isSecret: false, required: true },
      { key: "secret_key", label: "Secret Key", isSecret: true, required: true },
    ]);
    mockGetProviderCredentials.mockResolvedValue([
      { key: "app_id", value: null, isSet: false },
      { key: "secret_key", value: null, isSet: false },
    ]);
    // browser-api mock 的默认实现已在 vi.mock factory 中设置，clearAllMocks 后恢复默认即可
    mockWriteToClipboard.mockResolvedValue(undefined);
  });

  it("translate-page: 默认 source=auto target=zh，点翻译以 (text, 'zh', undefined) 调用", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    // Act：输入文本并点击翻译
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello World");
    const translateBtn = screen.getByRole("button", { name: "翻译" });
    await user.click(translateBtn);

    // Assert：translateText 以 (text, targetLang="zh", source=undefined) 被调用
    await waitFor(() => {
      expect(mockTranslateText).toHaveBeenCalledWith("Hello World", "zh", undefined);
    });
  });

  it("translate-page: 改目标语为 en 后点翻译，translateText 第二参为 'en'", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue({ kind: "plain", translated: "Hello World", sourceLang: "zh", targetLang: "en" });
    const user = userEvent.setup();
    render(<TranslatePage />);

    // 等 provider 加载完成
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "目标语言" })).toBeInTheDocument();
    });

    // Act：切换目标语为英文（自定义 Select：点 trigger 再点选项）
    await user.click(screen.getByRole("button", { name: "目标语言" }));
    await user.click(screen.getByRole("option", { name: "英文" }));

    // 输入并翻译
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "你好");
    await user.click(screen.getByRole("button", { name: "翻译" }));

    // Assert：第二参为 "en"，第三参为 undefined（source 仍是 auto）
    await waitFor(() => {
      expect(mockTranslateText).toHaveBeenCalledWith("你好", "en", undefined);
    });
  });

  it("translate-page: source 选具体语言（如 en）后点翻译，translateText 第三参为 'en'", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    // 等 provider 加载完成
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "源语言" })).toBeInTheDocument();
    });

    // Act：切换源语为英文（自定义 Select：点 trigger 再点选项）
    await user.click(screen.getByRole("button", { name: "源语言" }));
    await user.click(screen.getByRole("option", { name: "英文" }));

    // 输入并翻译
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello");
    await user.click(screen.getByRole("button", { name: "翻译" }));

    // Assert：第三参为 "en"（具体语言，不转为 undefined）
    await waitFor(() => {
      expect(mockTranslateText).toHaveBeenCalledWith("Hello", "zh", "en");
    });
  });

  it("translate-page: source=auto 时 translateText 第三参为 undefined", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    // 等 provider 加载完成（source 默认为 auto，不做切换）
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "源语言" })).toBeInTheDocument();
    });

    // Act：直接输入并翻译（不切换源语，保持默认 auto）
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello");
    await user.click(screen.getByRole("button", { name: "翻译" }));

    // Assert：source=auto → 第三参为 undefined
    await waitFor(() => {
      expect(mockTranslateText).toHaveBeenCalledWith("Hello", "zh", undefined);
    });
  });

  it("translate-page: 输入文本点击翻译后显示译文和语言方向", async () => {
    // Arrange
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    // Act：输入文本并点击翻译
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello World");
    const translateBtn = screen.getByRole("button", { name: "翻译" });
    await user.click(translateBtn);

    // Assert：显示译文文本
    await waitFor(() => {
      expect(screen.getByText("你好世界")).toBeInTheDocument();
    });
    // Assert：译文区方向标识（后端返回的实际方向）
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
    await user.click(screen.getByRole("button", { name: "翻译" }));
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
    await user.click(screen.getByRole("button", { name: "翻译" }));

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
    await user.click(screen.getByRole("button", { name: "翻译" }));
    await waitFor(() => {
      expect(screen.getByText("你好世界")).toBeInTheDocument();
    });

    // 第二次翻译失败
    await user.click(screen.getByRole("button", { name: "翻译" }));

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
    const translateBtn = screen.getByRole("button", { name: "翻译" });
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
    await user.click(screen.getByRole("button", { name: "翻译" }));
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

  it("translate-page: copy 操作 reject 时显示错误提示（role=alert）", async () => {
    // Arrange：writeToClipboard 模拟失败
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    mockWriteToClipboard.mockRejectedValue(new Error("剪贴板不可用"));
    const user = userEvent.setup();
    render(<TranslatePage />);

    // 先执行翻译使译文出现
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello World");
    await user.click(screen.getByRole("button", { name: "翻译" }));
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
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    render(<TranslatePage seed={{ text: "hello", nonce: 1 }} />);

    // Assert：translateText 被以 "hello" 调用（target="zh", source=undefined）
    await waitFor(() => {
      expect(mockTranslateText).toHaveBeenCalledWith("hello", "zh", undefined);
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
    expect(mockTranslateText).toHaveBeenNthCalledWith(2, "hello", "zh", undefined);
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

  it("③: translateText reject(Error('百度翻译签名错误')) → 错误显示真实消息而非固定兜底文案", async () => {
    mockTranslateText.mockRejectedValue(new Error("百度翻译签名错误: invalid sign"));
    const user = userEvent.setup();
    render(<TranslatePage />);

    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello");
    await user.click(screen.getByRole("button", { name: "翻译" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(screen.getByRole("alert").textContent).toContain("百度翻译签名错误");
  });

  it("③: translateText reject 时错误渲染在结果区（.tx-result 内），顶部不再有 .tx-error", async () => {
    mockTranslateText.mockRejectedValue(new Error("百度翻译签名错误: invalid sign"));
    const user = userEvent.setup();
    const { container } = render(<TranslatePage />);

    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello");
    await user.click(screen.getByRole("button", { name: "翻译" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });

    // 顶部 .tx-error 不再存在
    expect(container.querySelector(".tx-error")).toBeNull();
    // 错误渲染在 .tx-scroll 内的结果区（alert 在 .tx-scroll 内）
    const txScroll = container.querySelector(".tx-scroll");
    expect(txScroll).not.toBeNull();
    expect(txScroll!.querySelector("[role='alert']")).not.toBeNull();
  });

  it("①: 挂载时对 needsConfig provider 取凭据，已配置的 provider 在 DirBar 中不 disabled", async () => {
    // baidu 已配置
    mockGetProviderCredentials.mockResolvedValue([
      { key: "app_id", value: "my_id", isSet: true },
      { key: "secret_key", value: null, isSet: true },
    ]);
    const user = userEvent.setup();
    render(<TranslatePage />);

    await waitFor(() => {
      expect(mockGetProviderCredentials).toHaveBeenCalledWith("baidu");
    });

    // 展开翻译源下拉，百度翻译 option 不应标记 aria-disabled
    await user.click(await screen.findByRole("button", { name: "翻译源" }));
    await waitFor(() => {
      expect(screen.getByRole("option", { name: "百度翻译" })).toHaveAttribute("aria-disabled", "false");
    });
  });

  it("①: 挂载时 needsConfig provider 未配置 → DirBar 中该 option disabled", async () => {
    // 默认 beforeEach 返回 isSet=false（未配置）
    const user = userEvent.setup();
    render(<TranslatePage />);

    await waitFor(() => {
      expect(mockGetProviderCredentials).toHaveBeenCalledWith("baidu");
    });

    await user.click(await screen.findByRole("button", { name: "翻译源" }));
    expect(screen.getByRole("option", { name: "百度翻译" })).toHaveAttribute("aria-disabled", "true");
  });

  it("①: 收到 PROVIDER_CONFIG_CHANGED_EVENT → 重新取凭据并解禁已配置的 option", async () => {
    // 捕获所有 listen 调用的回调
    const callbacks = new Map<string, () => void>();
    mockListen.mockImplementation((eventName, handler) => {
      callbacks.set(eventName as string, handler as () => void);
      return Promise.resolve(() => {});
    });

    // 初始未配置
    mockGetProviderCredentials.mockResolvedValue([
      { key: "app_id", value: null, isSet: false },
      { key: "secret_key", value: null, isSet: false },
    ]);

    const user = userEvent.setup();
    render(<TranslatePage />);

    // 展开翻译源下拉：初始 baidu 应 disabled
    await user.click(await screen.findByRole("button", { name: "翻译源" }));
    expect(screen.getByRole("option", { name: "百度翻译" })).toHaveAttribute("aria-disabled", "true");

    // 模拟用户在设置页配好了 key，再次取凭据时已配置
    mockGetProviderCredentials.mockResolvedValue([
      { key: "app_id", value: "my_id", isSet: true },
      { key: "secret_key", value: null, isSet: true },
    ]);

    // 触发 provider-config-changed 事件（下拉保持展开，option 应实时解禁）
    callbacks.get(PROVIDER_CONFIG_CHANGED_EVENT)?.();

    await waitFor(() => {
      expect(screen.getByRole("option", { name: "百度翻译" })).toHaveAttribute("aria-disabled", "false");
    });
  });

  it("translate-page: 收到 translate-history-changed 事件后触发 listTranslateHistory 重新加载", async () => {
    // Arrange：按事件名分别捕获回调，以便后续精确触发目标事件
    const callbacks = new Map<string, () => void>();
    mockListen.mockImplementation((eventName, handler) => {
      callbacks.set(eventName as string, handler as () => void);
      return Promise.resolve(() => {});
    });
    mockListTranslateHistory.mockResolvedValue(MOCK_HISTORY);

    render(<TranslatePage />);

    // 等待挂载时的初始加载完成（listTranslateHistory 第 1 次调用）
    await waitFor(() => {
      expect(mockListTranslateHistory).toHaveBeenCalledTimes(1);
    });
    // 确认已订阅正确的事件名
    expect(mockListen).toHaveBeenCalledWith(TRANSLATE_HISTORY_CHANGED_EVENT, expect.any(Function));

    // Act：模拟后端触发 translate-history-changed 事件
    callbacks.get(TRANSLATE_HISTORY_CHANGED_EVENT)?.();

    // Assert：listTranslateHistory 被再次调用（第 2 次）以刷新历史
    await waitFor(() => {
      expect(mockListTranslateHistory).toHaveBeenCalledTimes(2);
    });
  });

  it("Bug3: 翻译 pending 期间结果区出现 role=status 的「翻译中…」加载态", async () => {
    // Arrange：用可控 pending promise，使翻译停留在加载态不 resolve
    let resolveFn: ((value: typeof MOCK_RESULT) => void) | undefined;
    mockTranslateText.mockReturnValue(
      new Promise((resolve) => {
        resolveFn = resolve;
      })
    );
    const user = userEvent.setup();
    const { container } = render(<TranslatePage />);

    // Act：输入文本并点击翻译（promise 不 resolve，停留在 pending）
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello World");
    await user.click(screen.getByRole("button", { name: "翻译" }));

    // Assert：结果区出现 role=status 的加载指示，含「翻译中…」文案
    await waitFor(() => {
      const status = screen.getByRole("status");
      expect(status).toBeInTheDocument();
      expect(status.textContent).toMatch(/翻译中/);
    });
    // Assert：加载态渲染在结果区（.tx-result）内
    const status = screen.getByRole("status");
    expect(container.querySelector(".tx-result")?.contains(status)).toBe(true);

    // 收尾：resolve 防止未处理的 pending promise 泄漏
    resolveFn?.(MOCK_RESULT);
  });

  it("Bug3 残留: translateText 秒回（立即 resolve）时 loading 态仍可见至少最小时长", async () => {
    // Arrange：translateText 立即 resolve，模拟真机「秒回」（Google 免费接口 / Lingva 命中缓存）。
    // 用 real timers 完成挂载与输入（userEvent 在 fake timers 下会因内部 delay 挂死），
    // 点击前再切到 fake timers，以便精确卡在「最小可见时长」中途断言 loading 仍在。
    mockTranslateText.mockResolvedValue(MOCK_RESULT);
    const user = userEvent.setup();
    render(<TranslatePage />);

    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello World");

    vi.useFakeTimers();
    try {
      // Act：点击翻译（translateText 立即 resolve，无最小时长保障时会瞬间切到译文）
      const translateBtn = screen.getByRole("button", { name: "翻译" });
      translateBtn.click();

      // 让点击产生的微任务链（await translateText / fetchHistory）跑完，
      // 但尚未推进定时器越过最小可见时长。
      await vi.advanceTimersByTimeAsync(0);

      // Assert：loading 态仍可见（role=status「翻译中…」），译文尚未出现。
      // 未实现最小时长时，此处 status 已被清掉、译文已出现 → 断言失败（红）。
      const status = screen.getByRole("status");
      expect(status.textContent).toMatch(/翻译中/);
      expect(screen.queryByText("你好世界")).toBeNull();

      // Act：推进假时钟越过最小可见时长
      await vi.advanceTimersByTimeAsync(400);

      // Assert：loading 消失、译文出现
      expect(screen.getByText("你好世界")).toBeInTheDocument();
      expect(screen.queryByRole("status")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("Bug3: 重复翻译 pending 期间旧译文被加载态盖掉（查不到上次译文）", async () => {
    // Arrange：第一次成功返回「第一次译文」，第二次返回可控 pending promise
    let resolveSecond: ((value: typeof MOCK_RESULT) => void) | undefined;
    mockTranslateText
      .mockResolvedValueOnce({ kind: "plain", translated: "第一次译文", sourceLang: "en", targetLang: "zh" })
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveSecond = resolve;
        })
      );
    const user = userEvent.setup();
    render(<TranslatePage />);

    // 第一次翻译成功，渲染出「第一次译文」
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "Hello");
    await user.click(screen.getByRole("button", { name: "翻译" }));
    await waitFor(() => {
      expect(screen.getByText("第一次译文")).toBeInTheDocument();
    });

    // Act：再次点击翻译（第二次 pending 不 resolve）
    await user.click(screen.getByRole("button", { name: "翻译" }));

    // Assert：旧译文「第一次译文」被加载态盖掉，结果区查不到
    await waitFor(() => {
      expect(screen.getByRole("status")).toBeInTheDocument();
    });
    expect(screen.queryByText("第一次译文")).toBeNull();

    // 收尾：resolve 防止未处理的 pending promise 泄漏
    resolveSecond?.(MOCK_RESULT);
  });
});
