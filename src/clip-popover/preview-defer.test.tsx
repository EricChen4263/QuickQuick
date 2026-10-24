import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import ClipPopoverApp from "./ClipPopoverApp";

// 渲染测试环境无 Tauri 运行时：事件订阅、窗口操作、IPC 全部打桩
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn().mockResolvedValue(null),
  listClipItems: vi.fn(),
  pasteToFront: vi.fn(),
  hideAndReturnFocus: vi.fn(),
  copyClipToClipboard: vi.fn(),
}));

const mockHide = vi.fn();
// 捕获 onFocusChanged 回调，供测试模拟「面板重新打开」
let capturedFocusCallback: ((event: { payload: boolean }) => void) | null = null;
const mockOnFocusChanged = vi.fn((cb: (event: { payload: boolean }) => void) => {
  capturedFocusCallback = cb;
  return Promise.resolve(() => {});
});

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    hide: mockHide,
    onFocusChanged: mockOnFocusChanged,
  }),
}));

import { listClipItems, pasteToFront, hideAndReturnFocus, copyClipToClipboard } from "../ipc/ipc-client";

const mockListClipItems = vi.mocked(listClipItems);

/** 四条目列表：顺序即扁平列表顺序（同组内保持入参顺序） */
const ITEMS = [
  { id: "id1", content: "first item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
  { id: "id2", content: "second item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
  { id: "id3", content: "third item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
  { id: "id4", content: "fourth item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
];

beforeEach(() => {
  vi.clearAllMocks();
  // 只假造定时器，保留真实 Date：分组逻辑依赖 Date.now() 判断「今天」
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  capturedFocusCallback = null;
  mockListClipItems.mockResolvedValue(ITEMS);
  vi.mocked(pasteToFront).mockResolvedValue({ outcome: "full_paste" });
  vi.mocked(hideAndReturnFocus).mockResolvedValue(undefined);
  vi.mocked(copyClipToClipboard).mockResolvedValue(undefined);
  mockHide.mockResolvedValue(undefined);
});

afterEach(() => {
  vi.useRealTimers();
});

/** 渲染并等 IPC 列表就绪（假定时器下不用 waitFor，直接刷空 microtask 队列） */
async function renderReady(): Promise<HTMLElement> {
  const { container } = render(<ClipPopoverApp />);
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
  return container;
}

/** 只取右侧预览区的文本，避免与左侧列表里同名的条目内容混淆 */
function previewText(container: HTMLElement): string {
  return container.querySelector(".popover-preview")?.textContent ?? "";
}

describe("ClipPopoverApp 行内预览分级延迟（P2-10）", () => {
  it("首个预览等 1500ms 才出现，此前显示中性占位而不是「无选中项」", async () => {
    const container = await renderReady();

    // 数据已在内存：列表立刻就有 4 条且已有选中项，等待只发生在预览区
    expect(screen.getAllByRole("option")).toHaveLength(4);
    expect(screen.getAllByRole("option")[0]?.getAttribute("aria-selected")).toBe("true");
    // 列表里明明有高亮行，预览区就不能说「无选中项」
    expect(previewText(container)).toBe("预览稍后显示");
    expect(previewText(container)).not.toContain("无选中项");

    act(() => {
      vi.advanceTimersByTime(1499);
    });
    expect(previewText(container)).toBe("预览稍后显示");
    expect(previewText(container)).not.toContain("first item");

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(previewText(container)).toContain("first item");
  });

  it("首个预览之后切换条目：200ms 内不切换，200ms 后切换", async () => {
    const container = await renderReady();
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    expect(previewText(container)).toContain("first item");

    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "ArrowDown" });

    act(() => {
      vi.advanceTimersByTime(199);
    });
    expect(previewText(container)).toContain("first item");
    expect(previewText(container)).not.toContain("second item");

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(previewText(container)).toContain("second item");
  });

  it("连按三次 ↓：中间条目的预览从未出现，最终只显示最后一条", async () => {
    const container = await renderReady();
    // 首个预览先落地，进入 200ms 档
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    expect(previewText(container)).toContain("first item");

    const input = screen.getByRole("searchbox");
    const samples: string[] = [];
    fireEvent.keyDown(input, { key: "ArrowDown" }); // → id2
    samples.push(previewText(container));
    act(() => {
      vi.advanceTimersByTime(100);
    });
    samples.push(previewText(container));
    fireEvent.keyDown(input, { key: "ArrowDown" }); // → id3
    samples.push(previewText(container));
    act(() => {
      vi.advanceTimersByTime(100);
    });
    samples.push(previewText(container));
    fireEvent.keyDown(input, { key: "ArrowDown" }); // → id4
    samples.push(previewText(container));
    act(() => {
      vi.advanceTimersByTime(199);
    });
    samples.push(previewText(container));

    // 整段推进时序里预览一直停在首个条目：中间条目从未被渲染出来
    for (const text of samples) {
      expect(text).toContain("first item");
      expect(text).not.toContain("second item");
      expect(text).not.toContain("third item");
    }

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(previewText(container)).toContain("fourth item");
  });

  it("列表为空（真的没有选中项）时仍显示「无选中项」，不退化成延迟占位", async () => {
    mockListClipItems.mockResolvedValue([]);
    const container = await renderReady();

    expect(screen.queryAllByRole("option")).toHaveLength(0);
    expect(previewText(container)).toBe("无选中项");
    expect(previewText(container)).not.toContain("预览稍后显示");
  });

  it("面板重新打开（重新获得焦点）后首个预览重新等 1500ms", async () => {
    const container = await renderReady();
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    expect(previewText(container)).toContain("first item");

    act(() => {
      capturedFocusCallback?.({ payload: true });
    });
    expect(previewText(container)).toBe("预览稍后显示");

    act(() => {
      vi.advanceTimersByTime(1499);
    });
    expect(previewText(container)).not.toContain("first item");

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(previewText(container)).toContain("first item");
  });

  it("卸载时清掉未到期的预览定时器，之后推进定时器不报错也不更新状态", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const { unmount } = render(<ClipPopoverApp />);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.getAllByRole("option")).toHaveLength(4);

    // React 自己会排一个 0ms 调度定时器，先让它落地；此后仅剩预览的 1500ms 定时器
    act(() => {
      vi.advanceTimersByTime(0);
    });
    expect(vi.getTimerCount()).toBe(1);

    unmount();
    // 卸载必须把预览定时器清干净：没有待触发的回调，也就不可能 setState
    expect(vi.getTimerCount()).toBe(0);

    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });
});
