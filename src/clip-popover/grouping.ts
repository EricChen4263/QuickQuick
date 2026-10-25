import type { ClipItem } from "../ipc/ipc-client";

/** 分组结果：收藏 / 今天 / 更早三个桶，各保持入参顺序。 */
export interface ClipGroups {
  favorites: ClipItem[];
  today: ClipItem[];
  earlier: ClipItem[];
}

/**
 * 分组的「今天/更早」按哪个时间戳划分。
 *
 * 必须与当前排序口径一致，否则跨组的全局顺序不成立：
 * - `lastCopied`：默认档，按最近一次复制时间（与「最近复制」排序、后端 ORDER BY 同源）；
 * - `firstCopied`：按首次入库时间（与「首次复制」排序同源）。否则一条今天被重新复制的
 *   旧条目会进「今天」组，而它在首次复制排序里其实是老条目，组间顺序就乱了。
 */
export type GroupBasis = "lastCopied" | "firstCopied";

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
 * 取条目在给定分组口径下的时间戳。
 *
 * `createdUtc` 缺省时回退 `lastModifiedUtc`：旧后端 / 测试桩没有这个字段时，
 * 分组退化成本改动前的行为，而不是把条目全塞进「更早」。
 */
export function groupingTimestamp(item: ClipItem, basis: GroupBasis): number {
  if (basis === "firstCopied") {
    return item.createdUtc ?? item.lastModifiedUtc;
  }
  return item.lastModifiedUtc;
}

/**
 * 将条目按收藏/今天/更早分组。
 * 收藏项独占 favorites，不再落入其它组；非收藏项按 `basis` 对应的时间戳是否落在本地今天分流。
 * 各组内保持与入参相同的顺序（入参由后端按当前排序给好）。
 *
 * @param basis - 分组口径，缺省 `lastCopied`（与改动前的默认行为一致）
 */
export function groupClipItems(
  items: ClipItem[],
  now: number,
  basis: GroupBasis = "lastCopied"
): ClipGroups {
  const favorites: ClipItem[] = [];
  const today: ClipItem[] = [];
  const earlier: ClipItem[] = [];

  for (const item of items) {
    if (item.isFavorite) {
      favorites.push(item);
    } else if (isToday(groupingTimestamp(item, basis), now)) {
      today.push(item);
    } else {
      earlier.push(item);
    }
  }

  return { favorites, today, earlier };
}
