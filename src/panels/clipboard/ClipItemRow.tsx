/**
 * 剪贴板列表单行：类型图标 + 内容摘要（两行截断）+ 元信息 + 内联操作。
 * 外层用 div[role="option"] 而非 button，避免内部 button 产生非法嵌套。
 * 高亮态由 CSS [aria-selected="true"] 驱动，不用 inline style。
 * 删除操作统一在右侧预览区执行，列表行不重复提供。
 */

import type { ClipItem } from "../../ipc/ipc-client";
import { KindIcon } from "../../components/KindIcon";
import { clipSearchText, type SearchRange } from "../../clip-popover/clip-search";
import { HighlightedText } from "../../clip-popover/HighlightedText";
import { useClipItemThumbnail } from "./useClipThumbnail";

/** 内容摘要最大字符数 */
const SUMMARY_MAX_LENGTH = 60;

/** 收藏状态按钮文案：行内星标是纯图标按钮，状态靠 aria-label 表达 */
const FAVORITE_LABEL_ON = "取消收藏";
const FAVORITE_LABEL_OFF = "收藏";

/** 类型显示名：行内元信息只放一个词，完整说明留给预览区 */
const KIND_LABEL: Record<ClipItem["kind"], string> = {
  text: "纯文本",
  richtext: "富文本",
  image: "图片",
};

interface ClipItemRowProps {
  item: ClipItem;
  isHighlighted: boolean;
  /**
   * 搜索命中区间，作用于 `title ?? content`。
   * 缺省表示无搜索条件，整段按普通文本渲染。
   */
  hitRanges?: SearchRange[];
  onToggleFavorite: (item: ClipItem) => Promise<void>;
  onSelect: (item: ClipItem) => void;
}

/** 截断文本至指定长度，超出加省略号 */
function truncateSummary(text: string): string {
  if (text.length <= SUMMARY_MAX_LENGTH) return text;
  return text.slice(0, SUMMARY_MAX_LENGTH) + "…";
}

/** 两位补零 */
function pad2(value: number): string {
  return String(value).padStart(2, "0");
}

/**
 * 行内时间：今天 / 昨天只写相对日，更早补上日期。
 * 相对日比「07/12」更容易一眼判断新旧，又不至于让每行都带完整日期。
 */
function formatRowTime(utcMs: number): string {
  const date = new Date(utcMs);
  const now = new Date();
  const clock = `${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const startOfYesterday = startOfToday - 24 * 60 * 60 * 1000;
  if (utcMs >= startOfToday) return `今天 ${clock}`;
  if (utcMs >= startOfYesterday) return `昨天 ${clock}`;
  return `${pad2(date.getMonth() + 1)}/${pad2(date.getDate())} ${clock}`;
}

/** 图片内容区：缩略图按需拉取；未就绪或取不到时保持既有 `[图片]` 占位，不留空白跳动 */
function ImageContent({ item }: { item: ClipItem }) {
  const thumbnailUrl = useClipItemThumbnail(item);
  if (thumbnailUrl === null) {
    return <span>[图片]</span>;
  }
  return (
    <img
      src={thumbnailUrl}
      alt="图片缩略图"
      className="clip-thumb"
    />
  );
}

/** 收藏星标按钮：所有类型（含图片）均显示；点按不冒泡到整行选择 */
function StarButton({ item, onToggleFavorite }: { item: ClipItem; onToggleFavorite: (item: ClipItem) => Promise<void> }) {
  const label = item.isFavorite ? FAVORITE_LABEL_ON : FAVORITE_LABEL_OFF;
  return (
    <button
      className={item.isFavorite ? "icon-btn on" : "icon-btn"}
      aria-label={label}
      aria-pressed={item.isFavorite}
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        onToggleFavorite(item);
      }}
    >
      {/*
        收藏星刻意不挂 base.css 的 .ico 基类：它是「空心 / 实心」两态切换，
        而 .ico 声明了 fill: none（样式表优先级高于 SVG 呈现属性），
        挂上去会把已收藏的实心星压回空心。描边数值与 .ico 保持一致，仅此一处例外。
      */}
      <svg
        viewBox="0 0 24 24"
        fill={item.isFavorite ? "currentColor" : "none"}
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="m12 3.5 2.6 5.6 6 .8-4.4 4.2 1.1 6-5.3-2.9-5.3 2.9 1.1-6L3.4 9.9l6-.8z" />
      </svg>
    </button>
  );
}

/** 剪贴板列表单行子组件 */
export function ClipItemRow({
  item,
  isHighlighted,
  hitRanges,
  onToggleFavorite,
  onSelect,
}: ClipItemRowProps) {
  // richtext 的正文在列表里按等宽呈现：代码 / 富文本片段用同一套字宽更容易扫读
  const isCode = item.kind === "richtext";

  return (
    <div
      role="option"
      aria-selected={isHighlighted}
      className="clip-row"
      onClick={() => { onSelect(item); }}
    >
      <span className="clip-kind">
        <KindIcon kind={item.kind} />
      </span>
      <div className="clip-main">
        {item.kind === "image" ? (
          <div className="clip-text clip-text--image body-6">
            <ImageContent item={item} />
          </div>
        ) : (
          <div className={isCode ? "clip-text body-6 mono" : "clip-text body-6"}>
            {/* 先按既有上限截断再切分高亮：区间落在被截掉的部分时会被自动丢弃 */}
            <HighlightedText text={truncateSummary(clipSearchText(item))} ranges={hitRanges} />
          </div>
        )}
        <div className="clip-meta caption-4">
          <span className="mono">{formatRowTime(item.lastModifiedUtc)}</span>
          <i className="c-dot" aria-hidden="true" />
          <span>{KIND_LABEL[item.kind]}</span>
          {item.isFavorite && (
            <>
              <i className="c-dot" aria-hidden="true" />
              <span>已收藏</span>
            </>
          )}
        </div>
      </div>
      <div className="clip-row-acts">
        <StarButton item={item} onToggleFavorite={onToggleFavorite} />
      </div>
    </div>
  );
}
