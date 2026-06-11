import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ClipPopoverApp from "./ClipPopoverApp";

vi.mock("../ipc/ipc-client", () => ({
  listClipItems: vi.fn(),
  pasteToFront: vi.fn(),
  hideAndReturnFocus: vi.fn(),
  copyClipToClipboard: vi.fn(),
}));

vi.mock("../panels/translate/browser-api", () => ({
  writeToClipboard: vi.fn(),
}));

// ClipPopoverApp 订阅 clipboard-changed（@tauri-apps/api/event.listen）；
// 测试环境无 Tauri 运行时，mock 为空 unlisten。
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const mockHide = vi.fn();

// 捕获 onFocusChanged 回调，供测试手动触发 focused=true/false
let capturedFocusCallback: ((event: { payload: boolean }) => void) | null = null;
const mockUnlisten = vi.fn();
const mockOnFocusChanged = vi.fn((cb: (event: { payload: boolean }) => void) => {
  capturedFocusCallback = cb;
  return Promise.resolve(mockUnlisten);
});

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    hide: mockHide,
    onFocusChanged: mockOnFocusChanged,
  }),
}));

import { listClipItems, pasteToFront, hideAndReturnFocus, copyClipToClipboard } from "../ipc/ipc-client";
import { writeToClipboard } from "../panels/translate/browser-api";

const mockListClipItems = vi.mocked(listClipItems);
const mockPasteToFront = vi.mocked(pasteToFront);
const mockWriteToClipboard = vi.mocked(writeToClipboard);
const mockHideAndReturnFocus = vi.mocked(hideAndReturnFocus);
const mockCopyClipToClipboard = vi.mocked(copyClipToClipboard);

/**
 * 刷空 microtask 队列：用于验证"某副作用不发生"的负断言。
 * 连刷数轮覆盖 IPC 的 .then/.catch 链深度，不用挂钟 setTimeout（慢 CI 下会 flaky）。
 */
async function flushMicrotasks(): Promise<void> {
  for (let i = 0; i < 5; i++) {
    await Promise.resolve();
  }
}

const ITEMS = [
  { id: "id1", content: "first item", kind: "text" as const, isFavorite: false, lastModifiedUtc: Date.now() },
  { id: "id2", content: "second item", kind: "text" as const, isFavorite: false, lastModifiedUtc: Date.now() },
  { id: "id3", content: "third item", kind: "text" as const, isFavorite: false, lastModifiedUtc: Date.now() },
  {
    id: "id-img",
    content: "",
    kind: "image" as const,
    isFavorite: false,
    lastModifiedUtc: Date.now(),
    thumbnailDataUrl: "data:image/png;base64,abc",
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  capturedFocusCallback = null;
  mockOnFocusChanged.mockImplementation((cb: (event: { payload: boolean }) => void) => {
    capturedFocusCallback = cb;
    return Promise.resolve(mockUnlisten);
  });
  mockListClipItems.mockResolvedValue(ITEMS);
  mockPasteToFront.mockResolvedValue({ outcome: "write_back_only" });
  mockWriteToClipboard.mockResolvedValue(undefined);
  mockHide.mockResolvedValue(undefined);
  mockHideAndReturnFocus.mockResolvedValue(undefined);
  mockCopyClipToClipboard.mockResolvedValue(undefined);
});

describe("ClipPopoverApp 键盘动作", () => {
  it("按 Enter 调 pasteToFront(selectedId) 并 hide", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("id1");
      expect(mockHide).toHaveBeenCalled();
    });
  });

  // RT1-F2-S02：Alt+Enter 复制改调 IPC copyClipToClipboard(id)（富文本保真），不再走 writeToClipboard。
  it("按 Alt+Enter 调 copyClipToClipboard(id) 并 hide", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "Enter", altKey: true });

    await waitFor(() => {
      expect(mockCopyClipToClipboard).toHaveBeenCalledWith("id1");
      expect(mockHide).toHaveBeenCalled();
    });
    expect(mockWriteToClipboard).not.toHaveBeenCalled();
  });

  it("按 ArrowDown 选中第二项", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("second item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "ArrowDown" });

    await waitFor(() => {
      const rows = screen.getAllByRole("option");
      const secondRow = rows.find((r) => r.textContent?.includes("second item"));
      expect(secondRow).toHaveAttribute("aria-selected", "true");
    });
  });

  it("按 ArrowDown 再 Enter 粘贴第二项", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("second item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("id2");
      expect(mockHide).toHaveBeenCalled();
    });
  });

  // 图片复制已落地（后端取原图 PNG 解码后 set_image），图片条目 Alt+Enter 也走 copyClipToClipboard。
  it("图片条目按 Alt+Enter：调 copyClipToClipboard(图片 id) 并 hide", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByRole("option").length).toBeGreaterThan(0);
    });

    // 初始选中 id1，连按 3 次 ArrowDown 选中 id-img（第 4 项）
    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });

    await waitFor(() => {
      const rows = screen.getAllByRole("option");
      // 第 4 项（index 3）是图片条目
      expect(rows[3]).toHaveAttribute("aria-selected", "true");
    });

    fireEvent.keyDown(input, { key: "Enter", altKey: true });

    await waitFor(() => {
      expect(mockCopyClipToClipboard).toHaveBeenCalledWith("id-img");
      expect(mockHide).toHaveBeenCalled();
    });
  });

  it("按 Esc 调 hideAndReturnFocus 关闭窗口并还焦", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "Escape" });

    // 方案 C：Esc 关闭走 hideAndReturnFocus（隐藏窗口 + 把焦点还给上一个外部 app），
    // 而非裸 getCurrentWindow().hide()（只隐藏、不还焦）。
    await waitFor(() => {
      expect(mockHideAndReturnFocus).toHaveBeenCalled();
    });
    expect(mockHide).not.toHaveBeenCalled();
  });

  it("pasteToFront 失败时不调 hide，控制台有 error", async () => {
    mockPasteToFront.mockRejectedValue(new Error("paste failed"));
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("id1");
    });

    // 验证 reject 后不调 hide：刷 microtask 队列让 .catch 链耗尽，不依赖挂钟时间。
    await flushMicrotasks();
    expect(mockHide).not.toHaveBeenCalled();
    expect(consoleError).toHaveBeenCalled();

    consoleError.mockRestore();
  });
});

describe("ClipPopoverApp 窗口焦点变化", () => {
  it("窗口获焦时输入框获得焦点", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    // 等待 onFocusChanged 订阅建立
    await waitFor(() => {
      expect(capturedFocusCallback).not.toBeNull();
    });

    const input = screen.getByRole("searchbox");
    // 先把焦点移到 body，模拟窗口显示前输入框未聚焦
    input.blur();
    expect(document.activeElement).not.toBe(input);

    // 模拟窗口获焦事件
    capturedFocusCallback!({ payload: true });

    await waitFor(() => {
      expect(document.activeElement).toBe(input);
    });
  });

  it("窗口获焦时 query 被重置为空字符串", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    await waitFor(() => {
      expect(capturedFocusCallback).not.toBeNull();
    });

    // 先在搜索框输入内容
    const input = screen.getByRole("searchbox");
    fireEvent.change(input, { target: { value: "hello" } });
    expect(input).toHaveValue("hello");

    // 触发窗口获焦
    capturedFocusCallback!({ payload: true });

    await waitFor(() => {
      expect(input).toHaveValue("");
    });
  });

  it("窗口获焦时 selectedId 重置为第一项", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    await waitFor(() => {
      expect(capturedFocusCallback).not.toBeNull();
    });

    // 先按 ArrowDown 把选中移到第二项
    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "ArrowDown" });

    await waitFor(() => {
      const rows = screen.getAllByRole("option");
      const secondRow = rows.find((r) => r.textContent?.includes("second item"));
      expect(secondRow).toHaveAttribute("aria-selected", "true");
    });

    // 触发窗口获焦，应回到第一项
    capturedFocusCallback!({ payload: true });

    await waitFor(() => {
      const rows = screen.getAllByRole("option");
      const firstRow = rows.find((r) => r.textContent?.includes("first item"));
      expect(firstRow).toHaveAttribute("aria-selected", "true");
    });
  });
});
