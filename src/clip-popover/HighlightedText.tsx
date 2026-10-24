/**
 * 命中高亮渲染：把命中区间切成交替的文本片段，命中片段包 <mark>。
 *
 * 只做 React 文本切分，既不用 dangerouslySetInnerHTML，也不拼接 HTML 字符串——
 * 条目内容里的 `<script>`、`&`、引号一律按普通文本渲染。
 */

import { splitHighlight, type SearchRange } from "./clip-search";

interface HighlightedTextProps {
  /** 已按展示口径截断好的文本（命中区间需相对于这段文本） */
  text: string;
  /** 命中区间；缺省或空数组时整段按普通文本渲染 */
  ranges?: SearchRange[];
}

/** 按命中区间渲染文本；高亮样式由 `.clip-hit` 承担，不写内联样式 */
export function HighlightedText({ text, ranges }: HighlightedTextProps) {
  const segments = splitHighlight(text, ranges ?? []);
  return (
    <>
      {segments.map((segment, index) =>
        segment.hit ? (
          <mark className="clip-hit" key={index}>
            {segment.text}
          </mark>
        ) : (
          segment.text
        ),
      )}
    </>
  );
}
