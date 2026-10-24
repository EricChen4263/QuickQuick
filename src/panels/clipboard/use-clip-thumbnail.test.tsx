/**
 * 缩略图按需拉取 hook 的测试。
 *
 * 这一层是 P4-3 的核心承诺：缩略图不在列表 payload 里，而是渲染时按 imageId 拉取，
 * 且同一 id 在缓存与在途请求之间只发一次 IPC。所以断言的都是「调用了几次」，
 * 而不是「DOM 里有没有图」——后者由 clip-image.test.tsx 覆盖。
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, renderHook, screen, waitFor } from "@testing-library/react";
import type { ReactElement } from "react";
import { readCachedClipThumbnail, resetClipThumbnailCacheForTest } from "./clip-thumbnails";
import { useClipThumbnail } from "./useClipThumbnail";

vi.mock("../../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn(),
}));

import { getClipThumbnail } from "../../ipc/ipc-client";

const mockGetClipThumbnail = vi.mocked(getClipThumbnail);

const THUMB = "data:image/webp;base64,AAA";

beforeEach(() => {
  vi.clearAllMocks();
  resetClipThumbnailCacheForTest();
});

describe("useClipThumbnail", () => {
  it("首次挂载按需拉取一次：未就绪先返回 null，resolve 后返回 data URL", async () => {
    mockGetClipThumbnail.mockResolvedValue(THUMB);

    const { result } = renderHook(() => useClipThumbnail("img-1"));

    expect(result.current).toBeNull();
    await waitFor(() => {
      expect(result.current).toBe(THUMB);
    });
    expect(mockGetClipThumbnail).toHaveBeenCalledTimes(1);
    expect(mockGetClipThumbnail).toHaveBeenCalledWith("img-1");
  });

  it("卸载后重新挂载同一 imageId 命中缓存，不再调 IPC", async () => {
    mockGetClipThumbnail.mockResolvedValue(THUMB);

    const first = renderHook(() => useClipThumbnail("img-1"));
    await waitFor(() => {
      expect(first.result.current).toBe(THUMB);
    });
    first.unmount();

    const second = renderHook(() => useClipThumbnail("img-1"));

    // 缓存命中时首次渲染直接是 data URL，不会先闪一下占位
    expect(second.result.current).toBe(THUMB);
    expect(mockGetClipThumbnail).toHaveBeenCalledTimes(1);
  });

  it("同一 imageId 两个组件同时挂载时只发一次请求（在途去重）", async () => {
    let resolveThumbnail!: (url: string | null) => void;
    mockGetClipThumbnail.mockReturnValue(
      new Promise<string | null>((resolve) => {
        resolveThumbnail = resolve;
      })
    );

    function Pair(): ReactElement {
      const first = useClipThumbnail("img-1");
      const second = useClipThumbnail("img-1");
      return <span data-testid="out">{`${first ?? "-"}|${second ?? "-"}`}</span>;
    }

    render(<Pair />);
    expect(mockGetClipThumbnail).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveThumbnail(THUMB);
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(screen.getByTestId("out").textContent).toBe(`${THUMB}|${THUMB}`);
    });
    expect(mockGetClipThumbnail).toHaveBeenCalledTimes(1);
  });

  it("拉取失败时返回 null 并把失败记入缓存，重新挂载不重试", async () => {
    mockGetClipThumbnail.mockRejectedValue(new Error("数据库忙"));

    const first = renderHook(() => useClipThumbnail("img-1"));
    await waitFor(() => {
      expect(readCachedClipThumbnail("img-1")).toBeNull();
    });
    expect(first.result.current).toBeNull();
    first.unmount();

    const second = renderHook(() => useClipThumbnail("img-1"));

    expect(second.result.current).toBeNull();
    expect(mockGetClipThumbnail).toHaveBeenCalledTimes(1);
  });

  it("imageId 为 null 时不发起任何请求", () => {
    const { result } = renderHook(() => useClipThumbnail(null));

    expect(result.current).toBeNull();
    expect(mockGetClipThumbnail).not.toHaveBeenCalled();
  });

  it("切换 imageId 时按新 id 重新拉取一次", async () => {
    mockGetClipThumbnail.mockImplementation(async (imageId: string) =>
      imageId === "img-1" ? "data:image/webp;base64,ONE" : "data:image/webp;base64,TWO"
    );

    const { result, rerender } = renderHook(
      ({ imageId }: { imageId: string }) => useClipThumbnail(imageId),
      { initialProps: { imageId: "img-1" } }
    );
    await waitFor(() => {
      expect(result.current).toBe("data:image/webp;base64,ONE");
    });

    rerender({ imageId: "img-2" });

    await waitFor(() => {
      expect(result.current).toBe("data:image/webp;base64,TWO");
    });
    expect(mockGetClipThumbnail).toHaveBeenCalledTimes(2);
    expect(mockGetClipThumbnail).toHaveBeenLastCalledWith("img-2");
  });
});
