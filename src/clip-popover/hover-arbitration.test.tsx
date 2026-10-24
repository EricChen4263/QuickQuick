import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ClipPopoverApp from "./ClipPopoverApp";

// 渲染测试环境无 Tauri 运行时：事件订阅、窗口操作、IPC 全部打桩
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn().mockResolvedValue(null),
  // 外观设置读取桩：默认值即生产默认（最近复制 / 24px / 1500ms / 背景高亮）
  getAppearanceSettings: vi.fn().mockResolvedValue({
    sortOrder: "recent",
    imageHeightPx: 24,
    previewFirstDelayMs: 1500,
    highlightStyle: "default",
  }),
  listClipItems: vi.fn(),
  pasteToFront: vi.fn(),
  hideAndReturnFocus: vi.fn(),
  copyClipToClipboard: vi.fn(),
}));

const mockHide = vi.fn();
const mockOnFocusChanged = vi.fn(() => Promise.resolve(() => {}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    hide: mockHide,
    onFocusChanged: mockOnFocusChanged,
  }),
}));

import { listClipItems } from "../ipc/ipc-client";

const mockListClipItems = vi.mocked(listClipItems);

/** 四条目列表：顺序即扁平遍历顺序，第一项为面板打开时的默认选中项 */
const ITEMS = [
  { id: "id1", content: "first item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
  { id: "id2", content: "second item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
  { id: "id3", content: "third item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
  { id: "id4", content: "fourth item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
];

beforeEach(() => {
  vi.clearAllMocks();
  mockOnFocusChanged.mockResolvedValue(() => {});
  mockHide.mockResolvedValue(undefined);
  mockListClipItems.mockResolvedValue(ITEMS);
});

/** 渲染并等待列表就绪，返回搜索框元素 */
async function renderReady(): Promise<HTMLInputElement> {
  render(<ClipPopoverApp />);
  await waitFor(() => {
    expect(screen.getAllByRole("option").length).toBe(ITEMS.length);
  });
  return screen.getByRole("searchbox") as HTMLInputElement;
}

/** 取第 index 行（role=option） */
function rowAt(index: number): HTMLElement {
  const row = screen.getAllByRole("option")[index];
  if (row === undefined) {
    throw new Error(`列表中不存在第 ${index + 1} 行`);
  }
  return row;
}

describe("ClipPopoverApp 悬停与键盘仲裁（P2-12）", () => {
  it("键盘导航后，静止指针的进入与同坐标移动都不抢走选中项", async () => {
    // Arrange：先让鼠标真实移动到第 4 行，主导权归鼠标
    const input = await renderReady();
    fireEvent.mouseMove(rowAt(3), { clientX: 20, clientY: 120 });
    await waitFor(() => {
      expect(rowAt(3)).toHaveAttribute("aria-selected", "true");
    });

    // 键盘上移两行到第 2 行，主导权交还键盘
    fireEvent.keyDown(input, { key: "ArrowUp" });
    fireEvent.keyDown(input, { key: "ArrowUp" });
    await waitFor(() => {
      expect(rowAt(1)).toHaveAttribute("aria-selected", "true");
    });

    // Act：键盘翻页使列表滚动，静止指针下浏览器补发同坐标 mousemove + mouseenter
    fireEvent.mouseMove(rowAt(3), { clientX: 20, clientY: 120 });
    fireEvent.mouseEnter(rowAt(3));

    // Assert：选中项仍在键盘选中的第 2 行
    expect(rowAt(1)).toHaveAttribute("aria-selected", "true");
    expect(rowAt(3)).toHaveAttribute("aria-selected", "false");
  });

  it("pointer 真实位移（坐标变化）把选中交给鼠标", async () => {
    // Arrange：键盘下移到第 2 行，主导权归键盘
    const input = await renderReady();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    await waitFor(() => {
      expect(rowAt(1)).toHaveAttribute("aria-selected", "true");
    });

    // Act：鼠标移动到第 3 行，坐标与上一次记录不同
    fireEvent.mouseMove(rowAt(2), { clientX: 40, clientY: 90 });

    // Assert：选中项跟到第 3 行
    await waitFor(() => {
      expect(rowAt(2)).toHaveAttribute("aria-selected", "true");
    });
    expect(rowAt(1)).toHaveAttribute("aria-selected", "false");

    // 主导权留在鼠标：此后仅进入某行（无位移）也能改选中
    fireEvent.mouseEnter(rowAt(0));
    await waitFor(() => {
      expect(rowAt(0)).toHaveAttribute("aria-selected", "true");
    });
  });

  it("键盘再次导航立即交还键盘，之后的悬停不再改选中项", async () => {
    // Arrange：鼠标主导，选中项在第 3 行
    const input = await renderReady();
    fireEvent.mouseMove(rowAt(2), { clientX: 40, clientY: 90 });
    await waitFor(() => {
      expect(rowAt(2)).toHaveAttribute("aria-selected", "true");
    });

    // Act：键盘从第 3 行下移到第 4 行
    fireEvent.keyDown(input, { key: "ArrowDown" });
    await waitFor(() => {
      expect(rowAt(3)).toHaveAttribute("aria-selected", "true");
    });

    // 鼠标没动，只是被滚动送到第 1 行下方
    fireEvent.mouseEnter(rowAt(0));

    // Assert
    expect(rowAt(3)).toHaveAttribute("aria-selected", "true");
    expect(rowAt(0)).toHaveAttribute("aria-selected", "false");
  });
});
