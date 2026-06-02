import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("../../ipc/ipc-client", () => ({
  getLaunchOnLogin: vi.fn().mockResolvedValue(true),
  setLaunchOnLogin: vi.fn().mockResolvedValue(undefined),
  getStayInTray: vi.fn().mockResolvedValue(true),
  setStayInTray: vi.fn().mockResolvedValue(undefined),
  getAutoUpdate: vi.fn().mockResolvedValue(true),
  setAutoUpdate: vi.fn().mockResolvedValue(undefined),
  checkForUpdates: vi.fn(),
}));

import {
  getLaunchOnLogin,
  getStayInTray,
  getAutoUpdate,
  checkForUpdates,
} from "../../ipc/ipc-client";
import GeneralPanel from "./GeneralPanel";

const mockCheckForUpdates = vi.mocked(checkForUpdates);

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getLaunchOnLogin).mockResolvedValue(true);
  vi.mocked(getStayInTray).mockResolvedValue(true);
  vi.mocked(getAutoUpdate).mockResolvedValue(true);
});

describe("GeneralPanel 立即检查更新按钮", () => {
  it("渲染「检查」按钮且初始未 disabled", async () => {
    render(<GeneralPanel />);

    const btn = await screen.findByRole("button", { name: "检查" });
    expect(btn).toBeInTheDocument();
    expect(btn).not.toBeDisabled();
  });

  it("available=true 时显示「发现新版本」文案", async () => {
    mockCheckForUpdates.mockResolvedValueOnce({
      available: true,
      version: "1.2.0",
      currentVersion: "1.0.0",
    });

    const user = userEvent.setup();
    render(<GeneralPanel />);

    await user.click(await screen.findByRole("button", { name: "检查" }));

    await waitFor(() => {
      expect(screen.getByText(/发现新版本/)).toBeInTheDocument();
      expect(screen.getByText(/1\.2\.0/)).toBeInTheDocument();
    });
  });

  it("available=false 时显示「已是最新版本」文案", async () => {
    mockCheckForUpdates.mockResolvedValueOnce({
      available: false,
      version: "",
      currentVersion: "1.0.0",
    });

    const user = userEvent.setup();
    render(<GeneralPanel />);

    await user.click(await screen.findByRole("button", { name: "检查" }));

    await waitFor(() => {
      expect(screen.getByText("已是最新版本")).toBeInTheDocument();
    });
  });

  it("checkForUpdates reject 时渲染 role=alert 失败文案", async () => {
    mockCheckForUpdates.mockRejectedValueOnce(
      new Error("检查更新失败：网络错误")
    );

    const user = userEvent.setup();
    render(<GeneralPanel />);

    await user.click(await screen.findByRole("button", { name: "检查" }));

    await waitFor(() => {
      const alert = screen.getByRole("alert");
      expect(alert).toBeInTheDocument();
      expect(alert).toHaveTextContent(/检查更新失败/);
    });
  });

  it("点击中按钮变为 disabled 且文案为「检查中…」", async () => {
    let resolveCheck!: (v: {
      available: boolean;
      version: string;
      currentVersion: string;
    }) => void;
    mockCheckForUpdates.mockReturnValueOnce(
      new Promise((res) => {
        resolveCheck = res;
      })
    );

    const user = userEvent.setup();
    render(<GeneralPanel />);

    await user.click(await screen.findByRole("button", { name: "检查" }));

    expect(
      await screen.findByRole("button", { name: "检查中…" })
    ).toBeDisabled();

    resolveCheck({ available: false, version: "", currentVersion: "1.0.0" });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "检查" })).not.toBeDisabled();
    });
  });
});
