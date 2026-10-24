/**
 * 缩略图按需拉取缓存（模块级，跨组件、跨挂载共享）。
 *
 * 为什么需要这一层：列表 payload 不再内联缩略图 base64，改为渲染时按 imageId 拉取。
 * 列表刷新会重建 items 数组、让图片行重新挂载，没有缓存的话一次刷新/滚动就会把
 * 同一批缩略图重新传一遍，等于把要消除的成本挪了个位置。
 *
 * 三条约束：
 * 1. 已确定结果（成功 data URL 或失败 null）缓存在 `settled`，命中直接返回，不再走 IPC；
 * 2. 同一 imageId 的在途请求只发一次（`inflight` 去重），多行同时挂载不会打出一串重复请求；
 * 3. 失败结果同样缓存为 null：取不到缩略图通常是该行已被清理/软删，或 BLOB 为空，
 *    重试只会制造请求风暴。代价是瞬时故障要重开窗口才能自愈——这是本项
 *    「消除重复传输」目标下的刻意取舍，不是遗漏。
 *
 * 无上限淘汰：条目规模由剪贴板历史容量决定（与列表同量级），
 * 按 LRU 淘汰会引入「滚动回来又重传」的抖动，收益不抵复杂度。
 */

import { getClipThumbnail } from "../../ipc/ipc-client";

/** 已确定的拉取结果：成功为 data URL，失败或无图为 null。 */
const settled = new Map<string, string | null>();

/** 在途请求：同一 imageId 并发只发一次。 */
const inflight = new Map<string, Promise<string | null>>();

/**
 * 缓存读取的**唯一入口**：判断「这条 id 有没有已确定的结果」一律走这里，
 * 别处不得直接摸 `settled`。
 *
 * 只有两个合法调用点，各有独立断言兜住：
 * - `useClipThumbnail` 在渲染期读一次，决定这一帧直接显示什么（对应「缓存命中时
 *   重挂载首次渲染就是 data URL」的同步断言）；
 * - `loadClipThumbnail` 读一次，决定要不要真的发 IPC（对应「缓存命中不再调 IPC」的计数断言）。
 * 任一处失效都会让对应断言变红，不会再出现「一处被删、另一处悄悄兜住」。
 *
 * @param imageId - clip_images 行 ID
 * @returns 已确定的 data URL / null；从未拉取过时返回 undefined（与「确定无图」区分）
 */
export function trySettledClipThumbnail(imageId: string): string | null | undefined {
  return settled.get(imageId);
}

/**
 * 拉取缩略图，优先命中缓存与在途请求，不重复发 IPC。
 *
 * 永不 reject：失败一律折叠为 null，调用方按「没有缩略图」渲染占位。
 *
 * @param imageId - clip_images 行 ID
 * @returns 缩略图 data URL 或 null
 */
export async function loadClipThumbnail(imageId: string): Promise<string | null> {
  const cached = trySettledClipThumbnail(imageId);
  if (cached !== undefined) return cached;

  const pending = inflight.get(imageId);
  if (pending !== undefined) return pending;

  const request: Promise<string | null> = getClipThumbnail(imageId)
    .catch(() => null)
    .then((dataUrl) => {
      settled.set(imageId, dataUrl);
      return dataUrl;
    })
    .finally(() => {
      inflight.delete(imageId);
    });
  inflight.set(imageId, request);
  return request;
}

/**
 * 清空缓存。
 *
 * 仅供测试使用：模块级状态会在同一测试文件的用例之间残留，
 * 生产代码没有清缓存的场景（缓存的正确性不依赖失效）。
 */
export function resetClipThumbnailCacheForTest(): void {
  settled.clear();
  inflight.clear();
}
