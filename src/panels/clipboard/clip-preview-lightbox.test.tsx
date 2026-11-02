import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ClipPreview } from "./ClipPreview";
import { resetClipThumbnailCacheForTest } from "./clip-thumbnails";
import type { ClipItem } from "../../ipc/ipc-client";

vi.mock("../../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn(),
  getClipImageOriginal: vi.fn(),
  getClipImagePreview: vi.fn(),
  copyClipToClipboard: vi.fn(),
}));

import { getClipImageOriginal, getClipImagePreview, getClipThumbnail } from "../../ipc/ipc-client";

const mockGetClipThumbnail = vi.mocked(getClipThumbnail);
const mockGetClipImageOriginal = vi.mocked(getClipImageOriginal);
const mockGetClipImagePreview = vi.mocked(getClipImagePreview);

const ORIGINAL_URL = "data:image/png;base64,ORIGINAL";
const PREVIEW_URL = "data:image/webp;base64,PREVIEW";

const NOOP = vi.fn();

function makeItem(id: string, overrides: Partial<ClipItem> = {}): ClipItem {
  return {
    id,
    content: `content-${id}`,
    kind: "text",
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: 1_700_000_000_000,
    ...overrides,
  };
}

const IMAGE_ITEM = makeItem("img-row", {
  content: "",
  kind: "image",
  hasThumbnail: true,
  imageId: "img-1",
});

function renderPreview(item: ClipItem | null) {
  return render(
    <ClipPreview
      item={item}
      onToggleFavorite={NOOP}
      onDelete={NOOP}
      onCopy={NOOP}
      onPasteToFront={NOOP}
      onTranslate={NOOP}
    />,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  resetClipThumbnailCacheForTest();
  mockGetClipThumbnail.mockResolvedValue(null);
  mockGetClipImageOriginal.mockResolvedValue(ORIGINAL_URL);
  mockGetClipImagePreview.mockResolvedValue(PREVIEW_URL);
});

describe("主窗口预览区：点击图片打开大图覆盖层", () => {
  it("点击预览图片打开覆盖层并显示大图", async () => {
    renderPreview(IMAGE_ITEM);
    const preview = await screen.findByRole("img", { name: "图片预览" });

    fireEvent.click(preview);

    expect(screen.getByRole("dialog", { name: "图片大图" })).toBeDefined();
    await waitFor(() => {
      expect(screen.getByRole("img", { name: "图片大图" })).toHaveAttribute("src", PREVIEW_URL);
    });
  });

  it("点击覆盖层关闭", async () => {
    renderPreview(IMAGE_ITEM);
    fireEvent.click(await screen.findByRole("img", { name: "图片预览" }));

    fireEvent.click(screen.getByRole("dialog", { name: "图片大图" }));

    expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
  });

  it("覆盖层打开时 Esc 只关覆盖层，不冒泡到宿主", async () => {
    renderPreview(IMAGE_ITEM);
    fireEvent.click(await screen.findByRole("img", { name: "图片预览" }));
    const hostListener = vi.fn();
    document.addEventListener("keydown", hostListener);

    fireEvent.keyDown(document.body, { key: "Escape" });

    document.removeEventListener("keydown", hostListener);
    expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
    expect(hostListener).not.toHaveBeenCalled();
  });

  it("切换到另一条目时覆盖层收起，不会盖着旧图", async () => {
    const { rerender } = renderPreview(IMAGE_ITEM);
    fireEvent.click(await screen.findByRole("img", { name: "图片预览" }));
    expect(screen.getByRole("dialog", { name: "图片大图" })).toBeDefined();

    rerender(
      <ClipPreview
        item={makeItem("txt-2")}
        onToggleFavorite={NOOP}
        onDelete={NOOP}
        onCopy={NOOP}
        onPasteToFront={NOOP}
        onTranslate={NOOP}
      />,
    );

    expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
  });

  it("切走再切回原图片条目，覆盖层不会自动重新打开", async () => {
    const { rerender } = renderPreview(IMAGE_ITEM);
    fireEvent.click(await screen.findByRole("img", { name: "图片预览" }));

    const other = makeItem("txt-2");
    const back = (item: ClipItem) => (
      <ClipPreview
        item={item}
        onToggleFavorite={NOOP}
        onDelete={NOOP}
        onCopy={NOOP}
        onPasteToFront={NOOP}
        onTranslate={NOOP}
      />
    );
    rerender(back(other));
    rerender(back(IMAGE_ITEM));

    expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
  });

  it("文本条目没有可点击的图片预览", () => {
    renderPreview(makeItem("txt-1"));

    expect(screen.queryByRole("img", { name: "图片预览" })).toBeNull();
    expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
  });
});
