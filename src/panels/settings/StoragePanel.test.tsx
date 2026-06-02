import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import StoragePanel from "./StoragePanel";

vi.mock("../../ipc/ipc-client", () => ({
  getStorageStats: vi.fn(),
  cleanupHistory: vi.fn(),
  getImageThreshold: vi.fn(),
  setImageThreshold: vi.fn(),
}));

import {
  getStorageStats,
  cleanupHistory,
  getImageThreshold,
  setImageThreshold,
} from "../../ipc/ipc-client";

const mockGetStorageStats = vi.mocked(getStorageStats);
const mockCleanupHistory = vi.mocked(cleanupHistory);
const mockGetImageThreshold = vi.mocked(getImageThreshold);
const mockSetImageThreshold = vi.mocked(setImageThreshold);

describe("StoragePanel 单张图片阈值", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetStorageStats.mockResolvedValue({ liveCount: 10, fileSizeBytes: 1024 * 1024 });
    mockCleanupHistory.mockResolvedValue({ softDeleted: 0, purged: 0 });
    mockSetImageThreshold.mockResolvedValue(undefined);
  });

  it("挂载后调用 getImageThreshold，select 值显示对应 MB（返回 10MiB → 选中 10）", async () => {
    mockGetImageThreshold.mockResolvedValue(10 * 1024 * 1024);

    render(<StoragePanel />);

    await waitFor(() => {
      expect(mockGetImageThreshold).toHaveBeenCalledTimes(1);
    });

    const select = screen.getByRole("combobox", { name: /单张图片阈值/ });
    expect(select).toHaveValue("10");
  });

  it("返回默认 20MiB 时，select 选中 20", async () => {
    mockGetImageThreshold.mockResolvedValue(20 * 1024 * 1024);

    render(<StoragePanel />);

    await waitFor(() => {
      const select = screen.getByRole("combobox", { name: /单张图片阈值/ });
      expect(select).toHaveValue("20");
    });
  });

  it("getImageThreshold 失败时退回默认值 20，select 选中 20", async () => {
    mockGetImageThreshold.mockRejectedValue(new Error("IPC error"));

    render(<StoragePanel />);

    await waitFor(() => {
      const select = screen.getByRole("combobox", { name: /单张图片阈值/ });
      expect(select).toHaveValue("20");
    });
  });

  it("改 select 到 50MB 时，以正确字节数调用 setImageThreshold（52428800）", async () => {
    mockGetImageThreshold.mockResolvedValue(20 * 1024 * 1024);
    const user = userEvent.setup();

    render(<StoragePanel />);

    await waitFor(() => {
      expect(screen.getByRole("combobox", { name: /单张图片阈值/ })).toHaveValue("20");
    });

    const select = screen.getByRole("combobox", { name: /单张图片阈值/ });
    await user.selectOptions(select, "50");

    await waitFor(() => {
      expect(mockSetImageThreshold).toHaveBeenCalledWith(52428800);
    });

    expect(select).toHaveValue("50");
  });

  it("select 包含预设档位 5/10/20/50/100 MB", async () => {
    mockGetImageThreshold.mockResolvedValue(20 * 1024 * 1024);

    render(<StoragePanel />);

    await waitFor(() => {
      const select = screen.getByRole("combobox", { name: /单张图片阈值/ });
      const options = Array.from(select.querySelectorAll("option")).map((o) => o.value);
      expect(options).toEqual(["5", "10", "20", "50", "100"]);
    });
  });

  it("desc 文案不含「静态展示，无对应 IPC」", async () => {
    mockGetImageThreshold.mockResolvedValue(20 * 1024 * 1024);

    render(<StoragePanel />);

    await waitFor(() => {
      expect(screen.queryByText(/静态展示，无对应 IPC/)).not.toBeInTheDocument();
    });
  });
});
