import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import { PopoverPreview } from "./PopoverPreview";
import type { ClipItem } from "../ipc/ipc-client";
import { resetClipThumbnailCacheForTest } from "../panels/clipboard/clip-thumbnails";

// 图片条目的两路数据源：缩略图（列表同款，先到）与大图预览（按需生成，后到）
vi.mock("../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn(),
  getClipImagePreview: vi.fn(),
}));

import { getClipImagePreview, getClipThumbnail } from "../ipc/ipc-client";

const mockGetClipThumbnail = vi.mocked(getClipThumbnail);
const mockGetClipImagePreview = vi.mocked(getClipImagePreview);

const THUMB_URL = "data:image/webp;base64,THUMB";
const PREVIEW_URL = "data:image/webp;base64,PREVIEW";

/** 可控的 Promise：测试决定何时结算，避免用「等一会儿」猜测时序 */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function makeImageItem(overrides: Partial<ClipItem> = {}): ClipItem {
  return {
    id: "img-item",
    content: "",
    kind: "image",
    isFavorite: false,
    hasThumbnail: true,
    imageId: "img-id-1",
    lastModifiedUtc: new Date("2026-06-02T10:00:00").getTime(),
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  resetClipThumbnailCacheForTest();
  mockGetClipThumbnail.mockResolvedValue(THUMB_URL);
  mockGetClipImagePreview.mockResolvedValue(null);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("Popover 图片预览回落链", () => {
  it("大图预览就绪后替换缩略图，并按 imageId 请求预览", async () => {
    mockGetClipImagePreview.mockResolvedValue(PREVIEW_URL);

    render(<PopoverPreview state={{ kind: "item", item: makeImageItem() }} />);

    await waitFor(() => {
      expect(screen.getByRole("img", { name: "图片预览" })).toHaveAttribute(
        "src",
        PREVIEW_URL
      );
    });
    expect(mockGetClipImagePreview).toHaveBeenCalledWith("img-id-1");
  });

  it("预览生成失败时回落缩略图，并留下诊断日志", async () => {
    const preview = deferred<string | null>();
    mockGetClipImagePreview.mockReturnValue(preview.promise);
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);

    render(<PopoverPreview state={{ kind: "item", item: makeImageItem() }} />);
    await waitFor(() => {
      expect(screen.getByRole("img", { name: "图片预览" })).toHaveAttribute(
        "src",
        THUMB_URL
      );
    });

    await act(async () => {
      preview.reject(new Error("解码失败"));
    });

    // 等 warn 出现即表示 hook 的失败分支已结算，之后 src 只能是缩略图
    await waitFor(() => expect(warn).toHaveBeenCalled());
    expect(screen.getByRole("img", { name: "图片预览" })).toHaveAttribute(
      "src",
      THUMB_URL
    );
  });

  it("预览返回 null（降级图无原图）时回落缩略图且不报错", async () => {
    const preview = deferred<string | null>();
    mockGetClipImagePreview.mockReturnValue(preview.promise);
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);

    render(<PopoverPreview state={{ kind: "item", item: makeImageItem() }} />);
    await act(async () => {
      preview.resolve(null);
    });

    expect(screen.getByRole("img", { name: "图片预览" })).toHaveAttribute(
      "src",
      THUMB_URL
    );
    expect(warn).not.toHaveBeenCalled();
  });

  it("预览与缩略图都没有时保持 [图片] 占位，不渲染空 img", async () => {
    const preview = deferred<string | null>();
    mockGetClipImagePreview.mockReturnValue(preview.promise);
    mockGetClipThumbnail.mockResolvedValue(null);

    render(<PopoverPreview state={{ kind: "item", item: makeImageItem() }} />);
    await act(async () => {
      preview.resolve(null);
    });

    expect(screen.getByText("[图片]")).toBeDefined();
    expect(screen.queryByRole("img", { name: "图片预览" })).toBeNull();
  });

  it("没有 imageId 的图片条目不发起预览请求", () => {
    render(
      <PopoverPreview
        state={{
          kind: "item",
          item: makeImageItem({ imageId: undefined, hasThumbnail: false }),
        }}
      />
    );

    expect(mockGetClipImagePreview).not.toHaveBeenCalled();
  });
});

describe("Popover 图片预览的竞态与卸载", () => {
  it("切换到另一张图后，上一张迟到的预览不会覆盖当前图片", async () => {
    const previewA = deferred<string | null>();
    mockGetClipThumbnail.mockImplementation(async (id: string) =>
      id === "img-A" ? "data:image/webp;base64,THUMB_A" : "data:image/webp;base64,THUMB_B"
    );
    mockGetClipImagePreview.mockImplementation((id: string) =>
      id === "img-A" ? previewA.promise : Promise.resolve("data:image/webp;base64,PREVIEW_B")
    );
    const itemA = makeImageItem({ id: "a", imageId: "img-A" });
    const itemB = makeImageItem({ id: "b", imageId: "img-B" });

    const { rerender } = render(<PopoverPreview state={{ kind: "item", item: itemA }} />);
    rerender(<PopoverPreview state={{ kind: "item", item: itemB }} />);

    await waitFor(() => {
      expect(screen.getByRole("img", { name: "图片预览" })).toHaveAttribute(
        "src",
        "data:image/webp;base64,PREVIEW_B"
      );
    });

    // A 的预览这时才迟到，必须被丢弃
    await act(async () => {
      previewA.resolve("data:image/webp;base64,PREVIEW_A");
    });

    expect(screen.getByRole("img", { name: "图片预览" })).toHaveAttribute(
      "src",
      "data:image/webp;base64,PREVIEW_B"
    );
  });

  it("卸载后预览才结算，不报错也不输出任何日志", async () => {
    const preview = deferred<string | null>();
    mockGetClipImagePreview.mockReturnValue(preview.promise);
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);

    const { unmount } = render(
      <PopoverPreview state={{ kind: "item", item: makeImageItem() }} />
    );
    unmount();
    await act(async () => {
      preview.resolve(PREVIEW_URL);
    });

    expect(error).not.toHaveBeenCalled();
    expect(warn).not.toHaveBeenCalled();
  });
});
