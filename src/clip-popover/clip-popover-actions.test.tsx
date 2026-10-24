import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import ClipPopoverApp from "./ClipPopoverApp";

vi.mock("../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn().mockResolvedValue(null),
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
  { id: "id1", content: "first item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
  { id: "id2", content: "second item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
  { id: "id3", content: "third item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
  {
    id: "id-img",
    content: "",
    kind: "image" as const,
    isFavorite: false,
    hasThumbnail: true,
    lastModifiedUtc: Date.now(),
    imageId: "img-id-1",
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

describe("ClipPopoverApp 输入法防御（P2-6）", () => {
  it("composition 期间按 ArrowDown 不改变选中项", async () => {
    render(<ClipPopoverApp />);
    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "ArrowDown", isComposing: true });

    // 选中仍是第一项：方向键在组词期间归输入法，不驱动列表导航
    const rows = screen.getAllByRole("option");
    expect(rows[0]).toHaveAttribute("aria-selected", "true");
    expect(rows[1]).toHaveAttribute("aria-selected", "false");
  });

  it("keyCode=229（WKWebView 组词表现）按 ArrowDown 同样不导航", async () => {
    render(<ClipPopoverApp />);
    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "ArrowDown", keyCode: 229 });

    const rows = screen.getAllByRole("option");
    expect(rows[0]).toHaveAttribute("aria-selected", "true");
    expect(rows[1]).toHaveAttribute("aria-selected", "false");
  });

  it("composition 结束后 ArrowDown 恢复导航", async () => {
    render(<ClipPopoverApp />);
    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "ArrowDown", isComposing: true });
    fireEvent.keyDown(input, { key: "ArrowDown" });

    await waitFor(() => {
      const rows = screen.getAllByRole("option");
      expect(rows[1]).toHaveAttribute("aria-selected", "true");
    });
  });

  it("composition 期间按 Esc 仍然关窗（Esc 是显式中断，不受组词防御影响）", async () => {
    render(<ClipPopoverApp />);
    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "Escape", isComposing: true });

    await waitFor(() => {
      expect(mockHideAndReturnFocus).toHaveBeenCalled();
    });
  });
});

describe("ClipPopoverApp 数字直达（P2-3）", () => {
  /** 十条目列表：用于验证 0 → 第 10 条 */
  const TEN_ITEMS = Array.from({ length: 10 }, (_, i) => ({
    id: `d${i + 1}`,
    content: `item ${i + 1}`,
    kind: "text" as const,
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: Date.now(),
  }));

  /**
   * fake timers 下刷完 microtask 队列。
   * waitFor 依赖真实计时器，注入假计时器后会一直挂到超时，故此处只用 act 冲微任务。
   */
  async function flushAsync(): Promise<void> {
    await act(async () => {
      for (let i = 0; i < 6; i++) {
        await Promise.resolve();
      }
    });
  }

  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("按 '2' 先高亮第 2 条，50ms 后才调用 pasteToFront", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const rows = screen.getAllByRole("option");
    expect(rows).toHaveLength(10);

    // Act：数字直达只吞键、先高亮
    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "2" });

    // Assert：反馈阶段——第 2 条已高亮，但尚未取用
    expect(rows[1]).toHaveAttribute("aria-selected", "true");
    expect(rows[0]).toHaveAttribute("aria-selected", "false");
    expect(mockPasteToFront).not.toHaveBeenCalled();

    // Act：推进反馈时长
    act(() => {
      vi.advanceTimersByTime(50);
    });
    await flushAsync();

    // Assert：取用第 2 条并关窗
    expect(mockPasteToFront).toHaveBeenCalledWith("d2");
    expect(mockHide).toHaveBeenCalled();
  });

  it("按 '0' 取第 10 条", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");

    // Act
    fireEvent.keyDown(input, { key: "0" });
    const rows = screen.getAllByRole("option");
    expect(rows[9]).toHaveAttribute("aria-selected", "true");

    act(() => {
      vi.advanceTimersByTime(50);
    });
    await flushAsync();

    // Assert
    expect(mockPasteToFront).toHaveBeenCalledWith("d10");
  });

  it("条目不足该序号时什么也不做（数字不入搜索框、不取用）", async () => {
    // Arrange：只有 4 条
    mockListClipItems.mockResolvedValue(ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");

    // Act：按第 5 条（不存在）；dispatchEvent 返回 false 表示默认行为已被阻止
    const notCancelled = fireEvent.keyDown(input, { key: "5" });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert：按键被面板吞掉，但不取用、也不移动选中
    expect(notCancelled).toBe(false);
    expect(mockPasteToFront).not.toHaveBeenCalled();
    expect(input).toHaveValue("");
    const rows = screen.getAllByRole("option");
    expect(rows[0]).toHaveAttribute("aria-selected", "true");
  });

  it("搜索框非空时数字让位给输入：不拦截、不取用", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");
    fireEvent.change(input, { target: { value: "item 1" } });

    // Act
    const notCancelled = fireEvent.keyDown(input, { key: "2" });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert：数字键未被数字直达接管
    expect(notCancelled).toBe(true);
    expect(mockPasteToFront).not.toHaveBeenCalled();
  });

  it("带修饰键时不算数字直达（⌘2 不取用）", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");

    // Act
    const notCancelled = fireEvent.keyDown(input, { key: "2", metaKey: true });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert
    expect(notCancelled).toBe(true);
    expect(mockPasteToFront).not.toHaveBeenCalled();
  });

  it("卸载时清掉反馈定时器：不再取用，也不产生 act 警告", async () => {
    // Arrange：把 console.error 当哨兵，捕获 React 的 act 警告
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    const { unmount } = render(<ClipPopoverApp />);
    await flushAsync();

    // Act：按下直达键后立即卸载（面板被关节径之一）
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "2" });
    unmount();
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert：定时器已清除，不会在卸载后取用，也没有 setState 警告
    expect(mockPasteToFront).not.toHaveBeenCalled();
    expect(consoleError).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  it("Esc 关窗时取消尚未触发的数字直达取用", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");

    // Act：先按直达键，50ms 内按 Esc 关窗
    fireEvent.keyDown(input, { key: "2" });
    fireEvent.keyDown(input, { key: "Escape" });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert：只关窗（hideAndReturnFocus），不再补一次粘贴
    expect(mockHideAndReturnFocus).toHaveBeenCalled();
    expect(mockPasteToFront).not.toHaveBeenCalled();
  });

  it("50ms 内按 Enter：只粘贴一次（待取用被取消，不双发）", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");

    // Act：先按直达键 arm 定时器，再在 50ms 内按 Enter
    fireEvent.keyDown(input, { key: "2" });
    fireEvent.keyDown(input, { key: "Enter" });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert：Enter 自己的那次粘贴照常发生，定时器那次不再补
    expect(mockPasteToFront).toHaveBeenCalledTimes(1);
    expect(mockPasteToFront).toHaveBeenCalledWith("d2");
  });

  it("50ms 内按 ⌥Enter：只复制不粘贴（不双发）", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");

    // Act
    fireEvent.keyDown(input, { key: "2" });
    fireEvent.keyDown(input, { key: "Enter", altKey: true });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert：复制一次（当前高亮 d2），粘贴一次都不发生
    expect(mockCopyClipToClipboard).toHaveBeenCalledTimes(1);
    expect(mockCopyClipToClipboard).toHaveBeenCalledWith("d2");
    expect(mockPasteToFront).not.toHaveBeenCalled();
  });

  it("50ms 内开始打字：不再粘贴已放弃的条目", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");

    // Act：按直达键后立刻输入一个字符（非数字键 → 取消待取用）
    fireEvent.keyDown(input, { key: "2" });
    fireEvent.keyDown(input, { key: "a" });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert
    expect(mockPasteToFront).not.toHaveBeenCalled();
  });

  it("组词期间的按键同样取消待取用（取消必须在输入法防御之前）", async () => {
    // Arrange：组词中按键会先在 isImeComposing 处 return，
    // 若把「非数字键取消」挪到那个 guard 之后，取消就永远走不到，用户按 2 后开始打拼音仍会被粘。
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");

    // Act
    fireEvent.keyDown(input, { key: "2" });
    fireEvent.keyDown(input, { key: "ArrowDown", isComposing: true });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert
    expect(mockPasteToFront).not.toHaveBeenCalled();
  });

  it("50ms 内按 ArrowDown 导航走：不再粘贴已导航走的条目", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");

    // Act
    fireEvent.keyDown(input, { key: "2" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert：用户已把高亮移到 d3，d2 不应再被粘
    expect(mockPasteToFront).not.toHaveBeenCalled();
    const rows = screen.getAllByRole("option");
    expect(rows[2]).toHaveAttribute("aria-selected", "true");
  });

  it("50ms 内连按两个数字：只取用最后一个", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(TEN_ITEMS);
    render(<ClipPopoverApp />);
    await flushAsync();
    const input = screen.getByRole("searchbox");

    // Act：先 2 再 3，两次都在 50ms 反馈窗口内
    fireEvent.keyDown(input, { key: "2" });
    fireEvent.keyDown(input, { key: "3" });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await flushAsync();

    // Assert：只有最后一次 arm 的 d3 被取用
    expect(mockPasteToFront).toHaveBeenCalledTimes(1);
    expect(mockPasteToFront).toHaveBeenCalledWith("d3");
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
    // 用 act 包裹：面板重新获焦会重置预览会话使其重渲染，裸调用会在 act 之外触发 setState
    act(() => {
      capturedFocusCallback!({ payload: true });
    });

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
    // 用 act 包裹：面板重新获焦会重置预览会话使其重渲染，裸调用会在 act 之外触发 setState
    act(() => {
      capturedFocusCallback!({ payload: true });
    });

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
    // 用 act 包裹：面板重新获焦会重置预览会话使其重渲染，裸调用会在 act 之外触发 setState
    act(() => {
      capturedFocusCallback!({ payload: true });
    });

    await waitFor(() => {
      const rows = screen.getAllByRole("option");
      const firstRow = rows.find((r) => r.textContent?.includes("first item"));
      expect(firstRow).toHaveAttribute("aria-selected", "true");
    });
  });
});
