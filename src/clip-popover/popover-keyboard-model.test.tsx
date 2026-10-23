import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ClipPopoverApp from "./ClipPopoverApp";

// 渲染测试环境无 Tauri 运行时：事件订阅、窗口操作、IPC 全部打桩
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../ipc/ipc-client", () => ({
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

import { listClipItems, pasteToFront, hideAndReturnFocus } from "../ipc/ipc-client";

const mockListClipItems = vi.mocked(listClipItems);
const mockPasteToFront = vi.mocked(pasteToFront);
const mockHideAndReturnFocus = vi.mocked(hideAndReturnFocus);

/** 四条目列表：顺序即收藏 → 今天 → 更早的扁平顺序（都落在同一组，保持入参顺序） */
const ITEMS = [
  { id: "id1", content: "first item", kind: "text" as const, isFavorite: false, lastModifiedUtc: Date.now() },
  { id: "id2", content: "second item", kind: "text" as const, isFavorite: false, lastModifiedUtc: Date.now() },
  { id: "id3", content: "third item", kind: "text" as const, isFavorite: false, lastModifiedUtc: Date.now() },
  { id: "id4", content: "fourth item", kind: "text" as const, isFavorite: false, lastModifiedUtc: Date.now() },
];

beforeEach(() => {
  vi.clearAllMocks();
  mockOnFocusChanged.mockResolvedValue(() => {});
  mockHide.mockResolvedValue(undefined);
  mockPasteToFront.mockResolvedValue({ outcome: "full_paste" });
  mockHideAndReturnFocus.mockResolvedValue(undefined);
  mockListClipItems.mockResolvedValue(ITEMS);
});

/**
 * 渲染并等待列表就绪，返回搜索框元素。
 */
async function renderReady(): Promise<HTMLInputElement> {
  render(<ClipPopoverApp />);
  await waitFor(() => {
    expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
  });
  return screen.getByRole("searchbox") as HTMLInputElement;
}

/** 取第 index 行（role=option） */
function rowAt(index: number): HTMLElement {
  const rows = screen.getAllByRole("option");
  const row = rows[index];
  if (row === undefined) {
    throw new Error(`列表中不存在第 ${index + 1} 行`);
  }
  return row;
}

describe("ClipPopoverApp 键盘模型：列表移动（P2-2）", () => {
  it("⌃N 与 ⌃J 下移一条", async () => {
    const input = await renderReady();

    fireEvent.keyDown(input, { key: "n", ctrlKey: true });
    await waitFor(() => {
      expect(rowAt(1)).toHaveAttribute("aria-selected", "true");
    });

    fireEvent.keyDown(input, { key: "j", ctrlKey: true });
    await waitFor(() => {
      expect(rowAt(2)).toHaveAttribute("aria-selected", "true");
    });
  });

  it("⌃P 与 ⌃K 上移一条", async () => {
    const input = await renderReady();

    // 先下移两行到 id3
    fireEvent.keyDown(input, { key: "n", ctrlKey: true });
    fireEvent.keyDown(input, { key: "n", ctrlKey: true });
    await waitFor(() => {
      expect(rowAt(2)).toHaveAttribute("aria-selected", "true");
    });

    fireEvent.keyDown(input, { key: "p", ctrlKey: true });
    await waitFor(() => {
      expect(rowAt(1)).toHaveAttribute("aria-selected", "true");
    });

    fireEvent.keyDown(input, { key: "k", ctrlKey: true });
    await waitFor(() => {
      expect(rowAt(0)).toHaveAttribute("aria-selected", "true");
    });
  });

  it("⌘↓ 跳到第一条以外的最后一条，⌘↑ 回到第一条", async () => {
    const input = await renderReady();

    fireEvent.keyDown(input, { key: "ArrowDown", metaKey: true });
    await waitFor(() => {
      expect(rowAt(3)).toHaveAttribute("aria-selected", "true");
    });

    fireEvent.keyDown(input, { key: "ArrowUp", metaKey: true });
    await waitFor(() => {
      expect(rowAt(0)).toHaveAttribute("aria-selected", "true");
    });
  });

  it("⌘↓ 在空列表时不做任何事（不调用粘贴、不抛错）", async () => {
    // Arrange：空列表
    mockListClipItems.mockResolvedValue([]);
    render(<ClipPopoverApp />);
    await waitFor(() => {
      expect(screen.getByText("剪贴板暂无内容")).toBeInTheDocument();
    });

    // Act
    const input = screen.getByRole("searchbox");
    const notCancelled = fireEvent.keyDown(input, { key: "ArrowDown", metaKey: true });

    // Assert：按键被面板吞掉，但没有可选项也不取用
    expect(notCancelled).toBe(false);
    expect(mockPasteToFront).not.toHaveBeenCalled();
  });

  it("⌥↓ 仍按原有行为下移一条（不被新增键位抢走）", async () => {
    const input = await renderReady();

    fireEvent.keyDown(input, { key: "ArrowDown", altKey: true });

    await waitFor(() => {
      expect(rowAt(1)).toHaveAttribute("aria-selected", "true");
    });
  });
});

describe("ClipPopoverApp 键盘模型：搜索框编辑（P2-2）", () => {
  it("⌃U 清空搜索框并把光标置 0", async () => {
    const input = await renderReady();
    fireEvent.change(input, { target: { value: "hello world" } });
    expect(input).toHaveValue("hello world");

    fireEvent.keyDown(input, { key: "u", ctrlKey: true });

    expect(input).toHaveValue("");
    expect(input.selectionStart).toBe(0);
  });

  it("⌃W 删掉光标前一个词，光标停在编辑后位置（不跳回末尾）", async () => {
    const input = await renderReady();
    fireEvent.change(input, { target: { value: "hello world" } });
    input.setSelectionRange(11, 11);

    fireEvent.keyDown(input, { key: "w", ctrlKey: true });

    expect(input).toHaveValue("hello ");
    expect(input.selectionStart).toBe(6);
  });

  it("⌃W 在光标位于中间时只删前半段词", async () => {
    const input = await renderReady();
    fireEvent.change(input, { target: { value: "hello world" } });
    input.setSelectionRange(5, 5);

    fireEvent.keyDown(input, { key: "w", ctrlKey: true });

    expect(input).toHaveValue(" world");
    expect(input.selectionStart).toBe(0);
  });

  it("⌃W 在光标位于开头时什么都不删", async () => {
    const input = await renderReady();
    fireEvent.change(input, { target: { value: "hello" } });
    input.setSelectionRange(0, 0);

    fireEvent.keyDown(input, { key: "w", ctrlKey: true });

    expect(input).toHaveValue("hello");
    expect(input.selectionStart).toBe(0);
  });

  it("⌃H 删掉光标前一个字符，等价 Backspace", async () => {
    const input = await renderReady();
    fireEvent.change(input, { target: { value: "hello" } });
    input.setSelectionRange(5, 5);

    fireEvent.keyDown(input, { key: "h", ctrlKey: true });

    expect(input).toHaveValue("hell");
    expect(input.selectionStart).toBe(4);
  });

  it("⌃W 删除后列表按新的搜索词过滤", async () => {
    const input = await renderReady();
    fireEvent.change(input, { target: { value: "first item x" } });
    input.setSelectionRange(12, 12);

    // Act：删掉光标前的 "x"，剩余搜索词恢复匹配
    fireEvent.keyDown(input, { key: "w", ctrlKey: true });

    expect(input).toHaveValue("first item ");
    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });
  });
});
