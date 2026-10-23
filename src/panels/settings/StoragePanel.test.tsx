import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import StoragePanel from "./StoragePanel";

vi.mock("../../ipc/ipc-client", () => ({
  getStorageStats: vi.fn(),
  cleanupHistory: vi.fn(),
  getImageThreshold: vi.fn(),
  setImageThreshold: vi.fn(),
  exportBackup: vi.fn(),
  importBackup: vi.fn(),
  revealBackupInFinder: vi.fn(),
}));

import {
  getStorageStats,
  cleanupHistory,
  getImageThreshold,
  setImageThreshold,
  exportBackup,
  importBackup,
  revealBackupInFinder,
} from "../../ipc/ipc-client";

const mockGetStorageStats = vi.mocked(getStorageStats);
const mockCleanupHistory = vi.mocked(cleanupHistory);
const mockGetImageThreshold = vi.mocked(getImageThreshold);
const mockSetImageThreshold = vi.mocked(setImageThreshold);
const mockExportBackup = vi.mocked(exportBackup);
const mockImportBackup = vi.mocked(importBackup);
const mockRevealBackupInFinder = vi.mocked(revealBackupInFinder);

/** 测试用导出结果：路径取真实备份包命名格式，便于断言展示文案 */
const BACKUP_PATH = "/Users/tester/Documents/QuickQuick-Backup-20231114-221320.qqbp";

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

describe("StoragePanel 备份 / 恢复", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetStorageStats.mockResolvedValue({ liveCount: 3, fileSizeBytes: 4096 });
    mockGetImageThreshold.mockResolvedValue(20 * 1024 * 1024);
    mockExportBackup.mockResolvedValue({
      path: BACKUP_PATH,
      itemCount: 3,
      skippedCount: 0,
      usedFallbackDir: false,
    });
    mockImportBackup.mockResolvedValue({ inserted: 2, skipped: 1 });
    mockRevealBackupInFinder.mockResolvedValue(undefined);
  });

  it("口令输入框是 password 类型（口令不明文显示）", () => {
    render(<StoragePanel />);

    expect(screen.getByLabelText("备份口令")).toHaveAttribute("type", "password");
  });

  it("导出成功时把口令交给后端、展示落盘路径，并可「在 Finder 中显示」", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<StoragePanel />);

    // Act
    await user.type(screen.getByLabelText("备份口令"), "backup-passphrase");
    await user.click(screen.getByRole("button", { name: "导出备份" }));

    // Assert: 口令按原样传给后端，且返回路径与条数被展示
    await waitFor(() => {
      expect(mockExportBackup).toHaveBeenCalledWith("backup-passphrase");
    });
    expect(await screen.findByText(BACKUP_PATH)).toBeInTheDocument();
    expect(screen.getByText(/已导出 3 条/)).toBeInTheDocument();

    // Act: 在 Finder 中显示
    await user.click(screen.getByRole("button", { name: "在 Finder 中显示" }));

    // Assert
    await waitFor(() => {
      expect(mockRevealBackupInFinder).toHaveBeenCalledWith(BACKUP_PATH);
    });
  });

  it("后端标记走了回退目录时，路径下方显示回退说明", async () => {
    // Arrange
    mockExportBackup.mockResolvedValue({
      path: BACKUP_PATH,
      itemCount: 3,
      skippedCount: 0,
      usedFallbackDir: true,
    });
    const user = userEvent.setup();
    render(<StoragePanel />);

    // Act
    await user.click(screen.getByRole("button", { name: "导出备份" }));

    // Assert
    expect(
      await screen.findByText("已回退到应用数据目录（文稿/下载目录不可用）")
    ).toBeInTheDocument();
  });

  it("未回退时不显示回退说明（不让用户看到假的降级提示）", async () => {
    // Arrange: 默认 mock 的 usedFallbackDir 为 false
    const user = userEvent.setup();
    render(<StoragePanel />);

    // Act
    await user.click(screen.getByRole("button", { name: "导出备份" }));
    await screen.findByText(BACKUP_PATH);

    // Assert
    expect(
      screen.queryByText("已回退到应用数据目录（文稿/下载目录不可用）")
    ).not.toBeInTheDocument();
  });

  it("导出被后端拒绝时用 role=alert 展示错误文案", async () => {
    // Arrange
    mockExportBackup.mockRejectedValue(new Error("备份加密失败：密钥派生失败"));
    const user = userEvent.setup();
    render(<StoragePanel />);

    // Act
    await user.click(screen.getByRole("button", { name: "导出备份" }));

    // Assert
    expect(await screen.findByRole("alert")).toHaveTextContent("备份加密失败：密钥派生失败");
  });

  it("导入口令错误时展示明确中文错误，且不显示成功文案", async () => {
    // Arrange
    mockImportBackup.mockRejectedValue(new Error("备份口令错误，或文件已损坏"));
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<StoragePanel />);

    // Act: 选文件 → 确认 → 后端报口令错
    fireEvent.change(screen.getByLabelText("选择备份文件"), {
      target: { files: [new File([new Uint8Array([1, 2, 3])], "backup.qqbp")] },
    });

    // Assert
    expect(await screen.findByRole("alert")).toHaveTextContent("备份口令错误，或文件已损坏");
    expect(screen.queryByText(/导入完成/)).not.toBeInTheDocument();
    confirmSpy.mockRestore();
  });

  it("导入前二次确认取消时不调用后端", async () => {
    // Arrange
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<StoragePanel />);

    // Act
    fireEvent.change(screen.getByLabelText("选择备份文件"), {
      target: { files: [new File([new Uint8Array([9, 9])], "backup.qqbp")] },
    });

    // Assert: 确认弹窗出现过，但后端一次都没被调用
    expect(confirmSpy).toHaveBeenCalledTimes(1);
    expect(mockImportBackup).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it("确认导入后把文件字节的 base64 与口令交给后端，并展示合并结果", async () => {
    // Arrange
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    const user = userEvent.setup();
    render(<StoragePanel />);

    // Act
    await user.type(screen.getByLabelText("备份口令"), "backup-passphrase");
    fireEvent.change(screen.getByLabelText("选择备份文件"), {
      // 字节 [65, 66, 67] 的标准 base64 为 "QUJD"
      target: { files: [new File([new Uint8Array([65, 66, 67])], "backup.qqbp")] },
    });

    // Assert
    await waitFor(() => {
      expect(mockImportBackup).toHaveBeenCalledWith("QUJD", "backup-passphrase");
    });
    expect(screen.getByText(/导入完成：新增 2 条，跳过已存在 1 条/)).toBeInTheDocument();
    confirmSpy.mockRestore();
  });
});
