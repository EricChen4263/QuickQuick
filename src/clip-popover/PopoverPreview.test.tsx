import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { PopoverPreview } from "./PopoverPreview";
import type { ClipItem } from "../ipc/ipc-client";

function makeTextItem(overrides: Partial<ClipItem> = {}): ClipItem {
  return {
    id: "text-1",
    content: "hello preview",
    kind: "text",
    isFavorite: false,
    lastModifiedUtc: new Date("2026-06-02T10:00:00").getTime(),
    ...overrides,
  };
}

describe("PopoverPreview 渲染", () => {
  it("无选中项时渲染空态文案「无选中项」", () => {
    render(<PopoverPreview item={null} />);
    expect(screen.getByText("无选中项")).toBeDefined();
  });

  it("文本条目渲染 content 内容", () => {
    render(<PopoverPreview item={makeTextItem()} />);
    expect(screen.getByText("hello preview")).toBeDefined();
  });

  it("图片条目有 thumbnailDataUrl 时渲染 img 元素，src 与 thumbnailDataUrl 一致", () => {
    const item = makeTextItem({
      id: "img-1",
      content: "",
      kind: "image",
      thumbnailDataUrl: "data:image/webp;base64,xxx",
    });

    render(<PopoverPreview item={item} />);

    const img = screen.getByRole("img", { name: "图片预览" });
    expect(img).toBeDefined();
    expect((img as HTMLImageElement).src).toContain("data:image/webp;base64,xxx");
  });

  it("图片条目无 thumbnailDataUrl 时渲染占位文字「[图片]」", () => {
    const item = makeTextItem({ id: "img-2", content: "", kind: "image" });

    render(<PopoverPreview item={item} />);

    expect(screen.getByText("[图片]")).toBeDefined();
  });

  it("收藏条目渲染「已收藏」badge", () => {
    render(<PopoverPreview item={makeTextItem({ isFavorite: true })} />);
    expect(screen.getByText("已收藏")).toBeDefined();
  });

  it("非收藏条目不渲染「已收藏」badge", () => {
    render(<PopoverPreview item={makeTextItem({ isFavorite: false })} />);
    expect(screen.queryByText("已收藏")).toBeNull();
  });
});
