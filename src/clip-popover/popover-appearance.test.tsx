import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import ClipPopoverApp from "./ClipPopoverApp";

// 渲染测试环境无 Tauri 运行时：事件订阅、窗口操作、IPC 全部打桩
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn().mockResolvedValue(null),
  getAppearanceSettings: vi.fn(),
  listClipItems: vi.fn(),
  pasteToFront: vi.fn(),
  hideAndReturnFocus: vi.fn(),
  copyClipToClipboard: vi.fn(),
}));

const mockOnFocusChanged = vi.fn(() => Promise.resolve(() => {}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    hide: vi.fn().mockResolvedValue(undefined),
    onFocusChanged: mockOnFocusChanged,
  }),
}));

import {
  copyClipToClipboard,
  getAppearanceSettings,
  hideAndReturnFocus,
  listClipItems,
  pasteToFront,
} from "../ipc/ipc-client";

/** 两条文本条目：列表顺序即扁平列表顺序 */
const ITEMS = [
  {
    id: "id1",
    content: "first item",
    kind: "text" as const,
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: Date.now(),
  },
  {
    id: "id2",
    content: "second item",
    kind: "text" as const,
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: Date.now(),
  },
];

beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  mockOnFocusChanged.mockResolvedValue(() => {});
  vi.mocked(listClipItems).mockResolvedValue(ITEMS);
  vi.mocked(pasteToFront).mockResolvedValue({ outcome: "full_paste" });
  vi.mocked(hideAndReturnFocus).mockResolvedValue(undefined);
  vi.mocked(copyClipToClipboard).mockResolvedValue(undefined);
  vi.mocked(getAppearanceSettings).mockResolvedValue({
    sortOrder: "recent",
    imageHeightPx: 24,
    previewFirstDelayMs: 1500,
    highlightStyle: "default",
  });
});

afterEach(() => {
  vi.useRealTimers();
});

/** 渲染并等 IPC 就绪（假定时器下不用 waitFor，直接刷空 microtask 队列） */
async function renderReady(): Promise<HTMLElement> {
  const { container } = render(<ClipPopoverApp />);
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
  return container;
}

/** 取 popover 外壳（外观设置都挂在这个元素上） */
function shell(container: HTMLElement): HTMLElement {
  const element = container.querySelector(".popover-shell");
  if (!(element instanceof HTMLElement)) throw new Error("找不到 .popover-shell");
  return element;
}

/** 只取右侧预览区的文本 */
function previewText(container: HTMLElement): string {
  return container.querySelector(".popover-preview")?.textContent ?? "";
}

describe("ClipPopoverApp 消费外观设置", () => {
  it("默认设置下视觉与改动前一致：背景高亮、缩略图 24px、首个预览 1500ms", async () => {
    const container = await renderReady();

    expect(shell(container).getAttribute("data-hit-style")).toBe("default");
    expect(shell(container).style.getPropertyValue("--popover-thumb-height")).toBe("24px");

    // 首个预览仍然等满 1500ms
    act(() => {
      vi.advanceTimersByTime(1499);
    });
    expect(previewText(container)).toBe("预览稍后显示");
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(previewText(container)).toContain("first item");
  });

  it("自定义设置生效：下划线高亮、缩略图 40px、首个预览 300ms", async () => {
    vi.mocked(getAppearanceSettings).mockResolvedValue({
      sortOrder: "first_copied",
      imageHeightPx: 40,
      previewFirstDelayMs: 300,
      highlightStyle: "underline",
    });

    const container = await renderReady();

    expect(shell(container).getAttribute("data-hit-style")).toBe("underline");
    expect(shell(container).style.getPropertyValue("--popover-thumb-height")).toBe("40px");

    act(() => {
      vi.advanceTimersByTime(299);
    });
    expect(previewText(container)).toBe("预览稍后显示");
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(previewText(container)).toContain("first item");
  });

  it("首个预览档可配，但首个之后切换仍固定 200ms", async () => {
    vi.mocked(getAppearanceSettings).mockResolvedValue({
      sortOrder: "recent",
      imageHeightPx: 24,
      previewFirstDelayMs: 300,
      highlightStyle: "bold",
    });

    const container = await renderReady();
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(previewText(container)).toContain("first item");

    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "ArrowDown" });
    act(() => {
      vi.advanceTimersByTime(199);
    });
    expect(previewText(container)).not.toContain("second item");
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(previewText(container)).toContain("second item");
  });

  it("读取外观设置失败时回退默认值，不白屏也不清空列表", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.mocked(getAppearanceSettings).mockRejectedValue(new Error("settings.json 读失败"));

    const container = await renderReady();

    expect(screen.getAllByRole("option")).toHaveLength(2);
    expect(shell(container).getAttribute("data-hit-style")).toBe("default");
    expect(shell(container).style.getPropertyValue("--popover-thumb-height")).toBe("24px");
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});
