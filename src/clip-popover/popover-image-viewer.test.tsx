import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { PopoverImageViewer } from "./PopoverImageViewer";
import { PopoverList } from "./PopoverList";
import { createInputOwnership } from "./keyboard-nav";
import type { ClipGroups } from "./grouping";
import type { ClipItem } from "../ipc/ipc-client";
import { resetClipThumbnailCacheForTest } from "../panels/clipboard/clip-thumbnails";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const mockHide = vi.fn();
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    hide: mockHide,
    onFocusChanged: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn(),
  getClipImagePreview: vi.fn(),
  getAppearanceSettings: vi.fn().mockResolvedValue({
    sortOrder: "recent",
    imageHeightPx: 40,
    previewFirstDelayMs: 1500,
    highlightStyle: "bold",
  }),
  listClipItems: vi.fn(),
  pasteToFront: vi.fn(),
  hideAndReturnFocus: vi.fn(),
  copyClipToClipboard: vi.fn(),
}));

vi.mock("../panels/translate/browser-api", () => ({
  writeToClipboard: vi.fn(),
}));

import { getClipImagePreview, getClipThumbnail, hideAndReturnFocus, listClipItems } from "../ipc/ipc-client";
import ClipPopoverApp from "./ClipPopoverApp";

const mockGetClipThumbnail = vi.mocked(getClipThumbnail);
const mockGetClipImagePreview = vi.mocked(getClipImagePreview);
const mockHideAndReturnFocus = vi.mocked(hideAndReturnFocus);
const mockListClipItems = vi.mocked(listClipItems);

const THUMB_URL = "data:image/webp;base64,THUMB";
const PREVIEW_URL = "data:image/webp;base64,PREVIEW";

function makeItem(id: string, overrides: Partial<ClipItem> = {}): ClipItem {
  return {
    id,
    content: `content-${id}`,
    kind: "text",
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: 1000,
    ...overrides,
  };
}

const IMAGE_ITEM = makeItem("img-row", {
  content: "",
  kind: "image",
  hasThumbnail: true,
  imageId: "img-1",
});

beforeEach(() => {
  vi.clearAllMocks();
  resetClipThumbnailCacheForTest();
  Element.prototype.scrollIntoView = vi.fn();
  mockGetClipThumbnail.mockResolvedValue(THUMB_URL);
  mockGetClipImagePreview.mockResolvedValue(PREVIEW_URL);
});

describe("PopoverImageViewer 覆盖层", () => {
  it("优先显示大图预览", async () => {
    render(<PopoverImageViewer imageId="img-1" onClose={() => undefined} />);

    await waitFor(() => {
      expect(screen.getByRole("img", { name: "图片大图" })).toHaveAttribute("src", PREVIEW_URL);
    });
    expect(mockGetClipImagePreview).toHaveBeenCalledWith("img-1");
  });

  it("预览取不到时回落缩略图", async () => {
    mockGetClipImagePreview.mockResolvedValue(null);

    render(<PopoverImageViewer imageId="img-1" onClose={() => undefined} />);

    await waitFor(() => {
      expect(screen.getByRole("img", { name: "图片大图" })).toHaveAttribute("src", THUMB_URL);
    });
  });

  it("预览与缩略图都没有时显示 [图片] 占位，不渲染空 img", async () => {
    mockGetClipImagePreview.mockResolvedValue(null);
    mockGetClipThumbnail.mockResolvedValue(null);

    render(<PopoverImageViewer imageId="img-1" onClose={() => undefined} />);

    await waitFor(() => {
      expect(mockGetClipImagePreview).toHaveBeenCalled();
    });
    expect(screen.getByText("[图片]")).toBeDefined();
    expect(screen.queryByRole("img", { name: "图片大图" })).toBeNull();
  });

  it("点击覆盖层任意处触发 onClose", () => {
    const onClose = vi.fn();
    render(<PopoverImageViewer imageId="img-1" onClose={onClose} />);

    fireEvent.click(screen.getByRole("dialog", { name: "图片大图" }));

    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

describe("PopoverList 缩略图点击", () => {
  it("点击图片行的缩略图同时请求打开大图并选中该行", async () => {
    const groups: ClipGroups = { favorites: [], today: [IMAGE_ITEM], earlier: [] };
    const onSelect = vi.fn();
    const onOpenImage = vi.fn();

    render(
      <PopoverList
        groups={groups}
        selectedId={null}
        onSelect={onSelect}
        onOpenImage={onOpenImage}
        ownership={createInputOwnership()}
      />
    );

    const thumb = await screen.findByRole("img", { name: "缩略图" });
    fireEvent.click(thumb);

    expect(onOpenImage).toHaveBeenCalledWith(IMAGE_ITEM);
    expect(onSelect).toHaveBeenCalledWith(IMAGE_ITEM);
  });

  it("点击文本行不触发打开大图", () => {
    const groups: ClipGroups = { favorites: [], today: [makeItem("t1")], earlier: [] };
    const onOpenImage = vi.fn();

    render(
      <PopoverList
        groups={groups}
        selectedId={null}
        onSelect={() => undefined}
        onOpenImage={onOpenImage}
        ownership={createInputOwnership()}
      />
    );

    fireEvent.click(screen.getByText("content-t1"));

    expect(onOpenImage).not.toHaveBeenCalled();
  });
});

describe("ClipPopoverApp 大图覆盖层的交互", () => {
  async function renderAppWithImageRow() {
    mockListClipItems.mockResolvedValue([makeItem("txt-1", { content: "plain text" }), IMAGE_ITEM]);
    render(<ClipPopoverApp />);
    const thumb = await screen.findByRole("img", { name: "缩略图" });
    return thumb;
  }

  it("点击缩略图打开覆盖层，点击覆盖层关闭", async () => {
    const thumb = await renderAppWithImageRow();

    fireEvent.click(thumb);
    expect(screen.getByRole("dialog", { name: "图片大图" })).toBeDefined();

    fireEvent.click(screen.getByRole("dialog", { name: "图片大图" }));
    expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
    expect(mockHideAndReturnFocus).not.toHaveBeenCalled();
  });

  it("覆盖层打开时 Esc 只关覆盖层，不关闭整个浮窗；再按一次 Esc 才关浮窗", async () => {
    const thumb = await renderAppWithImageRow();
    fireEvent.click(thumb);

    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Escape" });

    expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
    expect(mockHideAndReturnFocus).not.toHaveBeenCalled();

    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Escape" });
    await waitFor(() => {
      expect(mockHideAndReturnFocus).toHaveBeenCalledTimes(1);
    });
  });

  it("覆盖层打开后选中项移走，覆盖层收起", async () => {
    const thumb = await renderAppWithImageRow();
    fireEvent.click(thumb);
    expect(screen.getByRole("dialog", { name: "图片大图" })).toBeDefined();

    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "ArrowUp" });

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
    });
  });
});
