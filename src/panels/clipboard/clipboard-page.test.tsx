import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ClipboardPage from "./ClipboardPage";

// Mock Tauri event API：渲染测试环境无 Tauri 运行时
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock IPC：渲染测试环境无 Tauri 运行时
vi.mock("../../ipc/ipc-client", () => ({
  listClipItems: vi.fn(),
  deleteClipItem: vi.fn(),
  toggleFavoriteClip: vi.fn(),
  pasteToFront: vi.fn(),
  openAccessibilitySettings: vi.fn(),
}));

import { listen } from "@tauri-apps/api/event";
import { CLIPBOARD_CHANGED_EVENT } from "../../ipc/events";
import {
  listClipItems,
  deleteClipItem,
  toggleFavoriteClip,
  pasteToFront,
  openAccessibilitySettings,
  type ClipItem,
} from "../../ipc/ipc-client";

const mockListen = vi.mocked(listen);

const mockListClipItems = vi.mocked(listClipItems);
const mockDeleteClipItem = vi.mocked(deleteClipItem);
const mockToggleFavoriteClip = vi.mocked(toggleFavoriteClip);
const mockPasteToFront = vi.mocked(pasteToFront);
const mockOpenAccessibilitySettings = vi.mocked(openAccessibilitySettings);

/** 测试用剪贴板条目数据 */
const MOCK_ITEMS: ClipItem[] = [
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
    mockPasteToFront.mockResolvedValue({ outcome: "full_paste" });
    mockOpenAccessibilitySettings.mockResolvedValue(undefined);
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

    // Act：展开类型筛选下拉并选 richtext（自定义 Select：点 trigger 再点选项）
    await user.click(screen.getByRole("button", { name: "类型筛选" }));
    await user.click(screen.getByRole("option", { name: "富文本" }));

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

  it("clipboard-page: 预览区点击删除按钮调用 deleteClipItem 并传正确 id", async () => {
    // Arrange：删除操作由预览区（ClipPreview）统一提供，列表行不再有删除按钮
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    const user = userEvent.setup();
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // Act：预览区的删除按钮（初始高亮 item-1）
    const previewRegion = screen.getByRole("region", { name: "预览" });
    const deleteButton = within(previewRegion).getByRole("button", { name: /删除/ });
    await user.click(deleteButton);

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
    // Arrange：listClipItems 成功，deleteClipItem 失败；删除操作由预览区触发
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockDeleteClipItem.mockRejectedValue(new Error("删除 IPC 失败"));
    const user = userEvent.setup();
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // Act：点击预览区的删除按钮
    const previewRegion = screen.getByRole("region", { name: "预览" });
    const deleteButton = within(previewRegion).getByRole("button", { name: /删除/ });
    await user.click(deleteButton);

    // Assert：操作错误提示出现在 DOM
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(screen.getByRole("alert").textContent).toMatch(/操作失败|失败/);
  });

  it("clipboard-page: 点击第二行后预览切换到第二条内容（Bug1 点击选中）", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // 初始高亮 0，预览显示 item-1
    const previewRegion = screen.getByRole("region", { name: "预览" });
    expect(within(previewRegion).getByText("Hello World")).toBeInTheDocument();

    // Act：通过文本内容找到 item-2 所在的 option 行并点击
    // （比 index 定位更稳：不受 OnboardCard 等无关元素影响）
    const listbox = screen.getByRole("listbox");
    const item2Row = within(listbox).getByText("富文本内容示例").closest("[role='option']");
    fireEvent.click(item2Row!);

    // Assert：预览切换到 item-2 内容
    await waitFor(() => {
      expect(within(previewRegion).getByText("富文本内容示例")).toBeInTheDocument();
    });
  });

  it("clipboard-page: 点击第三行后预览切换到第三条内容（Bug1 点击选中）", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    const previewRegion = screen.getByRole("region", { name: "预览" });

    // Act：通过文本内容找到 item-3 所在的 option 行并点击
    const listbox = screen.getByRole("listbox");
    const item3Row = within(listbox).getByText("Another text item").closest("[role='option']");
    fireEvent.click(item3Row!);

    // Assert：预览切换到 item-3
    await waitFor(() => {
      expect(within(previewRegion).getByText("Another text item")).toBeInTheDocument();
    });
  });

  it("clipboard-page: 点击「粘贴到前台」调用 pasteToFront(item.id)，full_paste 不显示提示", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockPasteToFront.mockResolvedValue({ outcome: "full_paste" });
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    const previewRegion = screen.getByRole("region", { name: "预览" });
    const pasteBtn = within(previewRegion).getByRole("button", { name: "粘贴到前台" });
    await user.click(pasteBtn);

    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("item-1");
    });
    expect(screen.queryByText(/已复制到剪贴板/)).not.toBeInTheDocument();
  });

  it("clipboard-page: pasteToFront 返回 write_back_only 时显示降级提示", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockPasteToFront.mockResolvedValue({ outcome: "write_back_only" });
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    const previewRegion = screen.getByRole("region", { name: "预览" });
    const pasteBtn = within(previewRegion).getByRole("button", { name: "粘贴到前台" });
    await user.click(pasteBtn);

    await waitFor(() => {
      expect(screen.getByText(/已复制到剪贴板/)).toBeInTheDocument();
    });
  });

  it("clipboard-page: pasteToFront IPC reject 时显示操作错误提示", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockPasteToFront.mockRejectedValue(new Error("paste IPC 失败"));
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    const previewRegion = screen.getByRole("region", { name: "预览" });
    const pasteBtn = within(previewRegion).getByRole("button", { name: "粘贴到前台" });
    await user.click(pasteBtn);

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
  });

  it("clipboard-page: OnboardCard「前往系统设置」调用 openAccessibilitySettings", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    localStorage.removeItem("qq-onboard-dismissed");
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    const settingsBtn = screen.getByRole("button", { name: "前往系统设置" });
    await user.click(settingsBtn);

    await waitFor(() => {
      expect(mockOpenAccessibilitySettings).toHaveBeenCalledTimes(1);
    });
  });

  it("clipboard-page: onTranslateItem prop 在点击一键翻译后以条目 content 被调用", async () => {
    // RED：ClipboardPage 尚不接受 onTranslateItem prop，此测试预期失败
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    const mockOnTranslateItem = vi.fn();
    const user = userEvent.setup();
    // onboardCard 隐藏：避免布局干扰
    localStorage.setItem("qq-onboard-dismissed", "true");
    render(<ClipboardPage onTranslateItem={mockOnTranslateItem} />);

    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // 预览区高亮 item-1（text 类型）——一键翻译按钮仅非图片项可见
    const previewRegion = screen.getByRole("region", { name: "预览" });
    const translateBtn = within(previewRegion).getByRole("button", { name: /一键翻译/ });
    await user.click(translateBtn);

    // Assert：onTranslateItem 被以 item-1.content 调用
    expect(mockOnTranslateItem).toHaveBeenCalledWith("Hello World");
  });

  it("clipboard-page: 删除操作成功后触发 listClipItems 重加载刷新列表", async () => {
    // 验证 handleDelete 的完整业务流：deleteClipItem 成功 → loadItems 重新拉取列表。
    // 这同时覆盖 I-02 修复后 cancelledRef 被正确传入 loadItems 的路径。
    // （竞态 guard 的"卸载后不 setState"行为在 React 18 + jsdom 下无可观测信号，
    //  由 tester 的代码结构变异验证守门。）
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    // 初始挂载调用了一次
    expect(mockListClipItems).toHaveBeenCalledTimes(1);

    // 触发删除
    const previewRegion = screen.getByRole("region", { name: "预览" });
    const deleteButton = within(previewRegion).getByRole("button", { name: /删除/ });
    await user.click(deleteButton);

    // delete 成功后 loadItems 被再次调用
    await waitFor(() => {
      expect(mockListClipItems).toHaveBeenCalledTimes(2);
    });
    expect(mockDeleteClipItem).toHaveBeenCalledWith("item-1");
  });

  it("clipboard-page: 收藏操作成功后触发 listClipItems 重加载刷新列表", async () => {
    // 验证 handleToggleFavorite 的完整业务流：toggleFavoriteClip 成功 → loadItems 重拉。
    // 同样覆盖 I-02 修复后 cancelledRef 被正确传入 loadItems 的路径。
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitFor(() => {
      expect(screen.getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
    });

    expect(mockListClipItems).toHaveBeenCalledTimes(1);

    // 触发收藏（item-1 isFavorite=false，点后应变 true）
    const favoriteButtons = screen.getAllByRole("button", { name: /收藏/ });
    await user.click(favoriteButtons[0]);

    await waitFor(() => {
      expect(mockListClipItems).toHaveBeenCalledTimes(2);
    });
    expect(mockToggleFavoriteClip).toHaveBeenCalledWith("item-1", true);
  });

  it("clipboard-page: 收到 clipboard-changed 事件后触发 listClipItems 重新加载", async () => {
    // Arrange：捕获 listen 注册的回调，以便后续手动触发
    let capturedCallback: (() => void) | undefined;
    mockListen.mockImplementation((_eventName, handler) => {
      capturedCallback = handler as () => void;
      return Promise.resolve(() => {});
    });
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);

    render(<ClipboardPage />);

    // 等待挂载时的初始加载完成（listClipItems 第 1 次调用）
    await waitFor(() => {
      expect(mockListClipItems).toHaveBeenCalledTimes(1);
    });
    // 确认已订阅正确的事件名
    expect(mockListen).toHaveBeenCalledWith(CLIPBOARD_CHANGED_EVENT, expect.any(Function));

    // Act：模拟后端触发 clipboard-changed 事件
    capturedCallback!();

    // Assert：listClipItems 被再次调用（第 2 次）以刷新列表
    await waitFor(() => {
      expect(mockListClipItems).toHaveBeenCalledTimes(2);
    });
  });
});
