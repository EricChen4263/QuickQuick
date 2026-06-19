import { describe, it, expect, vi, afterEach } from "vitest";
import { render } from "@testing-library/react";

// 在 import TitleBar 前 mock @tauri-apps/api/window：模块级平台常量在 import 时求值，
// Windows 分支会引用 getCurrentWindow，必须先备好 mock 再加载组件。
const minimize = vi.fn();
const toggleMaximize = vi.fn();
const close = vi.fn();
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ minimize, toggleMaximize, close }),
}));

import { TitleBar } from "./TitleBar";

describe("TitleBar", () => {
  it("渲染品牌标题文字 QuickQuick，根容器包含完整 QuickQuick 文字", () => {
    render(<TitleBar />);
    const bar = document.querySelector(".qq-titlebar");
    expect(bar?.textContent).toBe("QuickQuick");
  });

  it("brand-accent span 存在且包含首个 Quick", () => {
    render(<TitleBar />);
    const accentSpan = document.querySelector(".brand-accent");
    expect(accentSpan).not.toBeNull();
    expect(accentSpan?.textContent).toBe("Quick");
  });

  it("标题栏根元素有 data-tauri-drag-region 属性", () => {
    render(<TitleBar />);
    const bar = document.querySelector("[data-tauri-drag-region]");
    expect(bar).not.toBeNull();
  });

  it("标题栏使用 qq-titlebar 类", () => {
    render(<TitleBar />);
    const bar = document.querySelector(".qq-titlebar");
    expect(bar).not.toBeNull();
  });

  it("默认（非 Windows）不渲染窗口控制按钮，不加 win 修饰类", () => {
    render(<TitleBar />);
    expect(document.querySelector(".qq-titlebar--win")).toBeNull();
    expect(document.querySelector(".qq-titlebar-controls")).toBeNull();
  });
});

describe("TitleBar on Windows", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  /// 平台常量在模块 import 时由 navigator.userAgent 求值，故须先打桩 UA、resetModules
  /// 再动态 re-import，才能命中 Windows 分支。
  async function renderWindowsTitleBar() {
    vi.stubGlobal("navigator", { userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)" });
    vi.resetModules();
    const mod = await import("./TitleBar");
    return render(<mod.TitleBar />);
  }

  it("Windows 下渲染最小化/最大化/关闭三个窗口控制按钮", async () => {
    await renderWindowsTitleBar();
    const controls = document.querySelector(".qq-titlebar-controls");
    expect(controls).not.toBeNull();
    expect(controls?.querySelectorAll("button").length).toBe(3);
  });

  it("Windows 下根容器带 qq-titlebar--win 修饰类", async () => {
    await renderWindowsTitleBar();
    expect(document.querySelector(".qq-titlebar--win")).not.toBeNull();
  });

  it("Windows 下窗口控制按钮容器不带 data-tauri-drag-region（避免点击被当拖动）", async () => {
    await renderWindowsTitleBar();
    const controls = document.querySelector(".qq-titlebar-controls");
    expect(controls?.hasAttribute("data-tauri-drag-region")).toBe(false);
  });
});
