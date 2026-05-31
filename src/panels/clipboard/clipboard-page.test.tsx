import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ClipboardPage from "./ClipboardPage";

// Mock IPC：渲染测试环境无 Tauri 运行时
vi.mock("../../ipc/ipc-client", () => ({
  listClipItems: vi.fn(),
  deleteClipItem: vi.fn(),
  toggleFavoriteClip: vi.fn(),
}));

import {
  listClipItems,
  deleteClipItem,
  toggleFavoriteClip,
} from "../../ipc/ipc-client";

const mockListClipItems = vi.mocked(listClipItems);
const mockDeleteClipItem = vi.mocked(deleteClipItem);
const mockToggleFavoriteClip = vi.mocked(toggleFavoriteClip);

/** 测试用剪贴板条目数据 */
const MOCK_ITEMS = [
  {
    id: "item-1",
    content: "Hello World",
    kind: "text",
    isFavorite: false,
    lastModifiedUtc: 1000,
  },
  {
    id: "item-2",
    content: "富文本内容示例",
    kind: "richtext",
    isFavorite: true,
    lastModifiedUtc: 2000,
  },
  {
    id: "item-3",
    content: "Another text item",
    kind: "text",
    isFavorite: false,
    lastModifiedUtc: 3000,
  },
];

describe("clipboard-page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDeleteClipItem.mockResolvedValue(undefined);
    mockToggleFavoriteClip.mockResolvedValue(undefined);
  });

  it("clipboard-page: 挂载后调用 listClipItems 并渲染所有条目内容", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);

    // Act
    render(<ClipboardPage />);

    // Assert：等待异步取数完成，列表区至少出现各条目文本（用 getAllBy 允许列表+预览多实例）
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });
    expect(screen.getAllByText("富文本内容示例").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Another text item").length).toBeGreaterThanOrEqual(1);
  });

  it("clipboard-page: 搜索框输入过滤词后列表只剩匹配项", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    const user = userEvent.setup();
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // Act：在搜索框输入 "Hello"
    const searchInput = screen.getByRole("searchbox");
    await user.type(searchInput, "Hello");

    // Assert：富文本内容示例和 Another text item 不再出现于 DOM
    await waitFor(() => {
      expect(screen.queryByText("富文本内容示例")).not.toBeInTheDocument();
      expect(screen.queryByText("Another text item")).not.toBeInTheDocument();
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });
  });

  it("clipboard-page: 类型筛选选 richtext 后只剩 richtext 条目", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    const user = userEvent.setup();
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // Act：选择 richtext 筛选
    const filterSelect = screen.getByRole("combobox");
    await user.selectOptions(filterSelect, "richtext");

    // Assert：只剩富文本条目，纯文本条目消失
    await waitFor(() => {
      expect(screen.getAllByText("富文本内容示例").length).toBeGreaterThanOrEqual(1);
      expect(screen.queryByText("Hello World")).not.toBeInTheDocument();
      expect(screen.queryByText("Another text item")).not.toBeInTheDocument();
    });
  });

  it("clipboard-page: ArrowDown 键高亮下移，右侧预览内容随之变化", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // 初始高亮索引 0，预览应显示第一条内容
    const previewRegion = screen.getByRole("region", { name: "预览" });
    expect(within(previewRegion).getByText("Hello World")).toBeInTheDocument();

    // Act：按 ArrowDown
    const searchInput = screen.getByRole("searchbox");
    fireEvent.keyDown(searchInput, { key: "ArrowDown" });

    // Assert：高亮移到索引 1，预览变为第二条内容
    await waitFor(() => {
      expect(within(previewRegion).getByText("富文本内容示例")).toBeInTheDocument();
    });
  });

  it("clipboard-page: ArrowUp 键高亮上移，右侧预览内容随之变化", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    const searchInput = screen.getByRole("searchbox");
    const previewRegion = screen.getByRole("region", { name: "预览" });

    // 先移到索引 1
    fireEvent.keyDown(searchInput, { key: "ArrowDown" });
    await waitFor(() => {
      expect(within(previewRegion).getByText("富文本内容示例")).toBeInTheDocument();
    });

    // Act：再按 ArrowUp 回到索引 0
    fireEvent.keyDown(searchInput, { key: "ArrowUp" });

    // Assert：预览回到第一条
    await waitFor(() => {
      expect(within(previewRegion).getByText("Hello World")).toBeInTheDocument();
    });
  });

  it("clipboard-page: 点击收藏按钮调用 toggleFavoriteClip 并传正确参数", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    const user = userEvent.setup();
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // Act：点击 item-1 的收藏按钮（isFavorite=false，点后应变 true）
    const favoriteButtons = screen.getAllByRole("button", { name: /收藏/ });
    await user.click(favoriteButtons[0]);

    // Assert：调用 toggleFavoriteClip(id, true)
    expect(mockToggleFavoriteClip).toHaveBeenCalledWith("item-1", true);
  });

  it("clipboard-page: 点击删除按钮调用 deleteClipItem 并传正确 id", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    const user = userEvent.setup();
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // Act：点击第一个删除按钮（对应 item-1）
    const deleteButtons = screen.getAllByRole("button", { name: /删除/ });
    await user.click(deleteButtons[0]);

    // Assert：调用 deleteClipItem("item-1")
    expect(mockDeleteClipItem).toHaveBeenCalledWith("item-1");
  });

  it("clipboard-page: listClipItems 失败时显示错误提示而非崩溃", async () => {
    // Arrange
    mockListClipItems.mockRejectedValue(new Error("IPC error"));

    // Act
    render(<ClipboardPage />);

    // Assert：显示错误提示文案
    await screen.findByText(/加载失败/);
  });

  it("clipboard-page: toggleFavoriteClip IPC reject 时显示操作错误提示", async () => {
    // Arrange：listClipItems 成功，toggleFavoriteClip 失败
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockToggleFavoriteClip.mockRejectedValue(new Error("收藏 IPC 失败"));
    const user = userEvent.setup();
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // Act：点击 item-1 的收藏按钮
    const favoriteButtons = screen.getAllByRole("button", { name: /收藏/ });
    await user.click(favoriteButtons[0]);

    // Assert：操作错误提示出现在 DOM
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(screen.getByRole("alert").textContent).toMatch(/操作失败|失败/);
  });

  it("clipboard-page: deleteClipItem IPC reject 时显示操作错误提示", async () => {
    // Arrange：listClipItems 成功，deleteClipItem 失败
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockDeleteClipItem.mockRejectedValue(new Error("删除 IPC 失败"));
    const user = userEvent.setup();
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // Act：点击第一个删除按钮
    const deleteButtons = screen.getAllByRole("button", { name: /删除/ });
    await user.click(deleteButtons[0]);

    // Assert：操作错误提示出现在 DOM
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(screen.getByRole("alert").textContent).toMatch(/操作失败|失败/);
  });
});
