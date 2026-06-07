import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import DirBar from "./DirBar";
import TranslatePage from "./TranslatePage";
import type { Provider } from "../../ipc/ipc-client";

// Mock Tauri event API：渲染测试环境无 Tauri 运行时
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock IPC：TranslatePage 挂载时会 fetch provider/历史/凭据，均需打桩
vi.mock("../../ipc/ipc-client", () => ({
  translateText: vi.fn(),
  listTranslateHistory: vi.fn(),
  getTranslateProviders: vi.fn(),
  getSelectedProvider: vi.fn(),
  setSelectedProvider: vi.fn(),
  getProviderCredentialSchema: vi.fn(),
  getProviderCredentials: vi.fn(),
}));

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
  getProviderCredentialSchema,
  getProviderCredentials,
} from "../../ipc/ipc-client";

const mockTranslateText = vi.mocked(translateText);
const mockListTranslateHistory = vi.mocked(listTranslateHistory);
const mockGetTranslateProviders = vi.mocked(getTranslateProviders);
const mockGetSelectedProvider = vi.mocked(getSelectedProvider);
const mockSetSelectedProvider = vi.mocked(setSelectedProvider);
const mockGetProviderCredentialSchema = vi.mocked(getProviderCredentialSchema);
const mockGetProviderCredentials = vi.mocked(getProviderCredentials);

/** 非官方免 key 源（lingva）与官方 keyed 源（baidu）混合列表 */
const MIXED_PROVIDERS: Provider[] = [
  { id: "lingva", name: "Lingva", needsKey: false, needsConfig: false, isUnofficial: true },
  { id: "baidu", name: "百度翻译", needsKey: true, needsConfig: true, isUnofficial: false },
];

describe("nonofficial_source_label_and_degrade_hint", () => {
  it("nonofficial_source_label_and_degrade_hint: isUnofficial 源在选择器 option 显示非官方标注、官方源不显示", async () => {
    // Arrange：lingva(非官方) + baidu(官方)，baidu 已配置以便可见
    const user = userEvent.setup();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MIXED_PROVIDERS}
        selectedProviderId="lingva"
        configuredIds={new Set(["baidu"])}
        onProviderChange={vi.fn()}
        onSourceChange={vi.fn()}
        onTargetChange={vi.fn()}
      />
    );

    // Act：展开翻译源下拉
    await user.click(screen.getByRole("button", { name: "翻译源" }));

    // Assert：非官方源 option 含「非官方」标注；官方源 option 不含
    const lingvaOption = screen.getByRole("option", { name: /Lingva/ });
    const baiduOption = screen.getByRole("option", { name: /百度翻译/ });
    expect(lingvaOption).toHaveTextContent("非官方");
    expect(baiduOption).not.toHaveTextContent("非官方");
  });

  it("nonofficial_source_label_and_degrade_hint: 当前源 isUnofficial 时翻译失败追加降级提示，官方源不追加", async () => {
    // Arrange：选中 lingva（非官方），翻译将失败
    const user = userEvent.setup();
    mockListTranslateHistory.mockResolvedValue([]);
    mockGetTranslateProviders.mockResolvedValue(MIXED_PROVIDERS);
    mockGetSelectedProvider.mockResolvedValue("lingva");
    mockSetSelectedProvider.mockResolvedValue(undefined);
    mockGetProviderCredentialSchema.mockResolvedValue([]);
    mockGetProviderCredentials.mockResolvedValue([]);
    mockTranslateText.mockRejectedValue(new Error("接口失效"));

    render(<TranslatePage />);

    // 等 provider 加载完成（selectedProviderId=lingva 生效）
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "翻译源" })).toHaveTextContent("Lingva");
    });

    // Act：输入并翻译（失败）
    await user.type(screen.getByRole("textbox"), "hello");
    await user.click(screen.getByRole("button", { name: "翻译" }));

    // Assert：错误区出现可区分的降级提示（非官方接口失效引导切源）
    await waitFor(() => {
      const alert = screen.getByRole("alert");
      expect(alert).toHaveTextContent("非官方");
      expect(alert).toHaveTextContent("切换");
    });
  });

  it("nonofficial_source_label_and_degrade_hint: 当前源为官方时翻译失败不追加非官方降级提示", async () => {
    // Arrange：选中 baidu（官方），翻译失败
    const user = userEvent.setup();
    mockListTranslateHistory.mockResolvedValue([]);
    mockGetTranslateProviders.mockResolvedValue(MIXED_PROVIDERS);
    mockGetSelectedProvider.mockResolvedValue("baidu");
    mockSetSelectedProvider.mockResolvedValue(undefined);
    mockGetProviderCredentialSchema.mockResolvedValue([]);
    mockGetProviderCredentials.mockResolvedValue([]);
    mockTranslateText.mockRejectedValue(new Error("百度认证失败"));

    render(<TranslatePage />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "翻译源" })).toHaveTextContent("百度翻译");
    });

    // Act：输入并翻译（失败）
    await user.type(screen.getByRole("textbox"), "hello");
    await user.click(screen.getByRole("button", { name: "翻译" }));

    // Assert：错误显示原始错误，但不含非官方降级提示
    await waitFor(() => {
      const alert = screen.getByRole("alert");
      expect(alert).toHaveTextContent("百度认证失败");
    });
    expect(screen.getByRole("alert")).not.toHaveTextContent("非官方");
  });
});
