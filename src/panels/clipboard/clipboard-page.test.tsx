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
  getClipThumbnail: vi.fn().mockResolvedValue(null),
  // 外观设置读取桩：默认值即生产默认（最近复制 / 40px / 1500ms / 加粗高亮）
  getAppearanceSettings: vi.fn().mockResolvedValue({
    sortOrder: "recent",
    imageHeightPx: 40,
    previewFirstDelayMs: 1500,
    highlightStyle: "bold",
  }),
  listClipItems: vi.fn(),
  deleteClipItem: vi.fn(),
  toggleFavoriteClip: vi.fn(),
  pasteToFront: vi.fn(),
  copyClipToClipboard: vi.fn(),
  openAccessibilitySettings: vi.fn(),
  // 首次引导用到的三个入口（自启偏好 / 历史热键 / 引导状态）
  getLaunchOnLogin: vi.fn().mockResolvedValue(true),
  setLaunchOnLogin: vi.fn().mockResolvedValue(undefined),
  getHotkeys: vi.fn().mockResolvedValue({
    history: "CmdOrCtrl+Shift+C",
    translate: "CmdOrCtrl+Shift+T",
    main: "CmdOrCtrl+Shift+M",
  }),
  getOnboardingStatus: vi.fn().mockResolvedValue({ action: "proceed", capability: "full_paste" }),
}));

import { listen } from "@tauri-apps/api/event";
import { CLIPBOARD_CHANGED_EVENT } from "../../ipc/events";
import {
  setLaunchOnLogin,
  listClipItems,
  deleteClipItem,
  toggleFavoriteClip,
  pasteToFront,
  copyClipToClipboard,
  openAccessibilitySettings,
  type ClipItem,
} from "../../ipc/ipc-client";

const mockListen = vi.mocked(listen);

const mockListClipItems = vi.mocked(listClipItems);
const mockDeleteClipItem = vi.mocked(deleteClipItem);
const mockToggleFavoriteClip = vi.mocked(toggleFavoriteClip);
const mockPasteToFront = vi.mocked(pasteToFront);
const mockCopyClipToClipboard = vi.mocked(copyClipToClipboard);
const mockOpenAccessibilitySettings = vi.mocked(openAccessibilitySettings);

/** 测试用剪贴板条目数据 */
const MOCK_ITEMS: ClipItem[] = [
  {
    id: "item-1",
    content: "Hello World",
    kind: "text",
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: 1000,
  },
  {
    id: "item-2",
    content: "富文本内容示例",
    kind: "richtext",
    isFavorite: true,
    hasThumbnail: false,
    lastModifiedUtc: 2000,
  },
  {
    id: "item-3",
    content: "Another text item",
    kind: "text",
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: 3000,
  },
];

/**
 * 等列表区真正渲染出至少一行，返回 listbox 元素。
 *
 * 不要用 `screen.getAllByText("Hello World")` 当就绪信号：挂载期 `ClipboardPage` 会把
 * renderCount 清零，列表可能在某一提交里是 0 行，而右侧预览区**已经**显示同一条内容——
 * 谓词被预览区提前满足，后续断言就落在中间提交上，跑起来随调度抖动（P6-2 登记的 flake）。
 * 这里把等待范围收进 listbox 子树，让「谓词」与「后续断言」在同一棵树上。
 */
async function waitForListReady(expectText = "Hello World"): Promise<HTMLElement> {
  const listbox = await screen.findByRole("listbox");
  await waitFor(() => {
    expect(within(listbox).getAllByText(expectText).length).toBeGreaterThanOrEqual(1);
  });
  return listbox;
}

describe("clipboard-page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDeleteClipItem.mockResolvedValue(undefined);
    mockToggleFavoriteClip.mockResolvedValue(undefined);
    mockPasteToFront.mockResolvedValue({ outcome: "full_paste" });
    mockCopyClipToClipboard.mockResolvedValue(undefined);
    mockOpenAccessibilitySettings.mockResolvedValue(undefined);
  });

  it("clipboard-page: 挂载后调用 listClipItems 并渲染所有条目内容", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);

    // Act
    render(<ClipboardPage />);

    // Assert：三条断言必须放在**同一个** waitFor 里，且范围收进 listbox。
    // 拆成「等第一条 → 断言其余两条」时，等待可能被右侧预览区提前满足
    // （挂载期列表先提交 0 行、预览已显示 Hello World），后续断言就会读到中间提交。
    const listbox = await screen.findByRole("listbox");
    await waitFor(() => {
      expect(within(listbox).getAllByText("Hello World").length).toBeGreaterThanOrEqual(1);
      expect(within(listbox).getAllByText("富文本内容示例").length).toBeGreaterThanOrEqual(1);
      expect(within(listbox).getAllByText("Another text item").length).toBeGreaterThanOrEqual(1);
    });
  });

  it("clipboard-page: 删空后清空选中并把窗口收回首屏块，回填不会一次性渲染几百行（P4-2）", async () => {
    // Arrange：300 条 → 用 ⌘↓ 跳到末条把窗口扩到 300
    const manyItems: ClipItem[] = Array.from({ length: 300 }, (_, index) => ({
      id: `bulk-${index}`,
      content: `批量条目 ${index}`,
      kind: "text" as const,
      isFavorite: false,
      hasThumbnail: false,
      lastModifiedUtc: 10_000 + index,
    }));
    mockListClipItems.mockResolvedValue(manyItems);
    const clipboardChanged = { fire: null as null | (() => void) };
    mockListen.mockImplementation((_eventName, handler) => {
      clipboardChanged.fire = handler as () => void;
      return Promise.resolve(() => {});
    });

    render(<ClipboardPage />);
    const listbox = await screen.findByRole("listbox");
    await waitFor(() => expect(within(listbox).getAllByRole("option")).toHaveLength(100));

    fireEvent.keyDown(screen.getByRole("searchbox"), {
      key: "ArrowDown",
      metaKey: true,
    });
    await waitFor(() => expect(within(listbox).getAllByRole("option")).toHaveLength(300));

    // Act 1：列表被删空（后端发 clipboard-changed，本次取回空列表）
    mockListClipItems.mockResolvedValue([]);
    clipboardChanged.fire?.();
    await waitFor(() => expect(within(listbox).queryAllByRole("option")).toHaveLength(0));

    // Act 2：回填同样的 300 条
    mockListClipItems.mockResolvedValue(manyItems);
    clipboardChanged.fire?.();
    await waitFor(() => expect(within(listbox).queryAllByRole("option").length).toBeGreaterThan(0));

    // Assert：窗口已收回首屏块（100），不是删空前扩大的 300。
    // 这条断言同时锁两件事：删空时 setRenderCount(initialRenderCount(0)) 与 setHighlightIndex(0)
    // ——去掉任一句都会让回填后渲染 300 行（后者通过「扩窗为包住旧 highlightIndex」实现）。
    expect(within(listbox).getAllByRole("option")).toHaveLength(100);
  });

  it("clipboard-page: 数据到达后的首个渲染批次即渲染全部行，不出现 0 行中间态（P4-2）", async () => {
    // Arrange：IPC 用「延迟 resolve」的 promise，先拿到挂载后的 listbox 再让数据到达
    let resolveItems!: (items: ClipItem[]) => void;
    mockListClipItems.mockReturnValue(
      new Promise<ClipItem[]>((resolve) => {
        resolveItems = resolve;
      }),
    );
    render(<ClipboardPage />);
    const listbox = await screen.findByRole("listbox");

    // 记录每次 DOM 变更后的行数：修复前「数据提交」那一帧渲染 0 行（窗口被压成 0），
    // 下一个提交才由 effect 补齐，于是首个批次是 0；修复后首个批次就是全部行。
    const snapshots: number[] = [];
    const observer = new MutationObserver(() => {
      snapshots.push(within(listbox).queryAllByRole("option").length);
    });
    observer.observe(listbox, { childList: true, subtree: true });

    // Act：让数据落地（这里刻意不包 act —— 需要让「数据提交」与「effect 补窗口的提交」
    // 分属两帧，否则两帧会被合并成一个批次，观察不到中间态）
    const silence = vi.spyOn(console, "error").mockImplementation(() => {});
    resolveItems(MOCK_ITEMS);
    await waitFor(() => {
      expect(within(listbox).getAllByRole("option")).toHaveLength(MOCK_ITEMS.length);
    });
    silence.mockRestore();
    observer.disconnect();

    // Assert：首个渲染批次就必须是全部行
    expect(snapshots.length).toBeGreaterThan(0);
    expect(snapshots[0]).toBe(MOCK_ITEMS.length);
  });

  it("clipboard-page: 搜索框输入过滤词后列表只剩匹配项", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    const user = userEvent.setup();
    render(<ClipboardPage />);
    const listbox = await waitForListReady();

    // Act：在搜索框输入 "Hello"
    const searchInput = screen.getByRole("searchbox");
    await user.type(searchInput, "Hello");

    // Assert：列表子树里只剩 Hello World 一行。
    // 注意：搜索态下命中片段被 <mark> 切成多个文本节点，getByText("Hello World") 反而找不到，
    // 之前那条断言能过只是因为**预览区**渲染了未切分的原文——正是 P6-2 flake 的同一类病。
    // 所以这里断言「行数 + 整行 textContent」，既不受切分影响，也不再借预览区的光。
    await waitFor(() => {
      const rows = within(listbox).getAllByRole("option");
      expect(rows).toHaveLength(1);
      expect(rows[0].textContent).toContain("Hello World");
      expect(listbox.textContent).not.toContain("富文本内容示例");
      expect(listbox.textContent).not.toContain("Another text item");
    });

    // 预览区也必须跟随筛选结果（不只是左侧列表）：搜索后预览显示命中的那条文案。
    // 反证方式：把预览来源从 filteredItems 换成未过滤的 items 时，这条会红。
    const previewRegion = screen.getByRole("region", { name: "预览" });
    await waitFor(() => {
      expect(within(previewRegion).getByText("Hello World")).toBeInTheDocument();
    });
  });

  it("clipboard-page: 类型筛选选 richtext 后只剩 richtext 条目", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    const user = userEvent.setup();
    render(<ClipboardPage />);
    const listbox = await waitForListReady();

    // Act：展开类型筛选下拉并选 richtext（自定义 Select：点 trigger 再点选项）
    await user.click(screen.getByRole("button", { name: "类型筛选" }));
    await user.click(screen.getByRole("option", { name: "富文本" }));

    // Assert：列表子树里只剩富文本一条（行数 + 整行 textContent，不借预览区的光）
    await waitFor(() => {
      const rows = within(listbox).getAllByRole("option");
      expect(rows).toHaveLength(1);
      expect(rows[0].textContent).toContain("富文本内容示例");
      expect(listbox.textContent).not.toContain("Hello World");
      expect(listbox.textContent).not.toContain("Another text item");
    });

    // 预览区同样要跟着筛选走：筛 richtext 后预览必须是那条富文本，而不是筛前的首条。
    // 反证：把预览来源从 filteredItems 换成未过滤的 items 时，这条会红。
    const previewRegion = screen.getByRole("region", { name: "预览" });
    await waitFor(() => {
      expect(within(previewRegion).getByText("富文本内容示例")).toBeInTheDocument();
    });
  });

  it("clipboard-page: ArrowDown 键高亮下移，右侧预览内容随之变化", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitForListReady();

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
    await waitForListReady();

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
    await waitForListReady();

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
    await waitForListReady();

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
    await waitForListReady();

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
    await waitForListReady();

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
    await waitForListReady();

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
    await waitForListReady();

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

    await waitForListReady();

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

    await waitForListReady();

    const previewRegion = screen.getByRole("region", { name: "预览" });
    const pasteBtn = within(previewRegion).getByRole("button", { name: "粘贴到前台" });
    await user.click(pasteBtn);

    await waitFor(() => {
      expect(screen.getByText(/已复制到剪贴板/)).toBeInTheDocument();
    });
  });

  it("clipboard-page: write_back_only 降级横幅含「打开辅助功能设置」按钮，点击触发 openAccessibilitySettings", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockPasteToFront.mockResolvedValue({ outcome: "write_back_only" });
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitForListReady();

    // 触发 write_back_only 降级
    const previewRegion = screen.getByRole("region", { name: "预览" });
    const pasteBtn = within(previewRegion).getByRole("button", { name: "粘贴到前台" });
    await user.click(pasteBtn);

    // 降级横幅出现后，定位其内的「打开辅助功能设置」按钮
    const banner = (await screen.findByText(/已复制到剪贴板/)).closest('[role="status"]') as HTMLElement;
    expect(banner).not.toBeNull();
    const openSettingsBtn = within(banner).getByRole("button", { name: "打开辅助功能设置" });

    // Act：点击按钮
    await user.click(openSettingsBtn);

    // Assert：openAccessibilitySettings 被调用
    await waitFor(() => {
      expect(mockOpenAccessibilitySettings).toHaveBeenCalledTimes(1);
    });
  });

  it("clipboard-page: write_back_only 降级横幅有 role=status（非紧急异步状态消息）", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockPasteToFront.mockResolvedValue({ outcome: "write_back_only" });
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitForListReady();

    const previewRegion = screen.getByRole("region", { name: "预览" });
    const pasteBtn = within(previewRegion).getByRole("button", { name: "粘贴到前台" });
    await user.click(pasteBtn);

    const banner = (await screen.findByText(/已复制到剪贴板/)).closest('[role="status"]');
    expect(banner).not.toBeNull();
  });

  it("clipboard-page: write_back_only 降级提示渲染在列表栏之前（顶部整行横幅，不挤塌两栏布局）", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockPasteToFront.mockResolvedValue({ outcome: "write_back_only" });
    const user = userEvent.setup();
    const { container } = render(<ClipboardPage />);

    await waitForListReady();

    const previewRegion = screen.getByRole("region", { name: "预览" });
    const pasteBtn = within(previewRegion).getByRole("button", { name: "粘贴到前台" });
    await user.click(pasteBtn);

    const pasteMsg = await screen.findByText(/已复制到剪贴板/);
    const listCol = container.querySelector(".clip-list-col");
    expect(listCol).not.toBeNull();
    // pasteMsg 必须在列表栏之前：DOCUMENT_POSITION_FOLLOWING(4) 表示 listCol 在 pasteMsg 之后
    const relation = pasteMsg.compareDocumentPosition(listCol as Node);
    expect(relation & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("clipboard-page: pasteToFront IPC reject 时显示操作错误提示", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockPasteToFront.mockRejectedValue(new Error("paste IPC 失败"));
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitForListReady();

    const previewRegion = screen.getByRole("region", { name: "预览" });
    const pasteBtn = within(previewRegion).getByRole("button", { name: "粘贴到前台" });
    await user.click(pasteBtn);

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
  });

  it("clipboard-page: 自启开关写盘失败时界面保持原值并显示后端原文（悲观更新，P5-8）", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    localStorage.removeItem("qq-onboard-finished");
    localStorage.removeItem("qq-onboard-dismissed");
    const backendMessage = "autostart.json 无法解析（JSON 语法错误），已拒绝写入以免覆盖你的设置";
    vi.mocked(setLaunchOnLogin).mockRejectedValueOnce(new Error(backendMessage));
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitForListReady();

    // 走到自启确认步骤：一路点「下一步」直到该按钮消失
    // （辅助功能步骤在已授权时会被自动跳过，具体点几次取决于授权状态，所以不写死次数）
    for (let step = 0; step < 4; step += 1) {
      const next = screen.queryByRole("button", { name: "下一步" });
      if (next === null) break;
      await user.click(next);
    }
    const toggle = await screen.findByRole("checkbox", { name: /登录时自动启动/ });
    expect(toggle).toBeChecked();

    // Act：点掉自启（后端拒写）
    await user.click(toggle);

    // Assert：悲观更新——界面从头到尾没动过（既没先变再回滚，也没停止在错误位置），
    // 后端原文如实显示，且只发生过这一次写请求（失败即不再重试）
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("已拒绝写入"));
    expect(screen.getByRole("checkbox", { name: /登录时自动启动/ })).toBeChecked();
    expect(setLaunchOnLogin).toHaveBeenCalledTimes(1);
    expect(setLaunchOnLogin).toHaveBeenCalledWith(false);
  });

  it("clipboard-page: 自启开关第一次失败后第二次成功，失败文案被清除（P5-8）", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    localStorage.removeItem("qq-onboard-finished");
    localStorage.removeItem("qq-onboard-dismissed");
    vi.mocked(setLaunchOnLogin).mockRejectedValueOnce(new Error("autostart 写入失败：已拒绝写入"));
    vi.mocked(setLaunchOnLogin).mockResolvedValueOnce(undefined);
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitForListReady();
    for (let step = 0; step < 4; step += 1) {
      const next = screen.queryByRole("button", { name: "下一步" });
      if (next === null) break;
      await user.click(next);
    }
    const toggle = await screen.findByRole("checkbox", { name: /登录时自动启动/ });

    // 第一次：失败 → 错误文案出现
    await user.click(toggle);
    expect(await screen.findByRole("alert")).toHaveTextContent("已拒绝写入");

    // 第二次：成功 → 旧失败文案必须消失（不是「一直挂着」）
    await user.click(screen.getByRole("checkbox", { name: /登录时自动启动/ }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("clipboard-page: 自启开关是悲观更新——后端没返回前界面不动（P5-8）", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    localStorage.removeItem("qq-onboard-finished");
    localStorage.removeItem("qq-onboard-dismissed");
    let resolveWrite!: () => void;
    vi.mocked(setLaunchOnLogin).mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveWrite = resolve;
      }),
    );
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitForListReady();
    for (let step = 0; step < 4; step += 1) {
      const next = screen.queryByRole("button", { name: "下一步" });
      if (next === null) break;
      await user.click(next);
    }
    const toggle = await screen.findByRole("checkbox", { name: /登录时自动启动/ });
    expect(toggle).toBeChecked();

    // Act：点掉开关，但后端还没返回
    await user.click(toggle);

    // Assert：悲观更新 → 界面仍是「勾选」；乐观实现此刻已经把它关掉了
    expect(screen.getByRole("checkbox", { name: /登录时自动启动/ })).toBeChecked();

    // 后端成功后界面才跟上
    resolveWrite();
    await waitFor(() =>
      expect(screen.getByRole("checkbox", { name: /登录时自动启动/ })).not.toBeChecked(),
    );
  });

  it("clipboard-page: 自启失败与页面操作失败并存，两条文案都可见（P5-8）", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    localStorage.removeItem("qq-onboard-finished");
    localStorage.removeItem("qq-onboard-dismissed");
    vi.mocked(setLaunchOnLogin).mockRejectedValueOnce(new Error("autostart 写入失败：已拒绝写入"));
    mockDeleteClipItem.mockRejectedValueOnce(new Error("删除失败：库被占用"));
    // 注：删除路径目前显示的是面板通用文案（未透出后端原文），这里只验证两来源并存
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitForListReady();
    for (let step = 0; step < 4; step += 1) {
      const next = screen.queryByRole("button", { name: "下一步" });
      if (next === null) break;
      await user.click(next);
    }

    // 1) 自启写失败
    await user.click(await screen.findByRole("checkbox", { name: /登录时自动启动/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("已拒绝写入");

    // 2) 预览区删除也失败（另一个来源）
    const previewRegion = screen.getByRole("region", { name: "预览" });
    await user.click(within(previewRegion).getByRole("button", { name: /删除/ }));

    // Assert：同一错误区里两条都在，谁也不遮谁
    const alert = await screen.findByRole("alert");
    await waitFor(() => expect(alert.textContent).toContain("操作失败"));
    expect(alert).toHaveTextContent("已拒绝写入");
    expect(within(alert).getByText(/已拒绝写入/)).toHaveAttribute(
      "data-error-source",
      "launch-on-login",
    );
    expect(within(alert).getByText(/操作失败/)).toHaveAttribute(
      "data-error-source",
      "page-operation",
    );
  });

  it("clipboard-page: 删除失败后再次删除成功，陈旧错误文案消失（P5-8）", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockDeleteClipItem.mockRejectedValueOnce(new Error("删除失败：库被占用"));
    const user = userEvent.setup();
    render(<ClipboardPage />);
    await waitForListReady();

    const previewRegion = screen.getByRole("region", { name: "预览" });
    const deleteButton = within(previewRegion).getByRole("button", { name: /删除/ });

    // 第一次删除失败 → 出错
    await user.click(deleteButton);
    expect(await screen.findByRole("alert")).toHaveTextContent("操作失败");

    // 第二次删除成功 → 陈旧错误必须消失（单槽位语义：成功即清）
    mockDeleteClipItem.mockResolvedValueOnce(undefined);
    await user.click(screen.getByRole("region", { name: "预览" }).querySelector("button[aria-label*='删除']") as HTMLElement);
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("clipboard-page: OnboardCard「前往系统设置」调用 openAccessibilitySettings", async () => {
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    localStorage.removeItem("qq-onboard-dismissed");
    const user = userEvent.setup();
    render(<ClipboardPage />);

    await waitForListReady();

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

    await waitForListReady();

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

    await waitForListReady();

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

    await waitForListReady();

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

// P2-7 主窗键盘流：Enter / ⌘1-9 必须真正走粘贴，而不是只移动高亮
describe("clipboard-page: 主窗键盘流", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDeleteClipItem.mockResolvedValue(undefined);
    mockToggleFavoriteClip.mockResolvedValue(undefined);
    mockPasteToFront.mockResolvedValue({ outcome: "full_paste" });
    mockCopyClipToClipboard.mockResolvedValue(undefined);
    mockOpenAccessibilitySettings.mockResolvedValue(undefined);
  });

  it("Enter 对高亮条目执行粘贴（pasteToFront 收到被高亮的 id）", async () => {
    // Arrange：初始高亮第一条 item-1
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitForListReady();

    // Act
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Enter" });

    // Assert
    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("item-1");
    });
  });

  it("ArrowDown 移动高亮后 Enter 粘贴被高亮的第二条", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitForListReady();
    const searchInput = screen.getByRole("searchbox");

    // Act
    fireEvent.keyDown(searchInput, { key: "ArrowDown" });
    fireEvent.keyDown(searchInput, { key: "Enter" });

    // Assert
    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("item-2");
    });
  });

  it("⌘3 直接粘贴第三条（不经过高亮移动）", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitForListReady();

    // Act
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "3", metaKey: true });

    // Assert
    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("item-3");
    });
  });

  it("⌘5 序号超出列表条数时不粘贴（不降级取邻近条目）", async () => {
    // Arrange：列表只有 3 条
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitForListReady();

    // Act
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "5", metaKey: true });

    // Assert
    await waitFor(() => {
      expect(mockListClipItems).toHaveBeenCalledTimes(1);
    });
    expect(mockPasteToFront).not.toHaveBeenCalled();
  });

  it("空列表按 Enter 不调用 pasteToFront", async () => {
    // Arrange：后端返回空列表
    mockListClipItems.mockResolvedValue([]);
    render(<ClipboardPage />);
    await screen.findByText("暂无记录");

    // Act
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Enter" });

    // Assert
    await waitFor(() => {
      expect(mockListClipItems).toHaveBeenCalledTimes(1);
    });
    expect(mockPasteToFront).not.toHaveBeenCalled();
  });

  it("Enter 粘贴返回 write_back_only 时仍显示降级提示横幅", async () => {
    // Arrange：走键盘路径，但降级提示沿用 handlePasteToFront 的既有逻辑
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockPasteToFront.mockResolvedValue({ outcome: "write_back_only" });
    render(<ClipboardPage />);
    await waitForListReady();

    // Act
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Enter" });

    // Assert
    await waitFor(() => {
      expect(screen.getByText(/已复制到剪贴板/)).toBeInTheDocument();
    });
    expect(mockPasteToFront).toHaveBeenCalledWith("item-1");
  });

  it("⌥Enter 只复制不粘贴（与 popover 的 ⌥Enter 语义一致）", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitForListReady();

    // Act
    const notCancelled = fireEvent.keyDown(screen.getByRole("searchbox"), {
      key: "Enter",
      altKey: true,
    });

    // Assert：复制走 IPC 保真路径，不触发前台粘贴
    expect(notCancelled).toBe(false);
    await waitFor(() => {
      expect(mockCopyClipToClipboard).toHaveBeenCalledWith("item-1");
    });
    expect(mockPasteToFront).not.toHaveBeenCalled();
    // 复制不经过 pasteToFront，因此不该出现 write_back_only（辅助功能降级）横幅
    expect(screen.queryByText(/已复制到剪贴板/)).not.toBeInTheDocument();
  });

  it("⌥Enter 复制失败时显示操作错误提示（不静默吞掉）", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockCopyClipToClipboard.mockRejectedValue(new Error("copy IPC 失败"));
    render(<ClipboardPage />);
    await waitForListReady();

    // Act
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Enter", altKey: true });

    // Assert
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(mockPasteToFront).not.toHaveBeenCalled();
  });

  it("Enter 与 ⌘3 阻止默认行为（不让浏览器再做一次默认动作）", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitForListReady();
    const input = screen.getByRole("searchbox");

    // Act & Assert：dispatchEvent 返回 false 表示 preventDefault 已生效
    expect(fireEvent.keyDown(input, { key: "Enter" })).toBe(false);
    expect(fireEvent.keyDown(input, { key: "3", metaKey: true })).toBe(false);
  });
});

// P2-6 输入法防御：主窗搜索框在组词期间不劫持方向键
describe("clipboard-page: 输入法防御", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDeleteClipItem.mockResolvedValue(undefined);
    mockToggleFavoriteClip.mockResolvedValue(undefined);
    mockPasteToFront.mockResolvedValue({ outcome: "full_paste" });
    mockCopyClipToClipboard.mockResolvedValue(undefined);
    mockOpenAccessibilitySettings.mockResolvedValue(undefined);
  });

  it("composition 期间按 ArrowDown 不改变高亮", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitForListReady();
    const previewRegion = screen.getByRole("region", { name: "预览" });

    // Act：组词中按方向键
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "ArrowDown", isComposing: true });

    // Assert：预览仍是第一条
    expect(within(previewRegion).getByText("Hello World")).toBeInTheDocument();
  });

  it("keyCode=229（WKWebView 组词表现）按 ArrowDown 同样不改变高亮", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitForListReady();
    const previewRegion = screen.getByRole("region", { name: "预览" });

    // Act
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "ArrowDown", keyCode: 229 });

    // Assert
    expect(within(previewRegion).getByText("Hello World")).toBeInTheDocument();
  });

  it("composition 结束后 ArrowDown 恢复高亮移动", async () => {
    // Arrange
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    render(<ClipboardPage />);
    await waitForListReady();
    const searchInput = screen.getByRole("searchbox");
    const previewRegion = screen.getByRole("region", { name: "预览" });

    // Act：先组词中按一次（无效），组词结束再按一次（生效）
    fireEvent.keyDown(searchInput, { key: "ArrowDown", isComposing: true });
    fireEvent.keyDown(searchInput, { key: "ArrowDown" });

    // Assert：高亮落到第二条
    await waitFor(() => {
      expect(within(previewRegion).getByText("富文本内容示例")).toBeInTheDocument();
    });
  });
});

// 边界探测（tester 补充，2026-06-08）

describe("clipboard-page: 边界探测 - 横幅 DOM 顺序", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDeleteClipItem.mockResolvedValue(undefined);
    mockToggleFavoriteClip.mockResolvedValue(undefined);
    mockPasteToFront.mockResolvedValue({ outcome: "full_paste" });
    mockCopyClipToClipboard.mockResolvedValue(undefined);
    mockOpenAccessibilitySettings.mockResolvedValue(undefined);
  });

  it("边界：opError 和 pasteMsg 同时出现时，两者均在 listCol 之前，opError 在 pasteMsg 之前", async () => {
    // 顺序刻意是「先 pasteMsg、后 opError」：粘贴成功会清 opError（单槽位语义：成功即清），
    // 所以只能先造出降级提示，再让后续操作失败产生 opError。
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockPasteToFront.mockResolvedValue({ outcome: "write_back_only" });
    mockToggleFavoriteClip.mockRejectedValue(new Error("IPC 失败"));
    const user = userEvent.setup();
    const { container } = render(<ClipboardPage />);

    await waitForListReady();

    // 先触发 pasteMsg（写入成功但只回写剪贴板 → 降级提示）
    const previewRegion = screen.getByRole("region", { name: "预览" });
    await user.click(within(previewRegion).getByRole("button", { name: "粘贴到前台" }));
    await screen.findByText(/已复制到剪贴板/);

    // 再触发 opError（收藏失败）
    const favoriteButtons = screen.getAllByRole("button", { name: /收藏/ });
    await user.click(favoriteButtons[0]);
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });

    const pasteMsgEl = await screen.findByText(/已复制到剪贴板/);
    const opErrorEl = screen.getByRole("alert");
    const listCol = container.querySelector(".clip-list-col") as Node;

    // 两条横幅都在 listCol 之前
    expect(pasteMsgEl.compareDocumentPosition(listCol) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(opErrorEl.compareDocumentPosition(listCol) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    // opError 在 pasteMsg 之前（opError 之后是 pasteMsg：DOCUMENT_POSITION_FOLLOWING）
    expect(opErrorEl.compareDocumentPosition(pasteMsgEl) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("边界：pasteMsg 为 null 时不渲染降级提示横幅", async () => {
    // 初始状态（未触发 write_back_only），横幅不存在
    mockListClipItems.mockResolvedValue(MOCK_ITEMS);
    mockPasteToFront.mockResolvedValue({ outcome: "full_paste" });
    render(<ClipboardPage />);

    await waitForListReady();

    expect(screen.queryByText(/已复制到剪贴板/)).not.toBeInTheDocument();
  });
});
