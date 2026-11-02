import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ImageViewer } from "./ImageViewer";
import { resetClipThumbnailCacheForTest } from "../panels/clipboard/clip-thumbnails";

vi.mock("../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn(),
  getClipImagePreview: vi.fn(),
}));

import { getClipImagePreview, getClipThumbnail } from "../ipc/ipc-client";

const mockGetClipThumbnail = vi.mocked(getClipThumbnail);
const mockGetClipImagePreview = vi.mocked(getClipImagePreview);

const THUMB_URL = "data:image/webp;base64,THUMB";
const PREVIEW_URL = "data:image/webp;base64,PREVIEW";

beforeEach(() => {
  vi.clearAllMocks();
  resetClipThumbnailCacheForTest();
  mockGetClipThumbnail.mockResolvedValue(THUMB_URL);
  mockGetClipImagePreview.mockResolvedValue(PREVIEW_URL);
});

describe("ImageViewer 大图覆盖层", () => {
  it("优先显示大图预览", async () => {
    render(<ImageViewer imageId="img-1" onClose={() => undefined} />);

    await waitFor(() => {
      expect(screen.getByRole("img", { name: "图片大图" })).toHaveAttribute("src", PREVIEW_URL);
    });
    expect(mockGetClipImagePreview).toHaveBeenCalledWith("img-1");
  });

  it("预览取不到时回落缩略图", async () => {
    mockGetClipImagePreview.mockResolvedValue(null);

    render(<ImageViewer imageId="img-1" onClose={() => undefined} />);

    await waitFor(() => {
      expect(screen.getByRole("img", { name: "图片大图" })).toHaveAttribute("src", THUMB_URL);
    });
  });

  it("预览与缩略图都没有时显示 [图片] 占位，不渲染空 img", async () => {
    mockGetClipImagePreview.mockResolvedValue(null);
    mockGetClipThumbnail.mockResolvedValue(null);

    render(<ImageViewer imageId="img-1" onClose={() => undefined} />);

    await waitFor(() => {
      expect(mockGetClipImagePreview).toHaveBeenCalled();
    });
    expect(screen.getByText("[图片]")).toBeDefined();
    expect(screen.queryByRole("img", { name: "图片大图" })).toBeNull();
  });

  it("点击覆盖层任意处触发 onClose", () => {
    const onClose = vi.fn();
    render(<ImageViewer imageId="img-1" onClose={onClose} />);

    fireEvent.click(screen.getByRole("dialog", { name: "图片大图" }));

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("按 Esc 触发 onClose，并阻止事件继续冒泡到宿主", () => {
    const onClose = vi.fn();
    const hostListener = vi.fn();
    document.addEventListener("keydown", hostListener);
    render(<ImageViewer imageId="img-1" onClose={onClose} />);

    fireEvent.keyDown(document.body, { key: "Escape" });

    document.removeEventListener("keydown", hostListener);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(hostListener).not.toHaveBeenCalled();
  });

  it("非 Esc 按键不关闭覆盖层", () => {
    const onClose = vi.fn();
    render(<ImageViewer imageId="img-1" onClose={onClose} />);

    fireEvent.keyDown(document.body, { key: "ArrowDown" });

    expect(onClose).not.toHaveBeenCalled();
  });

  it("点击图片本身不关闭覆盖层", async () => {
    const onClose = vi.fn();
    render(<ImageViewer imageId="img-1" onClose={onClose} />);
    fireEvent.click(await screen.findByRole("img", { name: "图片大图" }));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("点击工具栏按钮不关闭覆盖层", async () => {
    const onClose = vi.fn();
    render(<ImageViewer imageId="img-1" onClose={onClose} />);
    await screen.findByRole("img", { name: "图片大图" });
    fireEvent.click(screen.getByRole("button", { name: "适应窗口" }));
    expect(onClose).not.toHaveBeenCalled();
  });
});

/** 模拟图片加载完成：jsdom 不解码图片，naturalWidth 需手动给定 */
async function loadImageWithNaturalWidth(width: number, height: number) {
  const img = await screen.findByRole("img", { name: "图片大图" });
  Object.defineProperty(img, "naturalWidth", { configurable: true, value: width });
  Object.defineProperty(img, "naturalHeight", { configurable: true, value: height });
  fireEvent.load(img);
  return img;
}

describe("ImageViewer 缩放与原比例", () => {
  it("图片加载前缩放按钮不可用", async () => {
    render(<ImageViewer imageId="img-1" onClose={() => undefined} />);
    await screen.findByRole("img", { name: "图片大图" });

    expect(screen.getByRole("button", { name: "放大" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "缩小" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "原比例" })).toBeDisabled();
  });

  it("点击原比例后图片宽度等于原始像素宽度", async () => {
    render(<ImageViewer imageId="img-1" onClose={() => undefined} />);
    await loadImageWithNaturalWidth(1600, 1200);

    fireEvent.click(screen.getByRole("button", { name: "原比例" }));

    expect(screen.getByRole("img", { name: "图片大图" })).toHaveStyle({ width: "1600px" });
    expect(screen.getByText("100%")).toBeDefined();
  });

  it("放大与缩小按一级步长变化，缩回后回到原比例", async () => {
    render(<ImageViewer imageId="img-1" onClose={() => undefined} />);
    await loadImageWithNaturalWidth(1600, 1200);
    fireEvent.click(screen.getByRole("button", { name: "原比例" }));

    fireEvent.click(screen.getByRole("button", { name: "放大" }));
    expect(screen.getByRole("img", { name: "图片大图" })).toHaveStyle({ width: "2000px" });

    fireEvent.click(screen.getByRole("button", { name: "缩小" }));
    expect(screen.getByRole("img", { name: "图片大图" })).toHaveStyle({ width: "1600px" });
  });

  it("点击适应窗口后移除内联宽度，回到适应模式", async () => {
    render(<ImageViewer imageId="img-1" onClose={() => undefined} />);
    const img = await loadImageWithNaturalWidth(1600, 1200);
    fireEvent.click(screen.getByRole("button", { name: "原比例" }));

    fireEvent.click(screen.getByRole("button", { name: "适应窗口" }));

    expect(img.style.width).toBe("");
    expect(screen.getByText("适应", { selector: "span" })).toBeDefined();
  });

  it("键盘 + 放大、- 缩小、1 原比例、0 适应，并且不关闭覆盖层", async () => {
    const onClose = vi.fn();
    render(<ImageViewer imageId="img-1" onClose={onClose} />);
    await loadImageWithNaturalWidth(1600, 1200);

    fireEvent.keyDown(document.body, { key: "1" });
    expect(screen.getByRole("img", { name: "图片大图" })).toHaveStyle({ width: "1600px" });

    fireEvent.keyDown(document.body, { key: "+" });
    expect(screen.getByRole("img", { name: "图片大图" })).toHaveStyle({ width: "2000px" });

    fireEvent.keyDown(document.body, { key: "-" });
    expect(screen.getByRole("img", { name: "图片大图" })).toHaveStyle({ width: "1600px" });

    fireEvent.keyDown(document.body, { key: "0" });
    expect(screen.getByRole("img", { name: "图片大图" }).style.width).toBe("");

    expect(onClose).not.toHaveBeenCalled();
  });

  it("新图加载后回到适应窗口，不沿用上一张的倍率", async () => {
    render(<ImageViewer imageId="img-1" onClose={() => undefined} />);
    await loadImageWithNaturalWidth(1600, 1200);
    fireEvent.click(screen.getByRole("button", { name: "原比例" }));

    const img = await loadImageWithNaturalWidth(800, 600);

    expect(img.style.width).toBe("");
  });
});

