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
  downloadAndInstallUpdate: vi.fn().mockResolvedValue(undefined),
}));

import {
  getLaunchOnLogin,
  getStayInTray,
  getAutoUpdate,
  checkForUpdates,
  downloadAndInstallUpdate,
} from "../../ipc/ipc-client";
import GeneralPanel from "./GeneralPanel";

const mockCheckForUpdates = vi.mocked(checkForUpdates);
const mockDownloadAndInstall = vi.mocked(downloadAndInstallUpdate);

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getLaunchOnLogin).mockResolvedValue(true);
  vi.mocked(getStayInTray).mockResolvedValue(true);
  vi.mocked(getAutoUpdate).mockResolvedValue(true);
  mockDownloadAndInstall.mockResolvedValue(undefined);
});

describe("GeneralPanel 手动检查后下载安装", () => {
  it("general_panel_offers_install_after_update_found", async () => {
    mockCheckForUpdates.mockResolvedValueOnce({
      available: true,
      version: "1.3.0",
      currentVersion: "1.0.0",
    });

    const user = userEvent.setup();
    render(<GeneralPanel />);

    await user.click(await screen.findByRole("button", { name: "检查" }));

    const installBtn = await screen.findByRole("button", {
      name: "下载并安装",
    });
    expect(installBtn).toBeInTheDocument();
    expect(screen.getByText(/发现新版本/)).toBeInTheDocument();
    expect(screen.getByText(/1\.3\.0/)).toBeInTheDocument();

    await user.click(installBtn);

    await waitFor(() => {
      expect(mockDownloadAndInstall).toHaveBeenCalledTimes(1);
    });
  });

  it("available=false 时不出现「下载并安装」按钮", async () => {
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
    expect(
      screen.queryByRole("button", { name: "下载并安装" })
    ).not.toBeInTheDocument();
  });

  it("下载安装失败时渲染 role=alert 失败文案", async () => {
    mockCheckForUpdates.mockResolvedValueOnce({
      available: true,
      version: "1.3.0",
      currentVersion: "1.0.0",
    });
    mockDownloadAndInstall.mockRejectedValueOnce(new Error("下载失败：网络错误"));

    const user = userEvent.setup();
    render(<GeneralPanel />);

    await user.click(await screen.findByRole("button", { name: "检查" }));
    await user.click(
      await screen.findByRole("button", { name: "下载并安装" })
    );

    await waitFor(() => {
      const alert = screen.getByRole("alert");
      expect(alert).toHaveTextContent(/下载.*失败|安装失败/);
    });
  });
});
