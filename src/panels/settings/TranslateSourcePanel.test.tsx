import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("../../ipc/ipc-client", () => ({
  getTranslateProviders: vi.fn(),
  getSelectedProvider: vi.fn(),
  setSelectedProvider: vi.fn(),
  getProviderCredentialSchema: vi.fn(),
  getProviderCredentials: vi.fn(),
  setProviderCredentials: vi.fn(),
  deleteProviderCredentials: vi.fn(),
}));

import {
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
  getProviderCredentialSchema,
  getProviderCredentials,
  setProviderCredentials,
  deleteProviderCredentials,
} from "../../ipc/ipc-client";
import type { Provider, CredentialField, CredentialValue } from "../../ipc/ipc-client";
import TranslateSourcePanel from "./TranslateSourcePanel";

const mockGetTranslateProviders = vi.mocked(getTranslateProviders);
const mockGetSelectedProvider = vi.mocked(getSelectedProvider);
const mockSetSelectedProvider = vi.mocked(setSelectedProvider);
const mockGetProviderCredentialSchema = vi.mocked(getProviderCredentialSchema);
const mockGetProviderCredentials = vi.mocked(getProviderCredentials);
const mockSetProviderCredentials = vi.mocked(setProviderCredentials);

const PROVIDERS: Provider[] = [
  { id: "mymemory", name: "MyMemory", needsKey: false },
  { id: "baidu", name: "百度翻译", needsKey: true },
];

const BAIDU_SCHEMA: CredentialField[] = [
  { key: "appId", label: "App ID", isSecret: false, required: true },
  { key: "secret", label: "密钥", isSecret: true, required: true },
];

const BAIDU_CREDENTIALS_EMPTY: CredentialValue[] = [
  { key: "appId", value: null, isSet: false },
  { key: "secret", value: null, isSet: false },
];

const BAIDU_CREDENTIALS_SET: CredentialValue[] = [
  { key: "appId", value: "my-app-id", isSet: true },
  { key: "secret", value: null, isSet: true },
];

beforeEach(() => {
  vi.clearAllMocks();
  mockGetTranslateProviders.mockResolvedValue(PROVIDERS);
  mockGetSelectedProvider.mockResolvedValue("mymemory");
  mockSetSelectedProvider.mockResolvedValue(undefined);
  mockGetProviderCredentialSchema.mockResolvedValue(BAIDU_SCHEMA);
  mockGetProviderCredentials.mockResolvedValue(BAIDU_CREDENTIALS_EMPTY);
  mockSetProviderCredentials.mockResolvedValue(undefined);
});

describe("TranslateSourcePanel 徽标显示", () => {
  it("未配置的 needsKey provider 显示「待配置」徽标", async () => {
    mockGetProviderCredentials.mockResolvedValue(BAIDU_CREDENTIALS_EMPTY);

    render(<TranslateSourcePanel />);

    await waitFor(() => {
      const badges = screen.getAllByText("待配置");
      expect(badges.length).toBeGreaterThan(0);
    });
  });

  it("已配置的 needsKey provider 显示「已配置」徽标", async () => {
    mockGetProviderCredentials.mockResolvedValue(BAIDU_CREDENTIALS_SET);

    render(<TranslateSourcePanel />);

    await waitFor(() => {
      expect(screen.getByText("已配置")).toBeInTheDocument();
    });
  });
});

describe("TranslateSourcePanel 设为默认（解耦）", () => {
  it("点击左侧「设默认」热区（radio）→ 调用 setSelectedProvider，且不展开 CredentialForm", async () => {
    const user = userEvent.setup();

    render(<TranslateSourcePanel />);

    await waitFor(() => {
      expect(screen.getByText("百度翻译")).toBeInTheDocument();
    });

    const radio = screen.getByRole("radio", { name: "百度翻译" });
    await user.click(radio);

    await waitFor(() => {
      expect(mockSetSelectedProvider).toHaveBeenCalledWith("baidu");
    });

    expect(screen.queryByLabelText("App ID")).not.toBeInTheDocument();
  });

  it("点击左侧热区（.src-select）→ 调用 setSelectedProvider，不展开表单", async () => {
    const user = userEvent.setup();

    render(<TranslateSourcePanel />);

    await waitFor(() => {
      expect(screen.getByText("百度翻译")).toBeInTheDocument();
    });

    const selectZone = screen.getByText("百度翻译").closest(".src-select");
    expect(selectZone).not.toBeNull();
    await user.click(selectZone!);

    await waitFor(() => {
      expect(mockSetSelectedProvider).toHaveBeenCalledWith("baidu");
    });

    expect(screen.queryByLabelText("App ID")).not.toBeInTheDocument();
  });

  it("点击 mymemory radio → 调用 setSelectedProvider，不展开表单", async () => {
    const user = userEvent.setup();
    mockGetSelectedProvider.mockResolvedValue("baidu");
    mockGetProviderCredentials.mockResolvedValue(BAIDU_CREDENTIALS_SET);

    render(<TranslateSourcePanel />);

    await waitFor(() => {
      expect(screen.getByText("MyMemory")).toBeInTheDocument();
    });

    const radio = screen.getByRole("radio", { name: "MyMemory" });
    await user.click(radio);

    await waitFor(() => {
      expect(mockSetSelectedProvider).toHaveBeenCalledWith("mymemory");
    });

    expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
  });
});

describe("TranslateSourcePanel 配置按钮（解耦）", () => {
  it("点击「配置」按钮 → 展开 CredentialForm（App ID 输入出现），且不调用 setSelectedProvider", async () => {
    const user = userEvent.setup();

    render(<TranslateSourcePanel />);

    await waitFor(() => {
      expect(screen.getByText("百度翻译")).toBeInTheDocument();
    });

    const cfgBtn = screen.getByRole("button", { name: /配置/ });
    await user.click(cfgBtn);

    await waitFor(() => {
      expect(screen.getByLabelText("App ID")).toBeInTheDocument();
    });

    expect(mockSetSelectedProvider).not.toHaveBeenCalled();
  });

  it("再次点击「配置」按钮 → 收起表单（toggle）", async () => {
    const user = userEvent.setup();

    render(<TranslateSourcePanel />);

    await waitFor(() => {
      expect(screen.getByText("百度翻译")).toBeInTheDocument();
    });

    const cfgBtn = screen.getByRole("button", { name: /配置/ });

    await user.click(cfgBtn);
    await waitFor(() => {
      expect(screen.getByLabelText("App ID")).toBeInTheDocument();
    });

    await user.click(cfgBtn);
    await waitFor(() => {
      expect(screen.queryByLabelText("App ID")).not.toBeInTheDocument();
    });
  });

  it("无需 Key 的 MyMemory 不渲染配置按钮", async () => {
    render(<TranslateSourcePanel />);

    await waitFor(() => {
      expect(screen.getByText("MyMemory")).toBeInTheDocument();
    });

    const cfgBtns = screen.queryAllByRole("button", { name: /配置/ });
    expect(cfgBtns).toHaveLength(1);

    const myMemoryCard = screen.getByText("MyMemory").closest(".src-card");
    expect(myMemoryCard).not.toBeNull();
    const myMemoryCfgBtn = myMemoryCard!.querySelector(".src-cfg-btn");
    expect(myMemoryCfgBtn).toBeNull();
  });

  it("配置按钮展开时有 open 类", async () => {
    const user = userEvent.setup();

    render(<TranslateSourcePanel />);

    await waitFor(() => {
      expect(screen.getByText("百度翻译")).toBeInTheDocument();
    });

    const cfgBtn = screen.getByRole("button", { name: /配置/ });
    expect(cfgBtn.classList.contains("open")).toBe(false);

    await user.click(cfgBtn);
    await waitFor(() => {
      expect(cfgBtn.classList.contains("open")).toBe(true);
    });
  });
});

describe("TranslateSourcePanel 复核失败回退", () => {
  it("清除后复核 getProviderCredentials reject → 该 provider 从已配置回退为待配置", async () => {
    const user = userEvent.setup();
    const mockDelete = vi.mocked(deleteProviderCredentials);
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    // 初始加载 + CredentialForm 自身加载：已配置态（徽标「已配置」）
    mockGetProviderCredentials.mockResolvedValue(BAIDU_CREDENTIALS_SET);
    mockDelete.mockResolvedValue(undefined);

    render(<TranslateSourcePanel />);

    await waitFor(() => {
      expect(screen.getByText("已配置")).toBeInTheDocument();
    });

    // 展开配置表单
    await user.click(screen.getByRole("button", { name: /配置/ }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /清除/ })).toBeInTheDocument();
    });

    // 清除后 handleCredentialSaved 复核请求 reject
    mockGetProviderCredentials.mockRejectedValue(new Error("复核请求失败"));
    await user.click(screen.getByRole("button", { name: /清除/ }));

    // 复核失败保守回退：徽标变「待配置」，「已配置」消失
    await waitFor(() => {
      expect(screen.getByText("待配置")).toBeInTheDocument();
    });
    expect(screen.queryByText("已配置")).not.toBeInTheDocument();
    expect(errorSpy).toHaveBeenCalled();

    confirmSpy.mockRestore();
    errorSpy.mockRestore();
  });
});
