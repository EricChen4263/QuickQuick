import type { ClipItem } from "../ipc/ipc-client";

/** 分组结果：收藏 / 今天 / 更早三个桶，各保持入参顺序。 */
export interface ClipGroups {
  favorites: ClipItem[];
  today: ClipItem[];
  earlier: ClipItem[];
}

/**
 * 判断 utcMs 与 now 是否属于同一本地日历日（年/月/日相同）。
 * now 由调用方传入，避免在纯函数里访问全局 Date.now()，便于测试。
 */
export function isToday(utcMs: number, now: number): boolean {
  const d = new Date(utcMs);
  const n = new Date(now);
  return (
    d.getFullYear() === n.getFullYear() &&
    d.getMonth() === n.getMonth() &&
    d.getDate() === n.getDate()
  );
}

/**
 * 按 content 字段做大小写不敏感子串过滤。
 * query 为空或纯空白时返回全部条目的浅拷贝（保持调用方不受原数组变更影响）。
 */
export function filterClipBySearch(
  items: ClipItem[],
  query: string
): ClipItem[] {
  const trimmed = query.trim();
  if (trimmed === "") {
    return [...items];
  }
  const lower = trimmed.toLowerCase();
  return items.filter((item) => item.content.toLowerCase().includes(lower));
}

/**
 * 将条目按收藏/今天/更早分组。
 * 收藏项独占 favorites，不再落入其它组；非收藏项按 isToday 分流。
 * 各组内保持与入参相同的顺序。
 */
export function groupClipItems(
  items: ClipItem[],
  now: number
): ClipGroups {
  const favorites: ClipItem[] = [];
  const today: ClipItem[] = [];
  const earlier: ClipItem[] = [];

  for (const item of items) {
    if (item.isFavorite) {
      favorites.push(item);
    } else if (isToday(item.lastModifiedUtc, now)) {
      today.push(item);
    } else {
      earlier.push(item);
    }
  }

  return { favorites, today, earlier };
}
