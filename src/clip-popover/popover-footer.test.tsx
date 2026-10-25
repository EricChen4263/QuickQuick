import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import ClipPopoverApp from "./ClipPopoverApp";

// 渲染测试环境无 Tauri 运行时：事件订阅、窗口操作、IPC 全部打桩
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn().mockResolvedValue(null),
  // 外观设置读取桩：默认值即生产默认（最近复制 / 40px / 1500ms / 加粗高亮）
  getAppearanceSettings: vi.fn().mockResolvedValue({
    sortOrder: "recent",
    imageHeightPx: 40,
    previewFirstDelayMs: 1500,
    highlightStyle: "bold",
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

import { listClipItems, pasteToFront, hideAndReturnFocus, copyClipToClipboard } from "../ipc/ipc-client";

const ITEMS = [
  { id: "id1", content: "first item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
  { id: "id2", content: "second item", kind: "text" as const, isFavorite: false, hasThumbnail: false, lastModifiedUtc: Date.now() },
];

beforeEach(() => {
  vi.clearAllMocks();
  mockOnFocusChanged.mockResolvedValue(() => {});
  mockHide.mockResolvedValue(undefined);
  vi.mocked(listClipItems).mockResolvedValue(ITEMS);
  vi.mocked(pasteToFront).mockResolvedValue({ outcome: "full_paste" });
  vi.mocked(hideAndReturnFocus).mockResolvedValue(undefined);
  vi.mocked(copyClipToClipboard).mockResolvedValue(undefined);
});

/** 渲染面板，返回搜索框与「底部条当前文本」的取文函数 */
async function renderPopover(): Promise<{ input: HTMLInputElement; footerText: () => string }> {
  const { container } = render(<ClipPopoverApp />);
  // 等 IPC 列表落地：否则 setItems/setSelectedId 会落在 act 之外，产生 act 告警
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
  if (container.querySelector(".popover-footer") === null) {
    throw new Error("未渲染出 .popover-footer");
  }
  return {
    input: screen.getByRole("searchbox") as HTMLInputElement,
    footerText: () => container.querySelector(".popover-footer")?.textContent ?? "",
  };
}

describe("PopoverFooter 快捷键徽标随修饰键切换（P2-11）", () => {
  it("默认显示默认层徽标，且含数字直达", async () => {
    const { footerText } = await renderPopover();
    const text = footerText();

    expect(text).toContain("↵");
    expect(text).toContain("粘贴");
    expect(text).toContain("1–0");
    expect(text).toContain("直达");
    expect(text).toContain("esc");
    // 默认层不该出现组合层专属键位
    expect(text).not.toContain("⌃W");
    expect(text).not.toContain("⌘↑↓");
  });

  it("按下 ⌃ 后出现 ⌃W，且不再堆叠默认层的数字直达", async () => {
    const { input, footerText } = await renderPopover();
    fireEvent.keyDown(input, { key: "Control", ctrlKey: true });

    const text = footerText();
    expect(text).toContain("⌃W");
    expect(text).toContain("⌃N/⌃P");
    expect(text).toContain("⌃U");
    expect(text).not.toContain("1–0");
    expect(text).not.toContain("esc");
  });

  it("释放 ⌃ 后恢复默认层", async () => {
    const { input, footerText } = await renderPopover();
    fireEvent.keyDown(input, { key: "Control", ctrlKey: true });
    expect(footerText()).toContain("⌃W");

    fireEvent.keyUp(input, { key: "Control", ctrlKey: false });

    const text = footerText();
    expect(text).toContain("1–0");
    expect(text).toContain("esc");
    expect(text).not.toContain("⌃W");
  });

  it("窗口失焦（blur）时把修饰键状态全部重置回默认层", async () => {
    const { input, footerText } = await renderPopover();
    fireEvent.keyDown(input, { key: "Control", ctrlKey: true });
    expect(footerText()).toContain("⌃W");

    act(() => {
      window.dispatchEvent(new Event("blur"));
    });

    const text = footerText();
    expect(text).toContain("1–0");
    expect(text).not.toContain("⌃W");
  });

  it("同时按 ⌃ 与 ⌘ 时 ⌘ 优先，只显示 ⌘ 层", async () => {
    const { input, footerText } = await renderPopover();
    fireEvent.keyDown(input, { key: "Control", ctrlKey: true });
    fireEvent.keyDown(input, { key: "Meta", metaKey: true });

    const text = footerText();
    expect(text).toContain("⌘↑↓");
    expect(text).toContain("首尾");
    expect(text).not.toContain("⌃W");
  });

  it("底部条仍是语义 footer + kbd 键帽结构（可访问性未被破坏）", async () => {
    await renderPopover();
    const footer = document.querySelector(".popover-footer");
    expect(footer?.tagName).toBe("FOOTER");
    const kbds = document.querySelectorAll(".popover-footer kbd");
    expect(kbds).toHaveLength(5);
    expect(kbds[0]?.textContent).toBe("↵");
  });
});
