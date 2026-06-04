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

  it("挂载后调用 getImageThreshold，trigger 显示对应 MB（返回 10MiB → 10 MB）", async () => {
    mockGetImageThreshold.mockResolvedValue(10 * 1024 * 1024);

    render(<StoragePanel />);

    await waitFor(() => {
      expect(mockGetImageThreshold).toHaveBeenCalledTimes(1);
    });

    expect(screen.getByRole("button", { name: /单张图片阈值/ })).toHaveTextContent("10 MB");
  });

  it("返回默认 20MiB 时，trigger 显示 20 MB", async () => {
    mockGetImageThreshold.mockResolvedValue(20 * 1024 * 1024);

    render(<StoragePanel />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /单张图片阈值/ })).toHaveTextContent("20 MB");
    });
  });

  it("getImageThreshold 失败时退回默认值 20，trigger 显示 20 MB", async () => {
    mockGetImageThreshold.mockRejectedValue(new Error("IPC error"));

    render(<StoragePanel />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /单张图片阈值/ })).toHaveTextContent("20 MB");
    });
  });

  it("选 50MB 时，以正确字节数调用 setImageThreshold（52428800）", async () => {
    mockGetImageThreshold.mockResolvedValue(20 * 1024 * 1024);
    const user = userEvent.setup();

    render(<StoragePanel />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /单张图片阈值/ })).toHaveTextContent("20 MB");
    });

    await user.click(screen.getByRole("button", { name: /单张图片阈值/ }));
    await user.click(screen.getByRole("option", { name: "50 MB" }));

    await waitFor(() => {
      expect(mockSetImageThreshold).toHaveBeenCalledWith(52428800);
    });

    expect(screen.getByRole("button", { name: /单张图片阈值/ })).toHaveTextContent("50 MB");
  });

  it("下拉含预设档位 5/10/20/50/100 MB", async () => {
    mockGetImageThreshold.mockResolvedValue(20 * 1024 * 1024);
    const user = userEvent.setup();

    render(<StoragePanel />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /单张图片阈值/ })).toHaveTextContent("20 MB");
    });

    await user.click(screen.getByRole("button", { name: /单张图片阈值/ }));
    const labels = screen.getAllByRole("option").map((o) => o.textContent);
    expect(labels).toEqual(["5 MB", "10 MB", "20 MB", "50 MB", "100 MB"]);
  });

  it("desc 文案不含「静态展示，无对应 IPC」", async () => {
    mockGetImageThreshold.mockResolvedValue(20 * 1024 * 1024);

    render(<StoragePanel />);

    await waitFor(() => {
      expect(screen.queryByText(/静态展示，无对应 IPC/)).not.toBeInTheDocument();
    });
  });
});
