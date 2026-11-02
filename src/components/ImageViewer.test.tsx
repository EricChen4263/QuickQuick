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
});
