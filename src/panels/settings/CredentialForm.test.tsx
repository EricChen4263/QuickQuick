import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("../../ipc/ipc-client", () => ({
  getProviderCredentials: vi.fn(),
  setProviderCredentials: vi.fn(),
  deleteProviderCredentials: vi.fn(),
}));

import {
  getProviderCredentials,
  setProviderCredentials,
  deleteProviderCredentials,
} from "../../ipc/ipc-client";
import type { CredentialField, CredentialValue } from "../../ipc/ipc-client";
import CredentialForm from "./CredentialForm";

const mockGetProviderCredentials = vi.mocked(getProviderCredentials);
const mockSetProviderCredentials = vi.mocked(setProviderCredentials);
const mockDeleteProviderCredentials = vi.mocked(deleteProviderCredentials);

const BAIDU_SCHEMA: CredentialField[] = [
  { key: "appId", label: "App ID", isSecret: false, required: true },
  { key: "secret", label: "密钥", isSecret: true, required: true },
];

const EMPTY_CREDENTIALS: CredentialValue[] = [
  { key: "appId", value: null, isSet: false },
  { key: "secret", value: null, isSet: false },
];

const SET_CREDENTIALS: CredentialValue[] = [
  { key: "appId", value: "my-app-id", isSet: true },
  { key: "secret", value: null, isSet: true },
];

beforeEach(() => {
  vi.clearAllMocks();
  mockGetProviderCredentials.mockResolvedValue(EMPTY_CREDENTIALS);
  mockSetProviderCredentials.mockResolvedValue(undefined);
  mockDeleteProviderCredentials.mockResolvedValue(undefined);
  vi.spyOn(window, "confirm").mockReturnValue(true);
});

describe("CredentialForm", () => {
  it("按 schema 渲染 baidu：AppID text 输入框 + 密钥 password 输入框", async () => {
    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByLabelText("App ID")).toBeInTheDocument();
    });

    const appIdInput = screen.getByLabelText("App ID");
    const secretInput = screen.getByLabelText("密钥");

    expect(appIdInput).toHaveAttribute("type", "text");
    expect(secretInput).toHaveAttribute("type", "password");
  });

  it("已设置的 secret 字段 placeholder 含「已设置」", async () => {
    mockGetProviderCredentials.mockResolvedValue(SET_CREDENTIALS);

    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
      />
    );

    await waitFor(() => {
      const secretInput = screen.getByLabelText("密钥");
      expect(secretInput).toHaveAttribute("placeholder", expect.stringContaining("已设置"));
    });
  });

  it("已存 value 的非 secret 字段回填到输入框", async () => {
    mockGetProviderCredentials.mockResolvedValue(SET_CREDENTIALS);

    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
      />
    );

    await waitFor(() => {
      const appIdInput = screen.getByLabelText("App ID") as HTMLInputElement;
      expect(appIdInput.value).toBe("my-app-id");
    });
  });

  it("保存时空串 secret 字段不传给 setProviderCredentials", async () => {
    const user = userEvent.setup();
    const onSaved = vi.fn();

    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={onSaved}
      />
    );

    await waitFor(() => {
      expect(screen.getByLabelText("App ID")).toBeInTheDocument();
    });

    await user.type(screen.getByLabelText("App ID"), "test-app-id");

    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(mockSetProviderCredentials).toHaveBeenCalledWith(
        "baidu",
        { appId: "test-app-id" }
      );
    });
    expect(onSaved).toHaveBeenCalledTimes(1);
  });

  it("input 带 set-input 类、保存按钮带 btn 类（暗色适配防回归）", async () => {
    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByLabelText("App ID")).toBeInTheDocument();
    });

    const appIdInput = screen.getByLabelText("App ID");
    const secretInput = screen.getByLabelText("密钥");
    const saveBtn = screen.getByRole("button", { name: "保存" });

    expect(appIdInput).toHaveClass("set-input");
    expect(secretInput).toHaveClass("set-input");
    expect(saveBtn).toHaveClass("btn");
  });

  it("保存失败时显示错误提示（role=alert）", async () => {
    const user = userEvent.setup();
    mockSetProviderCredentials.mockRejectedValue(new Error("保存失败"));

    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByLabelText("App ID")).toBeInTheDocument();
    });

    await user.type(screen.getByLabelText("App ID"), "test-id");
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
  });
});

describe("CredentialForm - isConfigured prop", () => {
  it("isConfigured=true 时显示清除按钮", async () => {
    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
        isConfigured={true}
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /清除/ })).toBeInTheDocument();
    });
  });

  it("isConfigured=true 时显示已配置提示文字", async () => {
    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
        isConfigured={true}
      />
    );

    await waitFor(() => {
      expect(screen.getByText(/已配置/)).toBeInTheDocument();
    });
  });

  it("isConfigured=false 时不显示清除按钮", async () => {
    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
        isConfigured={false}
      />
    );

    await waitFor(() => {
      expect(screen.getByLabelText("App ID")).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: /清除/ })).not.toBeInTheDocument();
  });

  it("isConfigured 默认值（undefined）不显示清除按钮", async () => {
    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByLabelText("App ID")).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: /清除/ })).not.toBeInTheDocument();
  });

  it("点击清除并确认后调 deleteProviderCredentials 且调 onSaved", async () => {
    const user = userEvent.setup();
    const onSaved = vi.fn();

    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={onSaved}
        isConfigured={true}
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /清除/ })).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /清除/ }));

    await waitFor(() => {
      expect(mockDeleteProviderCredentials).toHaveBeenCalledWith("baidu");
      expect(onSaved).toHaveBeenCalledTimes(1);
    });
  });

  it("点击清除但取消确认时不调 deleteProviderCredentials", async () => {
    const user = userEvent.setup();
    vi.spyOn(window, "confirm").mockReturnValue(false);

    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
        isConfigured={true}
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /清除/ })).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /清除/ }));

    expect(mockDeleteProviderCredentials).not.toHaveBeenCalled();
  });

  it("清除失败时显示错误提示（role=alert）", async () => {
    const user = userEvent.setup();
    mockDeleteProviderCredentials.mockRejectedValue(new Error("清除失败"));

    render(
      <CredentialForm
        providerId="baidu"
        schema={BAIDU_SCHEMA}
        onSaved={vi.fn()}
        isConfigured={true}
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /清除/ })).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /清除/ }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
  });
});
