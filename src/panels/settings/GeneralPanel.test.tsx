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
  setLaunchOnLogin,
} from "../../ipc/ipc-client";
import GeneralPanel from "./GeneralPanel";
import { SettingsSaveErrorBanner, SettingsSaveErrorProvider } from "./settings-save-error";

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

describe("GeneralPanel 开关保存失败时把后端原文显示出来", () => {
  it("settings.json 拒写时渲染后端中文错误（含修复指引），开关不假装已生效", async () => {
    const backendMessage =
      "settings.json 无法解析（JSON 语法错误），已拒绝写入以免覆盖你的设置；请先修复该文件。备份副本：settings.json.1.bak";
    vi.mocked(setLaunchOnLogin).mockRejectedValueOnce(new Error(backendMessage));

    const user = userEvent.setup();
    render(
      <SettingsSaveErrorProvider>
        <GeneralPanel />
        <SettingsSaveErrorBanner />
      </SettingsSaveErrorProvider>,
    );
    const toggle = await screen.findByRole("switch", { name: "开机自启动" });
    expect(toggle).toHaveAttribute("aria-checked", "true");

    await user.click(toggle);

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent("已拒绝写入");
    });
    expect(screen.getByRole("alert")).toHaveTextContent("settings.json.1.bak");
    expect(screen.getByRole("switch", { name: "开机自启动" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });
});
