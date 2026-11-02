import { useEffect, useState } from "react";
import { getClipImagePreview } from "../ipc/ipc-client";

/**
 * Popover 大图预览的按需取图 hook。
 *
 * 只在预览区真正渲染图片时才请求：预览本身已由分级延迟（1500ms / 200ms）过滤，
 * 快速滚动经过的条目不会触发 IPC。
 *
 * 不做模块级缓存：预览图是按最长边缩放后的 WebP，单张可达数百 KB，
 * 无上限缓存会随历史增长吃内存；重复看同一张时重新生成的成本可以接受。
 *
 * 返回值只对当前 imageId 有效：切换到另一张图时，上一张的结果不会被复用，
 * 避免新图短暂显示成旧图。
 *
 * @param imageId - 图片条目的 clip_images 行 ID；null 表示不需要预览
 * @returns 预览 data URL；未就绪、无原图或生成失败时为 null（调用方回落缩略图）
 */
export function useClipPreviewImage(imageId: string | null): string | null {
  const [loaded, setLoaded] = useState<{ imageId: string; url: string | null } | null>(
    null
  );

  useEffect(() => {
    if (imageId === null) return;

    let cancelled = false;
    getClipImagePreview(imageId)
      .catch((err: unknown) => {
        // 生成失败不阻断渲染：回落缩略图即可，但要留下可诊断的痕迹
        console.warn("[clip-popover] 大图预览生成失败，回落缩略图:", err);
        return null;
      })
      .then((url) => {
        if (!cancelled) setLoaded({ imageId, url });
      });

    return () => {
      cancelled = true;
    };
  }, [imageId]);

  if (imageId === null || loaded === null || loaded.imageId !== imageId) {
    return null;
  }
  return loaded.url;
}
