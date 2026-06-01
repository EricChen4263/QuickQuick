import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

  it("app-shell: 主导航 nav 使用 qq-sidebar 类", () => {
    render(<App />);

    // qq-sidebar 类赋予侧边栏版式与背景，验证样式类已挂载
    const nav = screen.getByRole("navigation", { name: "主导航" });
    expect(nav).toHaveClass("qq-sidebar");
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

  it("app-shell: 主题切换区渲染三个 aria-pressed 按钮", () => {
    render(<App />);

    // 校验 ThemeSwitch 三按钮存在且有 aria-pressed 属性（auto/light/dark）
    const nav = screen.getByRole("navigation", { name: "主导航" });
    const themeBtns = within(nav).getAllByRole("button").filter(
      (btn) => btn.hasAttribute("aria-pressed")
    );
    expect(themeBtns).toHaveLength(3);
  });
});
