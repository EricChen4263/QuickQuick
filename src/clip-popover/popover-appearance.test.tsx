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
    imageHeightPx: 40,
    previewFirstDelayMs: 1500,
    highlightStyle: "bold",
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

/**
 * 按分组把列表行归桶：返回「组名 → 行文本数组」的有序 Map。
 *
 * 分组改为 `role="group"` 包裹（Listbox 的直接子项只能是 option / group，
 * 裸 div 的分组标题不合法），所以这里按 ARIA 结构取组名与行，
 * 而不是按 DOM 相邻关系推断——断言的语义（哪一行落在哪一组）没有变。
 */
function rowsByGroup(container: HTMLElement): Map<string, string[]> {
  const list = container.querySelector(".popover-list");
  if (list === null) throw new Error("找不到 .popover-list");
  const buckets = new Map<string, string[]>();
  for (const group of Array.from(list.querySelectorAll('[role="group"]'))) {
    const label = group.getAttribute("aria-label") ?? "";
    const rows = Array.from(group.querySelectorAll('[role="option"]')).map(
      (row) => row.textContent ?? "",
    );
    buckets.set(label, rows);
  }
  return buckets;
}

describe("ClipPopoverApp 消费外观设置", () => {
  it("默认设置对齐 Maccy：加粗高亮、缩略图 40px、首个预览 1500ms", async () => {
    const container = await renderReady();

    expect(shell(container).getAttribute("data-hit-style")).toBe("bold");
    expect(shell(container).style.getPropertyValue("--popover-thumb-height")).toBe("40px");

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

  it("自定义设置生效：下划线高亮、缩略图 120px、首个预览 300ms", async () => {
    vi.mocked(getAppearanceSettings).mockResolvedValue({
      sortOrder: "first_copied",
      imageHeightPx: 120,
      previewFirstDelayMs: 300,
      highlightStyle: "underline",
    });

    const container = await renderReady();

    expect(shell(container).getAttribute("data-hit-style")).toBe("underline");
    expect(shell(container).style.getPropertyValue("--popover-thumb-height")).toBe("120px");

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
      imageHeightPx: 40,
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
    expect(shell(container).getAttribute("data-hit-style")).toBe("bold");
    expect(shell(container).style.getPropertyValue("--popover-thumb-height")).toBe("40px");
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});

describe("ClipPopoverApp 分组口径跟随排序（P5-5 收尾）", () => {
  const NOW_OFFSET = 2 * 24 * 60 * 60 * 1000;

  /** 两条数据：A 今天被重新复制但 2 天前首次入库；B 今天首次入库但 2 天前复制过 */
  function groupingFixture() {
    return [
      {
        id: "created-today",
        content: "今天首次复制",
        kind: "text" as const,
        isFavorite: false,
        hasThumbnail: false,
        lastModifiedUtc: Date.now() - NOW_OFFSET,
        createdUtc: Date.now(),
      },
      {
        id: "created-old",
        content: "两天前首次复制",
        kind: "text" as const,
        isFavorite: false,
        hasThumbnail: false,
        lastModifiedUtc: Date.now(),
        createdUtc: Date.now() - NOW_OFFSET,
      },
    ];
  }

  it("最近复制档：按 lastModified 归档——今天被重新复制的旧条目进「今天」", async () => {
    vi.mocked(listClipItems).mockResolvedValue(groupingFixture());
    const container = await renderReady();

    const buckets = rowsByGroup(container);
    expect(buckets.get("今天")).toEqual(["两天前首次复制"]);
    expect(buckets.get("更早")).toEqual(["今天首次复制"]);
  });

  it("首次复制档：按 createdUtc 归档——分组口径与排序口径一致", async () => {
    vi.mocked(listClipItems).mockResolvedValue(groupingFixture());
    vi.mocked(getAppearanceSettings).mockResolvedValue({
      sortOrder: "first_copied",
      imageHeightPx: 40,
      previewFirstDelayMs: 1500,
      highlightStyle: "bold",
    });
    const container = await renderReady();

    const buckets = rowsByGroup(container);
    expect(buckets.get("今天")).toEqual(["今天首次复制"]);
    expect(buckets.get("更早")).toEqual(["两天前首次复制"]);
  });
});
