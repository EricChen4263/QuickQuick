import { describe, it, expect } from "vitest";
import {
  DEFAULT_SEARCH_MODE,
  INVALID_REGEX_NOTICE,
  clipSearchText,
  matchTextWithPlan,
  prepareQuery,
  searchClips,
  splitHighlight,
  type SearchMode,
  type TextMatch,
} from "./clip-search";
import type { ClipItem } from "../ipc/ipc-client";

/** 构建测试用 ClipItem */
function makeItem(overrides: Partial<ClipItem> & { id: string }): ClipItem {
  return {
    content: "test",
    kind: "text",
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: 1000,
    ...overrides,
  };
}

/** 单文本匹配：查询计划只准备一次（与生产调用路径一致） */
function match(text: string, query: string, mode: SearchMode = DEFAULT_SEARCH_MODE): TextMatch {
  return matchTextWithPlan(text, prepareQuery(query, mode));
}

/** 取出命中条目的 id，便于断言过滤结果 */
function hitIds(items: ClipItem[], query: string, mode?: SearchMode): string[] {
  return searchClips(items, query, mode).map((hit) => hit.item.id);
}

const CLIP_ITEMS: ClipItem[] = [
  makeItem({ id: "a", content: "Hello World" }),
  makeItem({ id: "b", content: "foo bar" }),
  makeItem({ id: "c", content: "HELLO uppercase" }),
  makeItem({ id: "d", content: "世界你好，剪贴板历史" }),
];

describe("clipSearchText：搜索/高亮共用的文本来源", () => {
  it("有 title 时用 title", () => {
    expect(clipSearchText(makeItem({ id: "x", content: "正文", title: "标题" }))).toBe("标题");
  });

  it("没有 title 时回退 content", () => {
    expect(clipSearchText(makeItem({ id: "x", content: "正文" }))).toBe("正文");
  });
});

describe("空查询", () => {
  it("空串返回全部条目且区间为空", () => {
    const hits = searchClips(CLIP_ITEMS, "");
    expect(hits.map((hit) => hit.item.id)).toEqual(["a", "b", "c", "d"]);
    expect(hits.every((hit) => hit.ranges.length === 0)).toBe(true);
  });

  it("纯空白等同于空查询", () => {
    const hits = searchClips(CLIP_ITEMS, "   \t ");
    expect(hits).toHaveLength(CLIP_ITEMS.length);
    expect(hits.every((hit) => hit.ranges.length === 0)).toBe(true);
  });

  it("默认模式是 mixed", () => {
    expect(DEFAULT_SEARCH_MODE).toBe("mixed");
  });
});

describe("exact：整串子串 + 多词 AND", () => {
  it("命中含该子串的条目，并按原顺序返回", () => {
    expect(hitIds(CLIP_ITEMS, "hello", "exact")).toEqual(["a", "c"]);
  });

  it("大小写不敏感（查询大写、正文小写同样命中）", () => {
    expect(hitIds(CLIP_ITEMS, "HELLO", "exact")).toEqual(["a", "c"]);
  });

  it("不命中时返回空数组", () => {
    expect(hitIds(CLIP_ITEMS, "xyz", "exact")).toEqual([]);
  });

  it("中文子串命中", () => {
    expect(hitIds(CLIP_ITEMS, "剪贴板", "exact")).toEqual(["d"]);
  });

  it("两个词都命中才算命中（AND 而非 OR）", () => {
    expect(hitIds(CLIP_ITEMS, "hello world", "exact")).toEqual(["a"]);
    expect(hitIds(CLIP_ITEMS, "hello bar", "exact")).toEqual([]);
  });

  it("只命中一个词时不算命中", () => {
    const items = [
      makeItem({ id: "only-foo", content: "foo only" }),
      makeItem({ id: "both", content: "foo bar" }),
    ];
    expect(hitIds(items, "foo bar", "exact")).toEqual(["both"]);
  });

  it("命中区间覆盖全部命中位置（同一词多次出现）", () => {
    const result = match("ab ab", "ab", "exact");
    expect(result.matched).toBe(true);
    expect(result.ranges).toEqual([
      { start: 0, end: 2 },
      { start: 3, end: 5 },
    ]);
  });
});

describe("fuzzy：子序列匹配", () => {
  it("abc 命中 aXbXc（字符按序出现即可）", () => {
    const result = match("aXbXc", "abc", "fuzzy");
    expect(result.matched).toBe(true);
    expect(result.ranges).toEqual([
      { start: 0, end: 1 },
      { start: 2, end: 3 },
      { start: 4, end: 5 },
    ]);
  });

  it("acb 不命中（顺序错）", () => {
    expect(match("acb", "abc", "fuzzy").matched).toBe(false);
  });

  it("相邻命中的字符会合并成一个区间", () => {
    expect(match("abc", "abc", "fuzzy").ranges).toEqual([{ start: 0, end: 3 }]);
  });

  it("大小写不敏感且中文可跨字匹配", () => {
    expect(match("HeLLo", "hlo", "fuzzy").matched).toBe(true);
    expect(match("剪贴板历史", "剪史", "fuzzy").matched).toBe(true);
  });

  it("多词 AND：两词都要是子序列", () => {
    const items = [
      makeItem({ id: "both", content: "a1b2c3 z9y8x7" }),
      makeItem({ id: "one", content: "a1b2c3" }),
    ];
    expect(hitIds(items, "abc zyx", "fuzzy")).toEqual(["both"]);
    expect(hitIds(items, "abc zyx", "exact")).toEqual([]);
  });
});

describe("regex：用户给正则", () => {
  it("正则命中并给出完整区间", () => {
    const result = match("订单 A12 与 B34", "\\d+", "regex");
    expect(result.matched).toBe(true);
    expect(result.ranges).toEqual([
      { start: 4, end: 6 },
      { start: 10, end: 12 },
    ]);
    expect(result.notice).toBeNull();
  });

  it("不命中时 matched 为 false 且区间为空", () => {
    expect(match("hello", "^h.*z$", "regex")).toEqual({
      matched: false,
      ranges: [],
      notice: null,
    });
  });

  it("多词时每个词各自作为正则并 AND", () => {
    const text = "foo 123 bar 456";
    expect(match(text, "fo+ \\d{3}", "regex").matched).toBe(true);
    expect(match(text, "fo+ \\d{4}", "regex").matched).toBe(false);
  });

  it("同一条正则被多个条目复用时游标必须复位（第二条不会漏匹配）", () => {
    const items = [makeItem({ id: "one", content: "id=7" }), makeItem({ id: "two", content: "id=8" })];
    const hits = searchClips(items, "id=\\d", "regex");
    expect(hits).toHaveLength(2);
    expect(hits[1].ranges).toEqual([{ start: 0, end: 4 }]);
  });

  it("非法正则降级为文本搜索，给出可读提示且不抛异常", () => {
    const plan = prepareQuery("foo[", "regex");
    expect(plan.matcher).toBe("exact");
    expect(plan.notice).toBe(INVALID_REGEX_NOTICE);
    // 降级后按「逐词子串」搜索：字面量 `foo[` 才能命中
    expect(hitIds([makeItem({ id: "lit", content: "foo[1]" })], "foo[", "regex")).toEqual([
      "lit",
    ]);
    expect(plan.notice).toContain("正则");
  });

  it("零宽正则（a* / ^）不会死循环，且只产生非零宽区间", () => {
    const startedAt = performance.now();
    // 真正的兜底是测试超时；这里再加一条时间上界，防止「能返回但已经跑了很久」
    const starOnText = match("a", "a*", "regex");
    const starOnNonMatch = match("bbb", "a*", "regex");
    const anchor = match("hello", "^", "regex");
    const elapsedMs = performance.now() - startedAt;

    expect(elapsedMs).toBeLessThan(500);
    expect(starOnText.matched).toBe(true);
    expect(starOnText.ranges).toEqual([{ start: 0, end: 1 }]);
    // 零宽命中算「匹配成功」，但高亮区间被剔除，不会退化成空区间死循环
    expect(starOnNonMatch.matched).toBe(true);
    expect(starOnNonMatch.ranges).toEqual([]);
    expect(anchor.matched).toBe(true);
    expect(anchor.ranges).toEqual([]);
  });

  it("零宽正则作用于长文本仍极快返回", () => {
    const longText = "b".repeat(2000);
    const startedAt = performance.now();
    const result = match(longText, "a*", "regex");
    expect(performance.now() - startedAt).toBeLessThan(500);
    expect(result.matched).toBe(true);
    expect(result.ranges).toEqual([]);
  });
});

describe("mixed：默认模式的判据", () => {
  it("不含正则元字符时走逐词子串（多词 AND）", () => {
    const plan = prepareQuery("hello world", "mixed");
    expect(plan.matcher).toBe("exact");
    expect(plan.tokens).toEqual(["hello", "world"]);
    expect(hitIds(CLIP_ITEMS, "hello world")).toEqual(["a"]);
  });

  it("含正则元字符时整串按正则处理", () => {
    expect(prepareQuery("^hel", "mixed").matcher).toBe("regex");
    expect(hitIds(CLIP_ITEMS, "^hel")).toEqual(["a", "c"]);
  });

  it("句点不算元字符（URL / 小数不会被误判成正则）", () => {
    const plan = prepareQuery("example.com", "mixed");
    expect(plan.matcher).toBe("exact");
    expect(hitIds([makeItem({ id: "url", content: "https://example.com/a" })], "example.com")).toEqual([
      "url",
    ]);
    // 若被当成正则，`.` 会匹配任意字符，从而命中 exampleXcom
    expect(hitIds([makeItem({ id: "dot", content: "exampleXcom" })], "example.com")).toEqual([]);
  });

  it("非法正则（mixed 自动识别出的）同样降级并给出提示", () => {
    const plan = prepareQuery("foo[", "mixed");
    expect(plan.matcher).toBe("exact");
    expect(plan.notice).toBe(INVALID_REGEX_NOTICE);
  });
});

describe("title 优先作为搜索对象", () => {
  it("命中与高亮都基于 title 而不是 content", () => {
    const items = [
      makeItem({ id: "titled", content: "无关正文", title: "会议纪要" }),
      makeItem({ id: "plain", content: "会议纪要附件" }),
    ];
    const hits = searchClips(items, "纪要");
    expect(hits.map((hit) => hit.item.id)).toEqual(["titled", "plain"]);
    expect(hits[0].ranges).toEqual([{ start: 2, end: 4 }]);
  });
});

describe("splitHighlight：文本切分", () => {
  it("空 ranges 返回单段普通文本", () => {
    expect(splitHighlight("hello", [])).toEqual([{ text: "hello", hit: false }]);
  });

  it("单段区间覆盖整串时返回单段命中", () => {
    expect(splitHighlight("hello", [{ start: 0, end: 5 }])).toEqual([
      { text: "hello", hit: true },
    ]);
  });

  it("多个不相邻区间切成命中/普通交替", () => {
    expect(
      splitHighlight("abcdef", [
        { start: 1, end: 2 },
        { start: 4, end: 5 },
      ]),
    ).toEqual([
      { text: "a", hit: false },
      { text: "b", hit: true },
      { text: "cd", hit: false },
      { text: "e", hit: true },
      { text: "f", hit: false },
    ]);
  });

  it("相邻区间合并成一段命中", () => {
    expect(
      splitHighlight("abcd", [
        { start: 0, end: 2 },
        { start: 2, end: 3 },
      ]),
    ).toEqual([
      { text: "abc", hit: true },
      { text: "d", hit: false },
    ]);
  });

  it("乱序、重叠、越界的区间都会被归一化且不丢字符", () => {
    const segments = splitHighlight("abcdef", [
      { start: 4, end: 99 },
      { start: 2, end: 3 },
      { start: 1, end: 2 },
      { start: 3, end: 3 },
      { start: -5, end: 1 },
    ]);
    expect(segments.map((segment) => segment.text).join("")).toBe("abcdef");
    expect(segments).toEqual([
      { text: "abc", hit: true },
      { text: "d", hit: false },
      { text: "ef", hit: true },
    ]);
  });

  it("文本含 HTML 特殊字符时只产出纯文本片段（不生成任何标记）", () => {
    const text = `<script>alert("x")</script> & 'q'`;
    const segments = splitHighlight(text, [{ start: 0, end: 8 }]);
    expect(segments.map((segment) => segment.text).join("")).toBe(text);
    expect(segments[0]).toEqual({ text: "<script>", hit: true });
    expect(segments[1]).toEqual({ text: `alert("x")</script> & 'q'`, hit: false });
    // 片段只是字符串，不含任何 HTML 语法糖（渲染层负责按文本渲染）
    expect(segments.every((segment) => typeof segment.text === "string")).toBe(true);
  });
});
