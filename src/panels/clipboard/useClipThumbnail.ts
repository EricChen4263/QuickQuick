/**
 * 图片行的缩略图 hook：按需拉取 + 命中模块级缓存，未就绪返回 null。
 *
 * 组件只负责渲染，invoke 全部收敛在 clip-thumbnails 层——
 * 这样单测能 mock ipc-client 精确计数「同一 id 只拉一次」，
 * 而不必去数组件渲染次数。
 */

import { useEffect, useState } from "react";
import type { ClipItem } from "../../ipc/ipc-client";
import { loadClipThumbnail, readCachedClipThumbnail } from "./clip-thumbnails";

/**
 * 取图片行的缩略图 data URL。
 *
 * 初始值同步读缓存：缓存命中时首次渲染就能出图，不会先闪一下占位。
 *
 * @param imageId - 要取缩略图的 clip_images 行 ID；传 null 表示这一行不需要缩略图
 * @returns 缩略图 data URL；未加载完成、无图或拉取失败时为 null
 */
export function useClipThumbnail(imageId: string | null): string | null {
  const [dataUrl, setDataUrl] = useState<string | null>(() =>
    imageId === null ? null : readCachedClipThumbnail(imageId) ?? null
  );

  useEffect(() => {
    if (imageId === null) {
      setDataUrl(null);
      return;
    }

    const cached = readCachedClipThumbnail(imageId);
    if (cached !== undefined) {
      setDataUrl(cached);
      return;
    }

    let cancelled = false;
    setDataUrl(null);
    loadClipThumbnail(imageId).then((url) => {
      if (cancelled) return;
      setDataUrl(url);
    });

    return () => {
      cancelled = true;
    };
  }, [imageId]);

  return dataUrl;
}

/**
 * 图片条目的缩略图：由条目自己判断「要不要拉」。
 *
 * 只有 `hasThumbnail` 为真且挂了 `imageId` 的条目才发请求；
 * 文本条目、无缩略图的图片条目一律返回 null 且零请求——
 * 列表里绝大多数行是文本，不能因为共用渲染路径就产生取图流量。
 *
 * @param item - 任意剪贴板条目
 * @returns 缩略图 data URL；未就绪、无图或拉取失败时为 null
 */
export function useClipItemThumbnail(
  item: Pick<ClipItem, "hasThumbnail" | "imageId">
): string | null {
  return useClipThumbnail(item.hasThumbnail ? item.imageId ?? null : null);
}
