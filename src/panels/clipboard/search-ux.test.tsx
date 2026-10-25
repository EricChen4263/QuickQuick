import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import ClipboardPage from "./ClipboardPage";
import { SEARCH_DEBOUNCE_MS } from "./useDebouncedValue";

// Mock Tauri event API：渲染测试环境无 Tauri 运行时
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock IPC：渲染测试环境无 Tauri 运行时
vi.mock("../../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn().mockResolvedValue(null),
  // 外观设置读取桩：默认值即生产默认（最近复制 / 40px / 1500ms / 加粗高亮）
  getAppearanceSettings: vi.fn().mockResolvedValue({
    sortOrder: "recent",
    imageHeightPx: 40,
    previewFirstDelayMs: 1500,
    highlightStyle: "bold",
  }),
  getClipImageOriginal: vi.fn().mockResolvedValue(null),
  listClipItems: vi.fn(),
  deleteClipItem: vi.fn(),
  toggleFavoriteClip: vi.fn(),
  pasteToFront: vi.fn(),
  copyClipToClipboard: vi.fn(),
  openAccessibilitySettings: vi.fn(),
}));

// 搜索计数桩：仍然执行真实实现，只是额外记录「到底重新搜索了几次」
vi.mock("../../clip-popover/clip-search", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../clip-popover/clip-search")>();
  return { ...actual, searchClipsWithPlan: vi.fn(actual.searchClipsWithPlan) };
});

import { listClipItems, type ClipItem } from "../../ipc/ipc-client";
import * as clipSearchModule from "../../clip-popover/clip-search";

const mockListClipItems = vi.mocked(listClipItems);
const searchSpy = vi.mocked(clipSearchModule.searchClipsWithPlan);

/** 三条条目：两条含 hello（大小写不同），一条含中文 */
const ITEMS: ClipItem[] = [
  { id: "id1", content: "Hello World", kind: "text", isFavorite: false, hasThumbnail: false, lastModifiedUtc: 1000 },
  { id: "id2", content: "second item", kind: "text", isFavorite: false, hasThumbnail: false, lastModifiedUtc: 2000 },
  { id: "id3", content: "HELLO 剪贴板", kind: "text", isFavorite: false, hasThumbnail: false, lastModifiedUtc: 3000 },
];

/** 渲染并让 IPC 取数落地（fake timers 下不能用 waitFor 干等） */
async function renderWithItems(items: ClipItem[]): Promise<HTMLElement> {
  mockListClipItems.mockResolvedValue(items);
  const { container } = render(<ClipboardPage />);
  await act(async () => {});
  return container;
}

/** 只伪造定时器：保留真实微任务，IPC promise 才能正常落地 */
function useOnlyTimers(): void {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
}

/** 推进防抖计时并让 React 提交重渲染 */
async function advance(ms: number): Promise<void> {
  await act(async () => {
    vi.advanceTimersByTime(ms);
  });
}

/** 当前列表渲染出的行数 */
function rowCount(): number {
  return screen.getAllByRole("option").length;
}

beforeEach(() => {
  // clearAllMocks 只清调用记录，不动桩里保留的真实实现
  vi.clearAllMocks();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("主窗搜索：200ms 防抖", () => {
  it("输入后 200ms 内不重新搜索，满 200ms 才按新词搜索", async () => {
    useOnlyTimers();
    await renderWithItems(ITEMS);
    expect(rowCount()).toBe(3);
    const input = screen.getByRole("searchbox");
    searchSpy.mockClear();

    // Act 1：输入后只过 199ms
    fireEvent.change(input, { target: { value: "hel" } });
    await advance(SEARCH_DEBOUNCE_MS - 1);

    // Assert 1：没有触发任何重新搜索，列表仍是全量（输入框已即时更新）
    expect(input).toHaveValue("hel");
    expect(searchSpy).not.toHaveBeenCalled();
    expect(rowCount()).toBe(3);

    // Act 2：再走 1ms，防抖到期
    await advance(1);

    // Assert 2：只搜索一次，且用的是最新搜索词
    expect(searchSpy).toHaveBeenCalledTimes(1);
    const plan = searchSpy.mock.calls[0][1];
    expect(plan.tokens).toEqual(["hel"]);
    expect(rowCount()).toBe(2);
  });

  it("连续输入只按最后一次搜索", async () => {
    useOnlyTimers();
    await renderWithItems(ITEMS);
    const input = screen.getByRole("searchbox");
    searchSpy.mockClear();

    fireEvent.change(input, { target: { value: "h" } });
    await advance(100);
    fireEvent.change(input, { target: { value: "he" } });
    await advance(100);
    fireEvent.change(input, { target: { value: "hello" } });
    expect(searchSpy).not.toHaveBeenCalled();

    await advance(SEARCH_DEBOUNCE_MS);
    expect(searchSpy).toHaveBeenCalledTimes(1);
    expect(searchSpy.mock.calls[0][1].tokens).toEqual(["hello"]);
  });
});

describe("主窗搜索：命中高亮", () => {
  it("命中片段渲染为 <mark class=clip-hit>，数量与文本正确", async () => {
    useOnlyTimers();
    const container = await renderWithItems(ITEMS);
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "hello" } });
    await advance(SEARCH_DEBOUNCE_MS);

    // 两条命中（大小写不敏感），每条一个 mark
    expect(rowCount()).toBe(2);
    const marks = container.querySelectorAll("mark.clip-hit");
    expect(marks).toHaveLength(2);
    expect(marks[0]).toHaveTextContent("Hello");
    expect(marks[1]).toHaveTextContent("HELLO");
  });

  it("多词 AND：两个词都命中的条目才留下，且各自高亮", async () => {
    useOnlyTimers();
    const items: ClipItem[] = [
      { id: "both", content: "hello world", kind: "text", isFavorite: false, hasThumbnail: false, lastModifiedUtc: 1000 },
      { id: "one", content: "hello there", kind: "text", isFavorite: false, hasThumbnail: false, lastModifiedUtc: 2000 },
    ];
    const container = await renderWithItems(items);
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "hello world" } });
    await advance(SEARCH_DEBOUNCE_MS);

    expect(rowCount()).toBe(1);
    const marks = Array.from(container.querySelectorAll("mark.clip-hit"));
    expect(marks.map((mark) => mark.textContent)).toEqual(["hello", "world"]);
  });

  it("清空搜索后不再有高亮（区间随查询一起清空）", async () => {
    useOnlyTimers();
    const container = await renderWithItems(ITEMS);
    const input = screen.getByRole("searchbox");
    fireEvent.change(input, { target: { value: "hello" } });
    await advance(SEARCH_DEBOUNCE_MS);
    expect(container.querySelectorAll("mark.clip-hit")).toHaveLength(2);

    fireEvent.change(input, { target: { value: "" } });
    await advance(SEARCH_DEBOUNCE_MS);

    expect(container.querySelectorAll("mark.clip-hit")).toHaveLength(0);
    expect(rowCount()).toBe(3);
  });
});

describe("主窗搜索：HTML 特殊字符按纯文本渲染（XSS 红线）", () => {
  it("内容含 <script> 与 & 引号时不产生脚本节点，且高亮照常生效", async () => {
    useOnlyTimers();
    const dangerous = `<script>alert("x")</script> & 'q'`;
    const container = await renderWithItems([
      { id: "xss", content: dangerous, kind: "text", isFavorite: false, hasThumbnail: false, lastModifiedUtc: 1000 },
    ]);

    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "<scr" } });
    await advance(SEARCH_DEBOUNCE_MS);

    // Assert 1：没有被当成 HTML 注入
    expect(container.querySelector("script")).toBeNull();
    // Assert 2：原文按普通文本原样呈现（截断上限内未丢字符）
    expect(container.querySelector(".clip-text")?.textContent).toBe(dangerous);
    // Assert 3：命中片段仍然是文本节点，只被 <mark> 包裹
    const marks = container.querySelectorAll("mark.clip-hit");
    expect(marks).toHaveLength(1);
    expect(marks[0].textContent).toBe("<scr");
    // Assert 4：文本里的尖括号不能凭空造出元素——切分渲染时行内只该有一个 <mark>
    const textCell = container.querySelector(".clip-text");
    expect(textCell?.querySelectorAll("*")).toHaveLength(1);
  });

  it("非法正则降级后在界面上给出可读提示（不静默）", async () => {
    useOnlyTimers();
    await renderWithItems(ITEMS);

    // "foo[" 含元字符 → 走正则 → 编译失败 → 降级为文本搜索
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "foo[" } });
    await advance(SEARCH_DEBOUNCE_MS);

    expect(screen.getByRole("status")).toHaveTextContent("正则表达式无效");
  });
});
