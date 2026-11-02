import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { listen } from "@tauri-apps/api/event";
import { ROUTE_EVENT, TRANSLATE_SEED_EVENT, TRAY_CLEAR_UNPINNED_REQUEST_EVENT } from "./ipc/events";
import { clearUnpinnedHistory } from "./ipc/ipc-client";
import App from "./App";

// Mock Tauri API：渲染测试环境无 Tauri 运行时
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    hide: vi.fn().mockResolvedValue(undefined),
  }),
}));

// Mock IPC client：listClipItems / listTranslateHistory 返回永远 pending 的 Promise，
// 避免子页面挂载后异步 state 更新在 act() 边界外触发警告。
// app-shell 测试只验证导航结构，不关心各页数据。
// getTranslateProviders / getSelectedProvider / setSelectedProvider 需补齐，
// 否则 TranslatePage 挂载时 provider fetch reject 会产生未捕获错误。
vi.mock("./ipc/ipc-client", () => ({
  listClipItems: vi.fn().mockReturnValue(new Promise(() => {})),
  deleteClipItem: vi.fn().mockResolvedValue(undefined),
  toggleFavoriteClip: vi.fn().mockResolvedValue(undefined),
  // 托盘「清空未固定」链路：默认返回空计数，具体用例用 mockResolvedValueOnce 覆盖
  clearUnpinnedHistory: vi.fn().mockResolvedValue({ softDeleted: 0, purged: 0 }),
  translateText: vi.fn().mockReturnValue(new Promise(() => {})),
  listTranslateHistory: vi.fn().mockReturnValue(new Promise(() => {})),
  getTranslateProviders: vi.fn().mockReturnValue(new Promise(() => {})),
  getSelectedProvider: vi.fn().mockReturnValue(new Promise(() => {})),
  setSelectedProvider: vi.fn().mockResolvedValue(undefined),
  getLaunchOnLogin: vi.fn().mockReturnValue(new Promise(() => {})),
  setLaunchOnLogin: vi.fn().mockResolvedValue(undefined),
  getStayInTray: vi.fn().mockReturnValue(new Promise(() => {})),
  setStayInTray: vi.fn().mockResolvedValue(undefined),
  getAutoUpdate: vi.fn().mockReturnValue(new Promise(() => {})),
  setAutoUpdate: vi.fn().mockResolvedValue(undefined),
  // ClipboardPage 挂载即读外观设置：用 pending Promise 避免 act 外状态更新
  getAppearanceSettings: vi.fn().mockReturnValue(new Promise(() => {})),
}));

// Mock browser-api：隔离 clipboard / speechSynthesis，防止 jsdom 环境报错
vi.mock("./panels/translate/browser-api", () => ({
  writeToClipboard: vi.fn().mockResolvedValue(undefined),
  speakText: vi.fn(),
}));

// V4-F2-A06: 主窗口外壳渲染测试（jsdom + @testing-library/react）
describe("app-shell", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("app-shell: 左侧边栏渲染三个一级入口（剪贴板/翻译/设置）", () => {
    // Arrange & Act
    render(<App />);

    // Assert：通过 button role 精确匹配导航入口，避免与页面占位区文字冲突
    expect(screen.getByRole("button", { name: "剪贴板" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "翻译" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "设置" })).toBeInTheDocument();
  });

  it("app-shell: 默认激活剪贴板页，page-clipboard 可见、page-translate 不可见", () => {
    // Arrange & Act
    render(<App />);

    // Assert
    expect(screen.getByTestId("page-clipboard")).toBeVisible();
    expect(screen.queryByTestId("page-translate")).not.toBeVisible();
  });

  it("app-shell: 点击翻译后 page-translate 变为可见、page-clipboard 隐藏", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<App />);

    // Act
    await user.click(screen.getByRole("button", { name: "翻译" }));

    // Assert
    expect(screen.getByTestId("page-translate")).toBeVisible();
    expect(screen.queryByTestId("page-clipboard")).not.toBeVisible();
  });

  it("app-shell: 点击设置后 page-settings 变为可见", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<App />);

    // Act
    await user.click(screen.getByRole("button", { name: "设置" }));

    // Assert
    expect(screen.getByTestId("page-settings")).toBeVisible();
  });

  it("app-shell: 当前选中项有 aria-current 属性，默认选中剪贴板", () => {
    // Arrange & Act
    render(<App />);

    // Assert：剪贴板入口有 aria-current="page"
    const clipboardNav = screen.getByRole("button", { name: "剪贴板" });
    expect(clipboardNav).toHaveAttribute("aria-current", "page");
  });

  it("app-shell: 点击翻译后翻译入口获得 aria-current、剪贴板入口失去", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<App />);

    // 用 within(nav) 限定在主导航内查找，避免与 TranslatePage 内部同名按钮冲突
    const nav = screen.getByRole("navigation", { name: "主导航" });

    // Act
    await user.click(within(nav).getByRole("button", { name: "翻译" }));

    // Assert
    expect(within(nav).getByRole("button", { name: "翻译" })).toHaveAttribute("aria-current", "page");
    expect(within(nav).getByRole("button", { name: "剪贴板" })).not.toHaveAttribute("aria-current");
  });

  it("app-shell: 主导航 nav 使用 qq-topnav 类", () => {
    render(<App />);

    // 外壳重组后一级导航从 112px 常驻侧栏并入顶部栏，类名随之改为 qq-topnav
    const nav = screen.getByRole("navigation", { name: "主导航" });
    expect(nav).toHaveClass("qq-topnav");
  });

  it("app-shell: 三个导航入口按钮均使用 qq-nav-item 类", () => {
    render(<App />);

    // 用可访问名精确匹配三个导航入口，排除 nav 内的主题切换按钮（theme-seg 有自己的类）
    const nav = screen.getByRole("navigation", { name: "主导航" });
    const navEntries = ["剪贴板", "翻译", "设置"];
    for (const name of navEntries) {
      expect(within(nav).getByRole("button", { name })).toHaveClass("qq-nav-item");
    }
  });

  it("app-shell: 主导航只容纳三个一级入口，不留主题开关", () => {
    render(<App />);

    // 主题开关已从顶部栏移到「设置 › 外观」：它是设一次就不再动的偏好，
    // 常驻顶栏会与导航争夺注意力。这里断言导航里只剩三个入口按钮。
    const nav = screen.getByRole("navigation", { name: "主导航" });
    expect(within(nav).getAllByRole("button")).toHaveLength(3);
    expect(
      within(nav).getAllByRole("button").filter((btn) => btn.hasAttribute("aria-pressed"))
    ).toHaveLength(0);
  });

  // 托盘「打开设置」链路的前端一半：后端 emit(ROUTE_EVENT, "settings") 后，
  // 主窗必须落到设置页。这条断言是唯一能挡住「路由字面量写错 → 跳到剪贴板页」
  // 这类静默失效的地方，故必须精确断言可见页而不只是「有反应」。
  it("app-shell: 收到 settings 路由后切到设置页，而不是停在剪贴板页", async () => {
    // Arrange：渲染后从 listen mock 的调用记录里取出 route 监听器
    render(<App />);
    const registration = vi
      .mocked(listen)
      .mock.calls.find(([event]) => event === ROUTE_EVENT);
    if (registration === undefined) {
      throw new Error("App 未注册 route 事件监听，托盘「打开设置」会静默失效");
    }
    const handler = registration[1] as unknown as (event: {
      payload: string;
    }) => void;

    // Act：模拟后端 `open_settings` 的 emit
    await act(async () => {
      handler({ payload: "settings" });
    });

    // Assert
    expect(screen.getByTestId("page-settings")).toBeVisible();
    expect(screen.getByTestId("page-clipboard")).not.toBeVisible();
  });

  it("app-shell: 路由事件名与后端 tray.rs 的 ROUTE_EVENT 一致", () => {
    // Tauri 事件名跨语言无法编译期共享，Rust 侧 tray::tests::route_event_matches_frontend_contract
    // 钉同一字面量；两侧任一改动都会在这里或那里变红。
    expect(ROUTE_EVENT).toBe("route");
  });

  // ===== 托盘「清空未固定…」：二次确认 =====

  /** 从 listen mock 的调用记录里取出指定事件的处理器（App 注册监听后才有）。 */
  function listenerFor(eventName: string): (payload: unknown) => void {
    const registration = vi
      .mocked(listen)
      .mock.calls.find(([event]) => event === eventName);
    if (registration === undefined) {
      throw new Error(`App 未注册 ${eventName} 事件监听，对应菜单项会静默失效`);
    }
    return registration[1] as unknown as (payload: unknown) => void;
  }

  it("app-shell: 确认清空未固定后才调后端命令，并回显被清条目数", async () => {
    // Arrange：用户点「确定」。两个计数故意取不同值——回执说的是「非收藏项」条数，
    // 取错字段（用 purged 冒充 softDeleted）必须能被这条断言抓住。
    vi.mocked(clearUnpinnedHistory).mockResolvedValueOnce({ softDeleted: 3, purged: 5 });
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<App />);
    const handler = listenerFor(TRAY_CLEAR_UNPINNED_REQUEST_EVENT);

    // Act
    await act(async () => {
      handler({});
    });

    // Assert：先确认再删；确认文案必须写明「收藏项会保留」这一代价边界
    expect(confirmSpy).toHaveBeenCalledTimes(1);
    const confirmMessage = confirmSpy.mock.calls[0]?.[0];
    expect(confirmMessage).toContain("收藏项会保留");
    expect(clearUnpinnedHistory).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("clear-unpinned-result")).toHaveTextContent(
      "已清空 3 条非收藏项"
    );
  });

  it("app-shell: 用户取消时不调用后端命令，也不显示结果", async () => {
    // Arrange：用户点「取消」
    vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<App />);
    const handler = listenerFor(TRAY_CLEAR_UNPINNED_REQUEST_EVENT);

    // Act
    await act(async () => {
      handler({});
    });

    // Assert：不可撤销操作在取消路径上必须一行都不删
    expect(clearUnpinnedHistory).not.toHaveBeenCalled();
    expect(screen.queryByTestId("clear-unpinned-result")).not.toBeInTheDocument();
  });

  it("app-shell: 清空未固定事件名与后端 tray.rs 的 CLEAR_UNPINNED_REQUEST_EVENT 一致", () => {
    // 与 ROUTE_EVENT 同理：Rust 侧 clear_unpinned_request_event_matches_frontend_contract
    // 钉同一字面量，两侧任一改动都会变红，避免「点了没反应」。
    expect(TRAY_CLEAR_UNPINNED_REQUEST_EVENT).toBe("tray-clear-unpinned-request");
  });
});

describe("app-shell: 浮窗展开的待译文本（translate-seed）", () => {
  /** 从 listen mock 的调用记录里取出指定事件的处理器（App 注册监听后才有）。 */
  function listenerFor(eventName: string): (payload: unknown) => void {
    const registration = vi
      .mocked(listen)
      .mock.calls.find(([event]) => event === eventName);
    if (registration === undefined) {
      throw new Error(`App 未注册 ${eventName} 事件监听，展开按钮会静默失效`);
    }
    return registration[1] as unknown as (payload: unknown) => void;
  }

  it("app-shell: 收到 translate-seed 后切到翻译页（不依赖路由事件）", async () => {
    render(<App />);
    const handler = listenerFor(TRANSLATE_SEED_EVENT);

    await act(async () => {
      handler({ payload: { text: "Hello world" } });
    });

    expect(screen.getByTestId("page-translate")).toBeVisible();
    expect(screen.getByTestId("page-clipboard")).not.toBeVisible();
  });

  it("app-shell: translate-seed 的空白文本被忽略，不切页", async () => {
    render(<App />);
    const handler = listenerFor(TRANSLATE_SEED_EVENT);

    await act(async () => {
      handler({ payload: { text: "   " } });
    });

    expect(screen.getByTestId("page-clipboard")).toBeVisible();
    expect(screen.getByTestId("page-translate")).not.toBeVisible();
  });

  it("app-shell: translate-seed 事件名与前端常量保持稳定", () => {
    // 纯前端事件无 Rust 侧同名常量，这条断言把字面量钉住，改名时测试会红。
    expect(TRANSLATE_SEED_EVENT).toBe("translate-seed");
  });
});
