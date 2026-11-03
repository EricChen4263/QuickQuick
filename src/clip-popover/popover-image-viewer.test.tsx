import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
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
    lastModifiedUtc: Date.now(),
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
  // 生产的 hideAndReturnFocus 是 async 函数，必须返回 Promise；否则第二次 Esc 调用 .catch 时抛错。
  mockHideAndReturnFocus.mockResolvedValue(undefined);
});

/** 渲染浮窗并选中图片条目：默认选中第一条，因此把图片条目放在第一位 */
async function renderAppWithImageSelected() {
  mockListClipItems.mockResolvedValue([IMAGE_ITEM, makeItem("txt-1", { content: "plain text" })]);
  render(<ClipPopoverApp />);
  // 预览区有分级延迟（首次 1500ms），等图片预览真正出现再继续
  await waitFor(() => {
    expect(screen.getByRole("img", { name: "图片预览" })).toBeDefined();
  }, { timeout: 3000 });
}

describe("浮窗：右侧预览图片的大图覆盖层", () => {
  it("点击右侧预览图片打开覆盖层，点击覆盖层关闭", async () => {
    await renderAppWithImageSelected();

    fireEvent.click(screen.getByRole("img", { name: "图片预览" }));
    expect(screen.getByRole("dialog", { name: "图片大图" })).toBeDefined();

    fireEvent.click(screen.getByRole("dialog", { name: "图片大图" }));
    expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
    expect(mockHideAndReturnFocus).not.toHaveBeenCalled();
  });

  it("列表里的缩略图点击不再打开大图，只负责选中", async () => {
    mockListClipItems.mockResolvedValue([makeItem("txt-1", { content: "plain text" }), IMAGE_ITEM]);
    render(<ClipPopoverApp />);
    const thumb = await screen.findByRole("img", { name: "缩略图" });

    fireEvent.click(thumb);

    expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
  });

  it("覆盖层打开时 Esc 只关覆盖层，不关闭整个浮窗；再按一次 Esc 才关浮窗", async () => {
    await renderAppWithImageSelected();
    fireEvent.click(screen.getByRole("img", { name: "图片预览" }));

    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Escape" });

    expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
    expect(mockHideAndReturnFocus).not.toHaveBeenCalled();

    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Escape" });
    await waitFor(() => {
      expect(mockHideAndReturnFocus).toHaveBeenCalledTimes(1);
    });
  });

  it("覆盖层打开后选中项移走，覆盖层收起", async () => {
    await renderAppWithImageSelected();
    fireEvent.click(screen.getByRole("img", { name: "图片预览" }));
    expect(screen.getByRole("dialog", { name: "图片大图" })).toBeDefined();

    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "ArrowDown" });

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "图片大图" })).toBeNull();
    });
  });
});
