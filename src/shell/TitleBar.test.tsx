import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
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
});
