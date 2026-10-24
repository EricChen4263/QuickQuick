/**
 * 剪贴板搜索：4 种模式 + 多词 AND + 命中区间计算。
 *
 * 主窗（panels/clipboard）与 popover 共用这一份实现，两边入口不再各写一份匹配逻辑。
 * 全部是纯函数：不读全局状态、不修改入参，同一输入必然得到同一输出。
 */

import type { ClipItem } from "../ipc/ipc-client";

/** 搜索模式：整串子串 / 子序列 / 正则 / 自动混合 */
export type SearchMode = "exact" | "fuzzy" | "regex" | "mixed";

/** 默认模式：多词 AND + 逐词子串，输入看起来像正则时自动按正则处理 */
export const DEFAULT_SEARCH_MODE: SearchMode = "mixed";

/** 非法正则降级为文本搜索时的可读提示；UI 直接展示，测试据此断言 */
export const INVALID_REGEX_NOTICE = "正则表达式无效，已按普通文本搜索";

/** 命中区间（UTF-16 码元、左闭右开），与 String#slice 的口径一致 */
export interface SearchRange {
  start: number;
  end: number;
}

/** 单条命中：条目 + 该条目搜索文本上的命中区间（已排序、已合并、无零宽区间） */
export interface SearchHit {
  item: ClipItem;
  ranges: SearchRange[];
}

/** 实际生效的匹配算法 */
export type Matcher = "exact" | "fuzzy" | "regex";

/**
 * 查询计划：分词与正则编译的结果。
 * 一次查询只准备一份，逐条目复用以避免在循环里重复分词、重复编译正则。
 */
export interface QueryPlan {
  /** 空查询（含纯空白）：放行全部条目，既不匹配也不产生高亮 */
  readonly isEmpty: boolean;
  readonly matcher: Matcher;
  /** 原始词项（按空白分词）；正则模式用它编译正则 */
  readonly tokens: string[];
  /** 小写化词项；exact / fuzzy 用它比较，正则模式下为空 */
  readonly foldedTokens: string[];
  /** 已编译的全局正则，与 tokens 一一对应；仅正则模式非空 */
  readonly patterns: RegExp[];
  /** 用户可读的模式说明或降级提示；null 表示按所选模式正常执行 */
  readonly notice: string | null;
}

/** 单条文本的匹配结果 */
export interface TextMatch {
  /** 是否满足全部词项（多词 AND） */
  matched: boolean;
  /** 命中区间，已排序合并、已剔除零宽区间 */
  ranges: SearchRange[];
  /** 模式降级提示；同一查询下所有条目相同 */
  notice: string | null;
}

/** 高亮切分段：hit 为 true 的片段交给渲染层包 <mark> */
export interface HighlightSegment {
  text: string;
  hit: boolean;
}

/**
 * 视为「用户想写正则」的元字符集合。
 * 刻意不含 `.`：URL、小数、句末点号太常见，含进来会把普通搜索误判成正则。
 */
const REGEX_METACHARACTERS = /[\\^$[\](){}|+*?]/;

/** 分词分隔符：半角与全角空白都算 */
const WORD_SEPARATOR = /\s+/;

/** 空查询的计划常量：所有条目原样放行，ranges 为空 */
const EMPTY_PLAN: QueryPlan = {
  isEmpty: true,
  matcher: "exact",
  tokens: [],
  foldedTokens: [],
  patterns: [],
  notice: null,
};

/**
 * 搜索与高亮共用的文本来源。
 * 优先后端预计算的 title（P4-5a 落库），缺省回退正文，保证后端还没给 title 时行为不变。
 *
 * @param item - 剪贴板条目
 * @returns 用于匹配与高亮的文本
 */
export function clipSearchText(item: ClipItem): string {
  return item.title ?? item.content;
}

/**
 * 判断整串输入是否应该按正则处理。
 *
 * @param query - 已 trim 的搜索串
 * @returns 含任一正则元字符则返回 true
 */
function looksLikeRegex(query: string): boolean {
  return REGEX_METACHARACTERS.test(query);
}

/**
 * 把用户选择的模式解析为实际生效的匹配算法。
 * mixed 的判据：整串含正则元字符就走正则，否则走逐词子串。
 */
function resolveMatcher(trimmed: string, mode: SearchMode): Matcher {
  switch (mode) {
    case "fuzzy":
      return "fuzzy";
    case "regex":
      return "regex";
    case "mixed":
      return looksLikeRegex(trimmed) ? "regex" : "exact";
    case "exact":
    default:
      return "exact";
  }
}

/** 词项小写化：中文不受影响，只折叠西文大小写 */
function foldTokens(tokens: string[]): string[] {
  return tokens.map((token) => token.toLowerCase());
}

/**
 * 生成查询计划：分词 + 解析模式 + 编译正则。
 *
 * 正则非法时整串降级为逐词子串搜索，并在 notice 里给出可读原因——既不抛异常，也不静默吞掉。
 *
 * @param query - 用户原始输入
 * @param mode - 搜索模式，缺省 mixed
 * @returns 查询计划（纯数据，可安全跨条目复用）
 */
export function prepareQuery(
  query: string,
  mode: SearchMode = DEFAULT_SEARCH_MODE,
): QueryPlan {
  const trimmed = query.trim();
  if (trimmed === "") return EMPTY_PLAN;

  const tokens = trimmed.split(WORD_SEPARATOR).filter((token) => token !== "");
  const matcher = resolveMatcher(trimmed, mode);
  if (matcher !== "regex") {
    return {
      isEmpty: false,
      matcher,
      tokens,
      foldedTokens: foldTokens(tokens),
      patterns: [],
      notice: null,
    };
  }

  const patterns: RegExp[] = [];
  for (const token of tokens) {
    try {
      patterns.push(new RegExp(token, "gi"));
    } catch {
      return {
        isEmpty: false,
        matcher: "exact",
        tokens,
        foldedTokens: foldTokens(tokens),
        patterns: [],
        notice: INVALID_REGEX_NOTICE,
      };
    }
  }
  return {
    isEmpty: false,
    matcher: "regex",
    tokens,
    foldedTokens: [],
    patterns,
    notice: null,
  };
}

/**
 * 收集 needle 在 haystack 里的全部不重叠出现位置。
 * 逐次从命中末尾继续查找，needle 非空时 indexOf 必然前进，不存在死循环。
 */
function collectSubstringRanges(
  haystack: string,
  needle: string,
): SearchRange[] {
  const ranges: SearchRange[] = [];
  let from = 0;
  while (from <= haystack.length - needle.length) {
    const index = haystack.indexOf(needle, from);
    if (index < 0) break;
    ranges.push({ start: index, end: index + needle.length });
    from = index + needle.length;
  }
  return ranges;
}

/**
 * 子序列匹配：needle 的字符按顺序出现即可，不必连续。
 * 复杂度 O(haystack.length)：贪心取每个字符的最早出现位置，命中区间是这些单位置区间。
 *
 * 按 UTF-16 码元逐位比较（与 String#indexOf 的索引口径一致，中文、emoji 都不会算错位）；
 *
 * @returns 命中时返回每个字符的区间（相邻位置先不合并），未命中返回 null
 */
function matchSubsequence(haystack: string, needle: string): SearchRange[] | null {
  const ranges: SearchRange[] = [];
  let cursor = 0;
  for (let index = 0; index < needle.length; index += 1) {
    const found = haystack.indexOf(needle[index], cursor);
    if (found < 0) return null;
    ranges.push({ start: found, end: found + 1 });
    cursor = found + 1;
  }
  return ranges;
}

/**
 * 收集单条正则的全部非零宽命中。
 *
 * 零宽命中（`a*`、`^`、`\b`）不产生高亮区间，但必须手动推进 lastIndex：
 * 全局正则的 exec 在零宽命中后不会自行前进，原地重试就是死循环。
 */
function collectRegexMatches(
  text: string,
  pattern: RegExp,
): { matched: boolean; ranges: SearchRange[] } {
  const ranges: SearchRange[] = [];
  let matched = false;
  // 正则在计划里被所有条目复用，每次进入都必须复位游标
  pattern.lastIndex = 0;
  let found = pattern.exec(text);
  while (found !== null) {
    matched = true;
    const start = found.index;
    const end = start + found[0].length;
    if (end > start) {
      ranges.push({ start, end });
    } else if (start >= text.length) {
      // 尾部零宽命中：后面没有位置可推进，直接收敛
      break;
    } else {
      pattern.lastIndex = start + 1;
    }
    found = pattern.exec(text);
  }
  return { matched, ranges };
}

/**
 * 归一化区间：丢弃非法/零宽/越界项，裁剪到文本长度，排序后合并重叠与相邻区间。
 *
 * @param ranges - 原始区间（可乱序、可重叠）
 * @param textLength - 目标文本长度，用于裁剪
 * @returns 新的区间数组，入参不被修改
 */
function mergeRanges(ranges: SearchRange[], textLength: number): SearchRange[] {
  const valid: SearchRange[] = [];
  for (const range of ranges) {
    const start = Math.max(0, Math.min(range.start, textLength));
    const end = Math.max(0, Math.min(range.end, textLength));
    if (end > start) valid.push({ start, end });
  }
  valid.sort((a, b) => a.start - b.start);

  const merged: SearchRange[] = [];
  for (const range of valid) {
    const last = merged[merged.length - 1];
    if (last !== undefined && range.start <= last.end) {
      last.end = Math.max(last.end, range.end);
      continue;
    }
    merged.push({ start: range.start, end: range.end });
  }
  return merged;
}

/**
 * 对单条文本执行匹配（`matchText` 的内核，查询计划由调用方复用时用这个）。
 *
 * 多词 AND：任一词项不命中即判定整条不命中。
 * 大小写不敏感：文本与小写化词项各自折叠后比较，中文不受影响。
 *
 * @param text - 目标文本
 * @param plan - 查询计划
 * @returns 是否命中 + 命中区间 + 降级提示
 */
export function matchTextWithPlan(text: string, plan: QueryPlan): TextMatch {
  if (plan.isEmpty) {
    return { matched: true, ranges: [], notice: plan.notice };
  }
  const miss: TextMatch = { matched: false, ranges: [], notice: plan.notice };
  const collected: SearchRange[] = [];

  if (plan.matcher === "regex") {
    for (const pattern of plan.patterns) {
      const result = collectRegexMatches(text, pattern);
      if (!result.matched) return miss;
      collected.push(...result.ranges);
    }
  } else {
    // 每条文本只折叠一次，不在词项循环里重复 toLowerCase
    const folded = text.toLowerCase();
    for (const token of plan.foldedTokens) {
      if (plan.matcher === "fuzzy") {
        const subsequence = matchSubsequence(folded, token);
        if (subsequence === null) return miss;
        collected.push(...subsequence);
        continue;
      }
      const occurrences = collectSubstringRanges(folded, token);
      if (occurrences.length === 0) return miss;
      collected.push(...occurrences);
    }
  }

  // 折叠可能改变个别特殊字符的长度（如土耳其语 İ），裁剪保证区间不越出原文本
  return { matched: true, ranges: mergeRanges(collected, text.length), notice: plan.notice };
}

/**
 * 按计划过滤条目并给出命中区间（主窗与 popover 的实际取数入口）。
 *
 * @param items - 全量条目，保持入参顺序
 * @param plan - 查询计划
 * @returns 命中的条目及其区间；空查询返回全部条目且区间为空
 */
export function searchClipsWithPlan(
  items: ClipItem[],
  plan: QueryPlan,
): SearchHit[] {
  const hits: SearchHit[] = [];
  for (const item of items) {
    const match = matchTextWithPlan(clipSearchText(item), plan);
    if (match.matched) hits.push({ item, ranges: match.ranges });
  }
  return hits;
}

/**
 * 搜索剪贴板条目：搜索对象是 `title ?? content`，命中判定与高亮同源。
 *
 * @param items - 全量条目，保持入参顺序
 * @param query - 用户搜索串；空串或纯空白返回全部条目且区间为空
 * @param mode - 搜索模式，缺省 mixed
 * @returns 命中的条目及其命中区间
 */
export function searchClips(
  items: ClipItem[],
  query: string,
  mode: SearchMode = DEFAULT_SEARCH_MODE,
): SearchHit[] {
  return searchClipsWithPlan(items, prepareQuery(query, mode));
}

/**
 * 把文本按命中区间切成「普通 / 命中」交替的片段，供渲染层包 <mark>。
 *
 * 纯数据，不产生 HTML：片段内容一律是原文本的子串，拼回去必然等于原文本。
 * 区间可乱序、可重叠、可越界，内部统一裁剪合并。
 *
 * @param text - 原文本
 * @param ranges - 命中区间；空数组时返回整段普通文本
 * @returns 覆盖整段文本的片段数组
 */
export function splitHighlight(
  text: string,
  ranges: SearchRange[],
): HighlightSegment[] {
  const merged = mergeRanges(ranges, text.length);
  if (merged.length === 0) return [{ text, hit: false }];

  const segments: HighlightSegment[] = [];
  let cursor = 0;
  for (const range of merged) {
    if (range.start > cursor) {
      segments.push({ text: text.slice(cursor, range.start), hit: false });
    }
    segments.push({ text: text.slice(range.start, range.end), hit: true });
    cursor = range.end;
  }
  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor), hit: false });
  }
  return segments;
}
