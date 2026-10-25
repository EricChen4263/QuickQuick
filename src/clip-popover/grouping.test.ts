import { describe, it, expect } from "vitest";
import { isToday, groupClipItems, groupingTimestamp } from "./grouping";
import type { ClipItem } from "../ipc/ipc-client";

/** 构建测试用 ClipItem */
function makeItem(
  overrides: Partial<ClipItem> & { id: string }
): ClipItem {
  return {
    content: "test",
    kind: "text",
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: 1000,
    ...overrides,
  };
}

describe("isToday", () => {
  it("同年月日返回 true", () => {
    const now = new Date("2026-06-02T15:30:00").getTime();
    const ts = new Date("2026-06-02T08:00:00").getTime();
    expect(isToday(ts, now)).toBe(true);
  });

  it("昨天返回 false（用 now=noon 避免跨时区误判）", () => {
    // now 设为正午，ts 设为前一天正午，任何时区下均跨日
    const now = new Date("2026-06-02T12:00:00").getTime();
    const ts = new Date("2026-06-01T12:00:00").getTime();
    expect(isToday(ts, now)).toBe(false);
  });

  it("明天返回 false（用本地正午避免跨时区误判）", () => {
    const now = new Date("2026-06-02T12:00:00").getTime();
    const ts = new Date("2026-06-03T12:00:00").getTime();
    expect(isToday(ts, now)).toBe(false);
  });
});

describe("groupClipItems", () => {
  // 用本地时间（无 Z 后缀）避免跨时区误判：正午时刻确保同日/跨日判断与时区无关
  const NOW = new Date("2026-06-02T12:00:00").getTime();
  const TODAY_TS = new Date("2026-06-02T09:00:00").getTime();
  const EARLIER_TS = new Date("2026-06-01T10:00:00").getTime();

  it("收藏项进 favorites，不落入 today/earlier", () => {
    const fav = makeItem({ id: "fav", isFavorite: true, lastModifiedUtc: TODAY_TS });
    const result = groupClipItems([fav], NOW);
    expect(result.favorites.map((i) => i.id)).toContain("fav");
    expect(result.today.map((i) => i.id)).not.toContain("fav");
    expect(result.earlier.map((i) => i.id)).not.toContain("fav");
  });

  it("今天的非收藏项进 today", () => {
    const todayItem = makeItem({ id: "t1", isFavorite: false, lastModifiedUtc: TODAY_TS });
    const result = groupClipItems([todayItem], NOW);
    expect(result.today.map((i) => i.id)).toContain("t1");
    expect(result.earlier).toHaveLength(0);
  });

  it("更早的非收藏项进 earlier", () => {
    const old = makeItem({ id: "old1", isFavorite: false, lastModifiedUtc: EARLIER_TS });
    const result = groupClipItems([old], NOW);
    expect(result.earlier.map((i) => i.id)).toContain("old1");
    expect(result.today).toHaveLength(0);
  });

  it("混合场景：收藏/今天/更早各归其组，保持输入顺序", () => {
    const items: ClipItem[] = [
      makeItem({ id: "fav1", isFavorite: true, lastModifiedUtc: TODAY_TS }),
      makeItem({ id: "t1", isFavorite: false, lastModifiedUtc: TODAY_TS }),
      makeItem({ id: "old1", isFavorite: false, lastModifiedUtc: EARLIER_TS }),
      makeItem({ id: "fav2", isFavorite: true, lastModifiedUtc: EARLIER_TS }),
      makeItem({ id: "t2", isFavorite: false, lastModifiedUtc: TODAY_TS }),
    ];
    const result = groupClipItems(items, NOW);

    expect(result.favorites.map((i) => i.id)).toEqual(["fav1", "fav2"]);
    expect(result.today.map((i) => i.id)).toEqual(["t1", "t2"]);
    expect(result.earlier.map((i) => i.id)).toEqual(["old1"]);
  });

  it("空数组返回三个空组", () => {
    const result = groupClipItems([], NOW);
    expect(result.favorites).toHaveLength(0);
    expect(result.today).toHaveLength(0);
    expect(result.earlier).toHaveLength(0);
  });
});

describe("groupClipItems — 分组口径跟随排序（P5-5 收尾）", () => {
  const NOW = new Date("2026-06-02T12:00:00").getTime();
  const TODAY_TS = new Date("2026-06-02T09:00:00").getTime();
  const EARLIER_TS = new Date("2026-06-01T10:00:00").getTime();

  it("默认口径仍是最近复制：createdUtc 不影响归档", () => {
    const item = makeItem({
      id: "recopied",
      lastModifiedUtc: TODAY_TS,
      createdUtc: EARLIER_TS,
    });

    const groups = groupClipItems([item], NOW);

    expect(groups.today.map((i) => i.id)).toEqual(["recopied"]);
    expect(groups.earlier).toHaveLength(0);
  });

  it("首次复制口径按 createdUtc 归档：今天被重新复制的旧条目落「更早」", () => {
    const item = makeItem({
      id: "recopied",
      lastModifiedUtc: TODAY_TS,
      createdUtc: EARLIER_TS,
    });

    const groups = groupClipItems([item], NOW, "firstCopied");

    expect(groups.earlier.map((i) => i.id)).toEqual(["recopied"]);
    expect(groups.today).toHaveLength(0);
  });

  it("首次复制口径按 createdUtc 归档：今天首次复制的条目落「今天」", () => {
    const item = makeItem({
      id: "created-today",
      lastModifiedUtc: EARLIER_TS,
      createdUtc: TODAY_TS,
    });

    const groups = groupClipItems([item], NOW, "firstCopied");

    expect(groups.today.map((i) => i.id)).toEqual(["created-today"]);
    expect(groups.earlier).toHaveLength(0);
  });

  it("createdUtc 缺省时回退 lastModifiedUtc（旧后端/测试桩不会掉进「更早」）", () => {
    const item = makeItem({ id: "no-created", lastModifiedUtc: TODAY_TS });
    expect(groupingTimestamp(item, "firstCopied")).toBe(TODAY_TS);

    const groups = groupClipItems([item], NOW, "firstCopied");
    expect(groups.today.map((i) => i.id)).toEqual(["no-created"]);
  });

  it("首次复制口径下收藏仍独占最前组", () => {
    const fav = makeItem({
      id: "fav",
      isFavorite: true,
      lastModifiedUtc: EARLIER_TS,
      createdUtc: EARLIER_TS,
    });
    const normal = makeItem({
      id: "normal",
      lastModifiedUtc: TODAY_TS,
      createdUtc: TODAY_TS,
    });

    const groups = groupClipItems([fav, normal], NOW, "firstCopied");

    expect(groups.favorites.map((i) => i.id)).toEqual(["fav"]);
    expect(groups.today.map((i) => i.id)).toEqual(["normal"]);
  });

  it("后端按 created 降序给数时，首次复制口径下「组间 + 组内」都保持 created 降序", () => {
    // 后端「首次复制」排序的输出顺序：created 降序，收藏整体在前
    const items: ClipItem[] = [
      makeItem({ id: "created-today-newest", lastModifiedUtc: EARLIER_TS, createdUtc: TODAY_TS }),
      makeItem({
        id: "created-today-older",
        lastModifiedUtc: TODAY_TS,
        createdUtc: TODAY_TS - 60_000,
      }),
      makeItem({
        id: "created-yesterday",
        lastModifiedUtc: TODAY_TS,
        createdUtc: EARLIER_TS,
      }),
    ];

    const groups = groupClipItems(items, NOW, "firstCopied");
    const concatenated = [...groups.favorites, ...groups.today, ...groups.earlier];

    expect(concatenated.map((i) => i.id)).toEqual([
      "created-today-newest",
      "created-today-older",
      "created-yesterday",
    ]);
    const createdSequence = concatenated.map((i) => i.createdUtc ?? 0);
    expect(createdSequence).toEqual([...createdSequence].sort((a, b) => b - a));
  });

  it("最近复制口径下同样的数据按最近复制时间归档（两档确实不同）", () => {
    const items: ClipItem[] = [
      makeItem({ id: "created-today-newest", lastModifiedUtc: EARLIER_TS, createdUtc: TODAY_TS }),
      makeItem({
        id: "created-today-older",
        lastModifiedUtc: TODAY_TS,
        createdUtc: TODAY_TS - 60_000,
      }),
    ];

    const recent = groupClipItems(items, NOW, "lastCopied");
    const first = groupClipItems(items, NOW, "firstCopied");

    expect(recent.today.map((i) => i.id)).toEqual(["created-today-older"]);
    expect(recent.earlier.map((i) => i.id)).toEqual(["created-today-newest"]);
    expect(first.today.map((i) => i.id)).toEqual([
      "created-today-newest",
      "created-today-older",
    ]);
    expect(first.earlier).toHaveLength(0);
  });
});
