import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { PopoverPreview } from "./PopoverPreview";
import { PREVIEW_WAITING_TEXT, type PreviewState } from "./preview-state";
import type { ClipItem } from "../ipc/ipc-client";
import { resetClipThumbnailCacheForTest } from "../panels/clipboard/clip-thumbnails";

// 缩略图字节不随列表下发，图片行渲染时才按 imageId 拉取，故这里要 mock 取图通道
vi.mock("../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn(),
}));

import { getClipThumbnail } from "../ipc/ipc-client";

const mockGetClipThumbnail = vi.mocked(getClipThumbnail);

beforeEach(() => {
  vi.clearAllMocks();
  resetClipThumbnailCacheForTest();
});

function makeTextItem(overrides: Partial<ClipItem> = {}): ClipItem {
  return {
    id: "text-1",
    content: "hello preview",
    kind: "text",
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: new Date("2026-06-02T10:00:00").getTime(),
    ...overrides,
  };
}

/** 以「有条目」态渲染：绝大多数用例只关心条目内容渲染 */
function renderItem(item: ClipItem) {
  return render(<PopoverPreview state={{ kind: "item", item }} />);
}

/** 以任意态渲染（占位 / 空态用例） */
function renderState(state: PreviewState) {
  return render(<PopoverPreview state={state} />);
}

describe("PopoverPreview 渲染", () => {
  it("无选中项（empty 态）时渲染空态文案「无选中项」", () => {
    renderState({ kind: "empty" });
    expect(screen.getByText("无选中项")).toBeDefined();
  });

  it("已选中但分级延迟未到点（waiting 态）时渲染中性占位，不渲染「无选中项」", () => {
    renderState({ kind: "waiting" });

    expect(screen.getByText("预览稍后显示")).toBeDefined();
    expect(screen.getByText(PREVIEW_WAITING_TEXT)).toBeDefined();
    expect(screen.queryByText("无选中项")).toBeNull();
    // 占位不暗示在加载本地数据
    expect(screen.queryByText(/加载/)).toBeNull();
  });

  it("文本条目渲染 content 内容", () => {
    renderItem(makeTextItem());
    expect(screen.getByText("hello preview")).toBeDefined();
  });

  it("图片条目带缩略图标志时按需拉取并渲染 img，src 与拉取结果一致", async () => {
    const thumbnail = "data:image/webp;base64,xxx";
    mockGetClipThumbnail.mockResolvedValue(thumbnail);
    const item = makeTextItem({
      id: "img-1",
      content: "",
      kind: "image",
      hasThumbnail: true,
      imageId: "img-id-1",
    });

    renderItem(item);

    // 拉取未完成前是占位，不当空白处理
    expect(screen.getByText("[图片]")).toBeDefined();
    await waitFor(() => {
      expect(screen.getByRole("img", { name: "图片预览" })).toHaveAttribute(
        "src",
        thumbnail
      );
    });
    expect(mockGetClipThumbnail).toHaveBeenCalledWith("img-id-1");
  });

  it("图片条目没有缩略图标志时渲染占位文字「[图片]」且不发起取图请求", () => {
    const item = makeTextItem({
      id: "img-2",
      content: "",
      kind: "image",
      hasThumbnail: false,
    });

    renderItem(item);

    expect(screen.getByText("[图片]")).toBeDefined();
    expect(mockGetClipThumbnail).not.toHaveBeenCalled();
  });

  it("收藏条目渲染「已收藏」badge", () => {
    renderItem(makeTextItem({ isFavorite: true }));
    expect(screen.getByText("已收藏")).toBeDefined();
  });

  it("非收藏条目不渲染「已收藏」badge", () => {
    renderItem(makeTextItem({ isFavorite: false }));
    expect(screen.queryByText("已收藏")).toBeNull();
  });

  it("popover_preview_renders_sanitized_richtext", () => {
    const item = makeTextItem({
      kind: "richtext",
      content: "fallback",
      htmlContent: "<b>hi</b><table><tr><td>x</td></tr></table>",
    });

    const { container } = renderItem(item);

    const content = container.querySelector(".popover-preview-content");
    expect(content).not.toBeNull();
    expect(content!.querySelector("b")).not.toBeNull();
    expect(content!.querySelector("table")).not.toBeNull();
    expect(content!.textContent).toContain("hi");
  });

  it("popover_preview_strips_malicious_html", () => {
    const item = makeTextItem({
      kind: "richtext",
      content: "fallback",
      htmlContent:
        '<img src=x onerror="alert(1)">点<script>alert(2)</script>击<a href="javascript:alert(3)">x</a><iframe src="javascript:alert(4)"></iframe>',
    });

    const { container } = renderItem(item);

    const content = container.querySelector(".popover-preview-content");
    expect(content).not.toBeNull();
    expect(content!.querySelector("script")).toBeNull();
    expect(content!.querySelector("iframe")).toBeNull();
    expect(content!.innerHTML.toLowerCase()).not.toContain("onerror");
    expect(content!.innerHTML.toLowerCase()).not.toContain("javascript:");
    expect(content!.textContent).toContain("点");
  });
});
