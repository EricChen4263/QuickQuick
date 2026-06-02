import { describe, it, expect } from "vitest";
import { isToday, filterClipBySearch, groupClipItems } from "./grouping";
import type { ClipItem } from "../ipc/ipc-client";

/** 构建测试用 ClipItem */
function makeItem(
  overrides: Partial<ClipItem> & { id: string }
): ClipItem {
  return {
    content: "test",
    kind: "text",
    isFavorite: false,
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

describe("filterClipBySearch", () => {
  const items: ClipItem[] = [
    makeItem({ id: "a", content: "Hello World" }),
    makeItem({ id: "b", content: "foo bar" }),
    makeItem({ id: "c", content: "HELLO uppercase" }),
  ];

  it("空 query 返回全部副本", () => {
    const result = filterClipBySearch(items, "");
    expect(result).toHaveLength(3);
    expect(result).not.toBe(items);
  });

  it("纯空白 query 返回全部副本", () => {
    const result = filterClipBySearch(items, "   ");
    expect(result).toHaveLength(3);
  });

  it("大小写不敏感命中", () => {
    const result = filterClipBySearch(items, "hello");
    expect(result.map((i) => i.id)).toEqual(["a", "c"]);
  });

  it("无命中返回空数组", () => {
    const result = filterClipBySearch(items, "xyz");
    expect(result).toHaveLength(0);
  });

  it("精确子串命中", () => {
    const result = filterClipBySearch(items, "bar");
    expect(result.map((i) => i.id)).toEqual(["b"]);
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
