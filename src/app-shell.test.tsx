import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
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

    // Act
    await user.click(screen.getByRole("button", { name: "翻译" }));

    // Assert
    const translateNav = screen.getByRole("button", { name: "翻译" });
    const clipboardNav = screen.getByRole("button", { name: "剪贴板" });
    expect(translateNav).toHaveAttribute("aria-current", "page");
    expect(clipboardNav).not.toHaveAttribute("aria-current");
  });
});
