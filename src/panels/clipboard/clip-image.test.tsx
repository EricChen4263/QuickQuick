/**
 * 图片剪贴板前端渲染层测试（V5-F1-S04）。
 * 覆盖：filter image、toHistoryItem image 分支、ClipItemRow 图片渲染、ClipPreview/ImagePreview 原图加载。
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import { filterByType } from "../history/filter";
import type { HistoryFilter } from "../history/filter";
import type { HistoryItem } from "../history/search";
import { ClipItemRow } from "./ClipItemRow";
import { ClipPreview } from "./ClipPreview";

// Mock ipc-client，需要 getClipImageOriginal
vi.mock("../../ipc/ipc-client", () => ({
  listClipItems: vi.fn(),
  deleteClipItem: vi.fn(),
  toggleFavoriteClip: vi.fn(),
  getClipImageOriginal: vi.fn(),
}));

import { getClipImageOriginal } from "../../ipc/ipc-client";

const mockGetClipImageOriginal = vi.mocked(getClipImageOriginal);

beforeEach(() => {
  vi.clearAllMocks();
});

describe("filterByType image", () => {
  const items: HistoryItem[] = [
    { id: "1", text: "纯文本", kind: "text" },
    { id: "2", text: "富文本", kind: "richtext" },
    { id: "3", text: "[图片] 200x300", kind: "image" },
    { id: "4", text: "[图片] 400x500", kind: "image" },
  ];

  it("filter=image 只返回 kind=image 的条目", () => {
    const filter: HistoryFilter = "image";
    const result = filterByType(items, filter);
    expect(result).toHaveLength(2);
    expect(result.every((i) => i.kind === "image")).toBe(true);
    expect(result.map((i) => i.id)).toEqual(["3", "4"]);
  });

  it("filter=text 不含 image 条目", () => {
    const filter: HistoryFilter = "text";
    const result = filterByType(items, filter);
    expect(result).toHaveLength(1);
    expect(result[0].kind).toBe("text");
    expect(result.every((i) => i.kind !== "image")).toBe(true);
  });

  it("filter=all 包含 image 条目", () => {
    const result = filterByType(items, "all");
    expect(result).toHaveLength(4);
    expect(result.some((i) => i.kind === "image")).toBe(true);
  });
});

describe("ClipItemRow image rendering", () => {
  const baseItem = {
    id: "img-1",
    content: "[图片] 200x300",
    kind: "image" as const,
    isFavorite: false,
    lastModifiedUtc: 1000,
  };

  it("图片项有 thumbnailDataUrl 时渲染 <img> 且 src 等于 thumbnailDataUrl", () => {
    const thumbnailDataUrl = "data:image/webp;base64,ABC123";
    const item = { ...baseItem, thumbnailDataUrl };

    render(
      <ClipItemRow
        item={item}
        isHighlighted={false}
        onToggleFavorite={vi.fn()}
        onDelete={vi.fn()}
      />
    );

    const img = screen.getByRole("img", { name: "图片缩略图" });
    expect(img).toBeInTheDocument();
    expect(img).toHaveAttribute("src", thumbnailDataUrl);
  });

  it("图片项无 thumbnailDataUrl 时渲染 '[图片]' 占位文字", () => {
    render(
      <ClipItemRow
        item={baseItem}
        isHighlighted={false}
        onToggleFavorite={vi.fn()}
        onDelete={vi.fn()}
      />
    );

    expect(screen.getByText("[图片]")).toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("文本项不渲染 img 标签", () => {
    const textItem = {
      id: "text-1",
      content: "Hello World",
      kind: "text" as const,
      isFavorite: false,
      lastModifiedUtc: 1000,
    };

    render(
      <ClipItemRow
        item={textItem}
        isHighlighted={false}
        onToggleFavorite={vi.fn()}
        onDelete={vi.fn()}
      />
    );

    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByText("Hello World")).toBeInTheDocument();
  });
});

describe("ClipPreview image item (ImagePreview)", () => {
  const imageItem = {
    id: "img-item-1",
    content: "[图片] 200x300",
    kind: "image" as const,
    isFavorite: false,
    lastModifiedUtc: 1000,
    imageId: "uuid-image-1",
    thumbnailDataUrl: "data:image/webp;base64,THUMB123",
  };

  it("加载中先显示缩略图，成功后显示原图 img", async () => {
    const originalDataUrl = "data:image/png;base64,ORIGINAL456";
    mockGetClipImageOriginal.mockResolvedValue(originalDataUrl);

    render(<ClipPreview item={imageItem} />);

    // 原图尚未加载完成时，应先以缩略图作为回退显示
    expect(screen.getByRole("img")).toHaveAttribute("src", imageItem.thumbnailDataUrl);

    // 原图加载完成后应显示原图 img
    await waitFor(() => {
      const images = screen.getAllByRole("img");
      const originalImg = images.find((img) =>
        img.getAttribute("src") === originalDataUrl
      );
      expect(originalImg).toBeDefined();
    });
  });

  it("getClipImageOriginal 返回 null 时显示缩略图回退", async () => {
    mockGetClipImageOriginal.mockResolvedValue(null);

    render(<ClipPreview item={imageItem} />);

    await waitFor(() => {
      const img = screen.getByRole("img");
      expect(img).toHaveAttribute("src", imageItem.thumbnailDataUrl);
    });
  });

  it("文本项显示 <p> 文本内容不渲染 img", () => {
    const textItem = {
      id: "text-1",
      content: "Hello text content",
      kind: "text" as const,
      isFavorite: false,
      lastModifiedUtc: 1000,
    };

    render(<ClipPreview item={textItem} />);

    expect(screen.getByText("Hello text content")).toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("快速切换 imageId 时旧请求迟到 resolve 不覆盖新结果（stale 防护）", async () => {
    const imageItem1 = {
      id: "img-item-1",
      content: "[图片] 200x300",
      kind: "image" as const,
      isFavorite: false,
      lastModifiedUtc: 1000,
      imageId: "uuid-image-1",
      thumbnailDataUrl: "data:image/webp;base64,THUMB1",
    };
    const imageItem2 = {
      id: "img-item-2",
      content: "[图片] 400x500",
      kind: "image" as const,
      isFavorite: false,
      lastModifiedUtc: 2000,
      imageId: "uuid-image-2",
      thumbnailDataUrl: "data:image/webp;base64,THUMB2",
    };

    const originalUrl1 = "data:image/png;base64,ORIGINAL_1";
    const originalUrl2 = "data:image/png;base64,ORIGINAL_2";

    // 为每次调用创建可手动控制的 deferred Promise
    let resolveItem1!: (url: string) => void;
    let resolveItem2!: (url: string) => void;
    const deferredItem1 = new Promise<string>((res) => { resolveItem1 = res; });
    const deferredItem2 = new Promise<string>((res) => { resolveItem2 = res; });

    // 第一次调用返回 item1 的 deferred，第二次返回 item2 的 deferred
    mockGetClipImageOriginal
      .mockReturnValueOnce(deferredItem1)
      .mockReturnValueOnce(deferredItem2);

    const { rerender } = render(<ClipPreview item={imageItem1} />);

    // 切换到 item2，触发第二次 effect（第一次 effect cleanup 将 cancelled=true）
    rerender(<ClipPreview item={imageItem2} />);

    // 先 resolve item2（新请求），再 resolve item1（旧请求，应被忽略）
    await act(async () => {
      resolveItem2(originalUrl2);
      await deferredItem2;
    });

    await act(async () => {
      resolveItem1(originalUrl1);
      await deferredItem1;
    });

    // 最终应显示 item2 的原图，不被 item1 的迟到 resolve 覆盖
    await waitFor(() => {
      const img = screen.getByRole("img");
      expect(img).toHaveAttribute("src", originalUrl2);
      expect(img).not.toHaveAttribute("src", originalUrl1);
    });
  });
});
