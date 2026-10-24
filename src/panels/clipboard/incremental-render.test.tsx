import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import ClipboardPage from "./ClipboardPage";
import { RENDER_BLOCK_SIZE } from "./render-window";

// Mock Tauri event API：渲染测试环境无 Tauri 运行时
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock IPC：渲染测试环境无 Tauri 运行时
vi.mock("../../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn().mockResolvedValue(null),
  // 外观设置读取桩：默认值即生产默认（最近复制 / 24px / 1500ms / 背景高亮）
  getAppearanceSettings: vi.fn().mockResolvedValue({
    sortOrder: "recent",
    imageHeightPx: 24,
    previewFirstDelayMs: 1500,
    highlightStyle: "default",
  }),
  listClipItems: vi.fn(),
  deleteClipItem: vi.fn(),
  toggleFavoriteClip: vi.fn(),
  pasteToFront: vi.fn(),
  copyClipToClipboard: vi.fn(),
  openAccessibilitySettings: vi.fn(),
}));

import { listClipItems, pasteToFront, type ClipItem } from "../../ipc/ipc-client";

const mockListClipItems = vi.mocked(listClipItems);
const mockPasteToFront = vi.mocked(pasteToFront);

/** 列表可视区/内容的桩几何尺寸，用于模拟「滚到底部」与「停在顶部」 */
const SCROLL_TOP_METRICS = { scrollTop: 0, scrollHeight: 12000, clientHeight: 400 };
const NEAR_BOTTOM_METRICS = { scrollTop: 11400, scrollHeight: 12000, clientHeight: 400 };

/** 造 count 条纯文本历史条目（内容形如 `条目1`..`条目N`） */
function buildItems(count: number): ClipItem[] {
  return Array.from({ length: count }, (_, index) => ({
    id: `item-${index + 1}`,
    content: `条目${index + 1}`,
    kind: "text" as const,
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: 1000 + index,
  }));
}

/**
 * 造 count 条条目，前 tagged 条内容带「特殊」标记、其余带「普通」标记。
 * 用标记搜索就能把列表收窄到指定条数，验证渲染窗口的重置。
 */
function buildTaggedItems(count: number, tagged: number): ClipItem[] {
  return buildItems(count).map((item, index) => ({
    ...item,
    content: index < tagged ? `特殊-条目${index + 1}` : `普通-条目${index + 1}`,
  }));
}

/**
 * 造条目，让「甲组」与「乙组」两个搜索词各命中 hitCount 条。
 * 两个结果集命中条数相同、内容不同，用来钉住「重置判定看的是过滤条件本身，而不是命中条数」。
 */
function buildEqualHitItems(count: number, hitCount: number): ClipItem[] {
  return buildItems(count).map((item, index) => {
    if (index < hitCount) return { ...item, content: `甲组-条目${index + 1}` };
    if (index < hitCount * 2) return { ...item, content: `乙组-条目${index + 1}` };
    return item;
  });
}

/** 覆盖元素的滚动几何：jsdom 默认全部为 0，无法表达「已滚到底部」 */
function stubScrollMetrics(
  element: HTMLElement,
  metrics: { scrollTop: number; scrollHeight: number; clientHeight: number },
): void {
  for (const [key, value] of Object.entries(metrics)) {
    Object.defineProperty(element, key, { value, configurable: true });
  }
}

/** 取列表滚动容器（role=listbox，与类型筛选下拉的 listbox 区分开） */
function getListBox(): HTMLElement {
  return screen.getByRole("listbox", { name: "剪贴板历史" });
}

/** 当前 DOM 里渲染出的列表行 */
function getRenderedRows(): HTMLElement[] {
  return within(getListBox()).getAllByRole("option");
}

/** 渲染页面并等待异步取数落地（首条内容出现即视为加载完成） */
async function renderWithItems(items: ClipItem[]): Promise<void> {
  mockListClipItems.mockResolvedValue(items);
  render(<ClipboardPage />);
  await waitFor(() => {
    expect(getRenderedRows().length).toBeGreaterThan(0);
  });
}

describe("clipboard-page 列表增量渲染", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPasteToFront.mockResolvedValue({ outcome: "full_paste" });
  });

  it("250 条时首屏只渲染首块行数", async () => {
    // Arrange / Act
    await renderWithItems(buildItems(250));

    // Assert：DOM 行数被压到一块，超出部分根本不进 DOM
    expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE);
    expect(within(getListBox()).queryByText("条目101")).not.toBeInTheDocument();
  });

  it("滚动接近底部时追加一块，停在顶部时不追加", async () => {
    // Arrange
    await renderWithItems(buildItems(250));
    const listBox = getListBox();

    // Act 1：停在顶部滚动一次
    stubScrollMetrics(listBox, SCROLL_TOP_METRICS);
    fireEvent.scroll(listBox);

    // Assert 1：未进入追加区，窗口不变
    expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE);

    // Act 2：滚到底部附近再滚动一次
    stubScrollMetrics(listBox, NEAR_BOTTOM_METRICS);
    fireEvent.scroll(listBox);

    // Assert 2：追加一块
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE * 2);
    });
    expect(within(getListBox()).getByText("条目200")).toBeInTheDocument();
  });

  it("⌘↓ 跳到末条时把渲染窗口扩到末条，Enter 粘贴的也是末条", async () => {
    // Arrange
    await renderWithItems(buildItems(250));
    expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE);

    // Act：⌘↓ 跳末条
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "ArrowDown", metaKey: true });

    // Assert：末条真的进了 DOM 且处于选中态（不是只改了索引）
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(250);
    });
    const rows = getRenderedRows();
    expect(rows[249]).toHaveTextContent("条目250");
    expect(rows[249]).toHaveAttribute("aria-selected", "true");

    // Act：Enter 粘贴
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Enter" });

    // Assert：粘贴的是末条，证明选择与渲染扩窗同步
    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("item-250");
    });
  });

  it("↓ 跨过首块边界时按块扩窗，高亮行仍在 DOM 里", async () => {
    // Arrange：选中首块最后一条（索引 99）
    await renderWithItems(buildItems(250));
    const rows = getRenderedRows();
    expect(rows).toHaveLength(RENDER_BLOCK_SIZE);
    fireEvent.click(rows[RENDER_BLOCK_SIZE - 1]);

    // Act：↓ 走到第一条未渲染的记录（索引 100）
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "ArrowDown" });

    // Assert：窗口按块扩到 200，索引 100 那一行真的在 DOM 里且高亮
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE * 2);
    });
    const nextRows = getRenderedRows();
    expect(nextRows[100]).toHaveTextContent("条目101");
    expect(nextRows[100]).toHaveAttribute("aria-selected", "true");
  });

  it("搜索词变化后渲染窗口重置回首块", async () => {
    // Arrange：先滚到 200 行，再搜索一个能命中 120 条的词
    await renderWithItems(buildTaggedItems(250, 120));
    const listBox = getListBox();
    stubScrollMetrics(listBox, NEAR_BOTTOM_METRICS);
    fireEvent.scroll(listBox);
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE * 2);
    });

    // Act
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "特殊" } });

    // Assert：结果集 120 条，但窗口重置回一块（不重置会渲染满 120 条）
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE);
    });
  });

  it("命中条数相同的两个搜索词之间切换时窗口仍重置回首块", async () => {
    // Arrange：全量 250 条，先滚到 200 行
    await renderWithItems(buildEqualHitItems(250, 120));
    const listBox = getListBox();
    stubScrollMetrics(listBox, NEAR_BOTTOM_METRICS);
    fireEvent.scroll(listBox);
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE * 2);
    });

    // Act 1：搜「甲组」（命中 120 条）→ 窗口重置回一块，再滚满 120 行
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "甲组" } });
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE);
    });
    fireEvent.scroll(listBox);
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(120);
    });

    // Act 2：切到「乙组」——命中条数同样是 120，只有结果集内容变了
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "乙组" } });

    // Assert：窗口必须回到首块；若重置判定退化成「比对命中条数」就不会重置，会留下 120 行
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE);
    });
    // 顺带确认切换后的结果集确实换了（第一条是索引 120 的乙组条目）
    expect(getRenderedRows()[0]).toHaveTextContent("乙组-条目121");
  });

  it("选中项在窗口外很远时输入搜索把列表缩短，高亮回落到第一条且不崩", async () => {
    // Arrange：滚到 200 行，选中第 150 条
    await renderWithItems(buildTaggedItems(250, 3));
    const listBox = getListBox();
    stubScrollMetrics(listBox, NEAR_BOTTOM_METRICS);
    fireEvent.scroll(listBox);
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(RENDER_BLOCK_SIZE * 2);
    });
    fireEvent.click(getRenderedRows()[149]);
    expect(getRenderedRows()[149]).toHaveAttribute("aria-selected", "true");

    // Act：搜索把列表缩短到 3 条（选中索引 149 已越界）
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "特殊" } });

    // Assert：只剩 3 行，高亮回落到第一条，预览跟着第一条
    await waitFor(() => {
      expect(getRenderedRows()).toHaveLength(3);
    });
    const rows = getRenderedRows();
    expect(rows[0]).toHaveAttribute("aria-selected", "true");
    expect(rows[0]).toHaveTextContent("特殊-条目1");
    expect(within(screen.getByRole("region", { name: "预览" })).getByText("特殊-条目1")).toBeInTheDocument();
  });
});
