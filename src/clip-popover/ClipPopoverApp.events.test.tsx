import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import ClipPopoverApp from "./ClipPopoverApp";

// Mock Tauri event API：渲染测试环境无 Tauri 运行时。
// 捕获 listen 注册的回调，供测试手动触发 clipboard-changed。
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

import { listen } from "@tauri-apps/api/event";
import { CLIPBOARD_CHANGED_EVENT } from "../ipc/events";
import { listClipItems } from "../ipc/ipc-client";

const mockListen = vi.mocked(listen);
const mockListClipItems = vi.mocked(listClipItems);

/**
 * 按内容文本定位其所在的列表行（role=option），用于断言 aria-selected。
 * 限定在 listbox 内查找：内容同时出现在列表行与预览区，需排除预览区命中。
 */
function rowByText(text: string): HTMLElement {
  const listbox = screen.getByRole("listbox");
  const node = within(listbox).getByText(text).closest("[role='option']");
  if (node === null) {
    throw new Error(`未找到内容为「${text}」的列表行`);
  }
  return node as HTMLElement;
}

const INITIAL_ITEMS = [
  { id: "id1", content: "first item", kind: "text" as const, isFavorite: false, lastModifiedUtc: Date.now() },
];

const AFTER_CHANGE_ITEMS = [
  { id: "id2", content: "brand new item", kind: "text" as const, isFavorite: false, lastModifiedUtc: Date.now() },
  ...INITIAL_ITEMS,
];

beforeEach(() => {
  vi.clearAllMocks();
  mockOnFocusChanged.mockResolvedValue(() => {});
  mockHide.mockResolvedValue(undefined);
  mockListen.mockResolvedValue(() => {});
});

describe("ClipPopoverApp 订阅 clipboard-changed 自动刷新", () => {
  it("收到 clipboard-changed 事件后重新 listClipItems 并显示新项", async () => {
    // 捕获 listen 注册的回调，以便手动触发
    let capturedCallback: (() => void) | undefined;
    mockListen.mockImplementation((_eventName, handler) => {
      capturedCallback = handler as () => void;
      return Promise.resolve(() => {});
    });
    mockListClipItems
      .mockResolvedValueOnce(INITIAL_ITEMS)
      .mockResolvedValue(AFTER_CHANGE_ITEMS);

    render(<ClipPopoverApp />);

    // 初次挂载加载完成
    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });
    expect(mockListClipItems).toHaveBeenCalledTimes(1);

    // 已订阅正确事件名
    expect(mockListen).toHaveBeenCalledWith(CLIPBOARD_CHANGED_EVENT, expect.any(Function));

    // 模拟后端广播 clipboard-changed
    capturedCallback!();

    // 重新拉取列表（第 2 次），新项出现在列表中
    await waitFor(() => {
      expect(mockListClipItems).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      expect(screen.getAllByText("brand new item").length).toBeGreaterThan(0);
    });
  });

  it("卸载时调用 unlisten 取消订阅（防泄漏）", async () => {
    const mockUnlisten = vi.fn();
    mockListen.mockResolvedValue(mockUnlisten);
    mockListClipItems.mockResolvedValue(INITIAL_ITEMS);

    const { unmount } = render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });
    // 等待 listen 的 Promise resolve、unlisten 被存下
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith(CLIPBOARD_CHANGED_EVENT, expect.any(Function));
    });

    unmount();

    await waitFor(() => {
      expect(mockUnlisten).toHaveBeenCalledTimes(1);
    });
  });

  it("刷新不抢占当前选中：新项进入后选中仍为原 id1", async () => {
    let capturedCallback: (() => void) | undefined;
    mockListen.mockImplementation((_eventName, handler) => {
      capturedCallback = handler as () => void;
      return Promise.resolve(() => {});
    });
    mockListClipItems
      .mockResolvedValueOnce(INITIAL_ITEMS)
      .mockResolvedValue(AFTER_CHANGE_ITEMS);

    render(<ClipPopoverApp />);

    // 初次加载后默认选中第一项 id1（aria-selected=true，可观测）
    await waitFor(() => {
      expect(rowByText("first item")).toHaveAttribute("aria-selected", "true");
    });

    // 触发 clipboard-changed：新 id2 置顶 + 原 id1 仍在
    capturedCallback!();

    // 新项已进入列表
    await waitFor(() => {
      expect(rowByText("brand new item")).toBeInTheDocument();
    });

    // 选中仍为原 id1，新项 id2 未被抢占为选中
    expect(rowByText("first item")).toHaveAttribute("aria-selected", "true");
    expect(rowByText("brand new item")).toHaveAttribute("aria-selected", "false");
  });

  it("listen 在卸载后才 resolve：立即 unlisten 防泄漏", async () => {
    // listen 返回手动可控的 deferred，render 后不立即 resolve。
    // 模拟"订阅尚未注册成功就已卸载"的竞态：cleanup 跑时 unlisten 仍为 undefined，
    // 待 listen 之后才 resolve，组件 .then 的 cancelled 分支应立即调 fn() 取消，避免泄漏。
    const mockLateUnlisten = vi.fn();
    let resolveListen: ((fn: () => void) => void) | undefined;
    mockListen.mockReturnValue(
      new Promise<() => void>((resolve) => {
        resolveListen = resolve;
      }),
    );
    mockListClipItems.mockResolvedValue(INITIAL_ITEMS);

    const { unmount } = render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    // listen promise 尚未 resolve 时卸载
    unmount();

    // 卸载后订阅才注册成功：cancelled 分支命中，立即取消订阅
    resolveListen!(mockLateUnlisten);

    await waitFor(() => {
      expect(mockLateUnlisten).toHaveBeenCalledTimes(1);
    });
  });

  it("初始空列表 → 事件带来首条数据后自动选中该项", async () => {
    let capturedCallback: (() => void) | undefined;
    mockListen.mockImplementation((_eventName, handler) => {
      capturedCallback = handler as () => void;
      return Promise.resolve(() => {});
    });
    mockListClipItems
      .mockResolvedValueOnce([])
      .mockResolvedValue([
        { id: "new1", content: "fresh capture", kind: "text" as const, isFavorite: false, lastModifiedUtc: Date.now() },
      ]);

    render(<ClipPopoverApp />);

    // 初始空态：无任何 option，预览区显示空态
    await waitFor(() => {
      expect(screen.getByText("剪贴板暂无内容")).toBeInTheDocument();
    });
    expect(screen.queryAllByRole("option")).toHaveLength(0);

    // 触发事件带来首条数据
    capturedCallback!();

    // new1 出现并成为选中项（aria-selected=true，可观测）
    await waitFor(() => {
      expect(rowByText("fresh capture")).toBeInTheDocument();
    });
    expect(rowByText("fresh capture")).toHaveAttribute("aria-selected", "true");
  });
});
