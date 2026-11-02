/**
 * 右侧预览区：展示当前高亮剪贴板条目的完整内容。
 * 图片项由 ImagePreview 子组件处理原图加载（先缩略图后原图）。
 * 无高亮 / 首次取数未完成时给占位视图，绝不继续显示上一条内容。
 *
 * 预览区是剪贴板页**唯一**的抬升面（--surface + --shadow-panel）：
 * 它承载「阅读」这件事，列表栏保持扁平。层次由背景色差表达，不靠边框三件套。
 */

import { Fragment, useEffect, useState } from "react";
import { copyClipToClipboard, getClipImageOriginal, type ClipItem } from "../../ipc/ipc-client";
import EmptyState from "../../components/EmptyState";
import { ActionBar } from "../../components/ActionBar";
import { sanitizeRichHtml } from "./sanitize-html";
import { handleRichLinkClick } from "./rich-link";
import { useClipThumbnail } from "./useClipThumbnail";
import { ImageViewer } from "../../components/ImageViewer";

interface ClipPreviewProps {
  /**
   * 左侧列表里是否还有任何条目（不受搜索 / 筛选影响）。
   * 用来区分「库是空的」与「有内容但没选中」——两种情况下右侧该说的话不一样。
   */
  hasAnyClip?: boolean;
  item: ClipItem | null;
  /** 首次取数尚未落地：此时右侧提示「正在读取」，不展示任何旧内容 */
  isLoading?: boolean;
  onToggleFavorite: (item: ClipItem) => void;
  onDelete: (item: ClipItem) => void;
  onCopy: (item: ClipItem) => void;
  /** 里程碑3接入：粘贴到前台窗口（IPC paste）*/
  onPasteToFront: (item: ClipItem) => void;
  /** 里程碑3接入：跳转翻译页并填入内容 */
  onTranslate: (item: ClipItem) => void;
}

interface ImagePreviewProps {
  imageId: string;
  hasThumbnail: boolean;
  /** 点击预览图片：请求以大图覆盖层查看 */
  onOpen: () => void;
}

/** 两位补零 */
function pad2(value: number): string {
  return String(value).padStart(2, "0");
}

/** 格式化 UTC 时间戳为本地可读字符串（今天 / 昨天用相对日，更早带日期） */
function formatTimestamp(utcMs: number): string {
  const date = new Date(utcMs);
  const now = new Date();
  const clock = `${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  if (utcMs >= startOfToday) return `今天 ${clock}`;
  if (utcMs >= startOfToday - 24 * 60 * 60 * 1000) return `昨天 ${clock}`;
  return `${pad2(date.getMonth() + 1)}/${pad2(date.getDate())} ${clock}`;
}

/** 类型显示名映射 */
const KIND_LABEL: Record<ClipItem["kind"], string> = {
  text: "纯文本",
  richtext: "富文本",
  image: "图片",
};

/**
 * 图片预览子组件：原图优先、缩略图回落。
 * 缩略图按需拉取（模块级缓存），原图异步到达后覆盖缩略图；
 * 两者都还没就绪时显示 `[图片]` 占位，避免图片到达时布局跳动。
 * effect 内局部 cancelled 闭包防止切换条目时产生 stale 更新。
 */
function ImagePreview({ imageId, hasThumbnail, onOpen }: ImagePreviewProps) {
  const [originalDataUrl, setOriginalDataUrl] = useState<string | null>(null);
  const thumbnailUrl = useClipThumbnail(hasThumbnail ? imageId : null);

  useEffect(() => {
    const cancelled = { current: false };
    setOriginalDataUrl(null);

    getClipImageOriginal(imageId)
      .then((url) => {
        if (cancelled.current) return;
        setOriginalDataUrl(url);
      })
      .catch(() => {
        if (cancelled.current) return;
        setOriginalDataUrl(null);
      });

    return () => {
      cancelled.current = true;
    };
  }, [imageId]);

  const displayUrl = originalDataUrl ?? thumbnailUrl;

  if (displayUrl === null) {
    return <div className="preview-img-placeholder body-6">[图片]</div>;
  }

  return (
    <img
      src={displayUrl}
      alt="图片预览"
      className="preview-img"
      onClick={onOpen}
    />
  );
}

/** 空态图标 SVG（24 网格 / 1.5 描边 / 圆头圆角） */
const EmptyIcon = (
  <svg className="ico ico-20" viewBox="0 0 24 24" aria-hidden="true">
    <rect x="3.5" y="4.5" width="17" height="15" rx="2" />
    <path d="M3.5 9.5h17" />
    <path d="M8 4.5v5" />
  </svg>
);

/** 粘贴到前台图标 */
const PasteIcon = (
  <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
    <path d="M12 3.5v11" />
    <path d="m8 10.5 4 4 4-4" />
    <path d="M4.5 15.5v3a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-3" />
  </svg>
);

/** 复制图标 */
const CopyIcon = (
  <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
    <rect x="9" y="9" width="11" height="11" rx="2" />
    <path d="M5 15.5V6a2 2 0 0 1 2-2h9.5" />
  </svg>
);

/** 翻译图标 */
const TranslateIcon = (
  <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
    <path d="M4 6h9" />
    <path d="M8 3.5v2.5" />
    <path d="M12 6c0 4-3 7-8 8" />
    <path d="m13.5 20 3.5-9 3.5 9" />
    <path d="M15 17h4.5" />
  </svg>
);

/** 收藏星（描边；已收藏时由外层 .on 换色，形状不变） */
const StarIcon = (
  <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
    <path d="m12 3.5 2.6 5.6 6 .8-4.4 4.2 1.1 6-5.3-2.9-5.3 2.9 1.1-6L3.4 9.9l6-.8z" />
  </svg>
);

/** 垃圾桶图标 */
const TrashIcon = (
  <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
    <path d="M4 7h16" />
    <path d="M9.5 7V5.2A1.2 1.2 0 0 1 10.7 4h2.6a1.2 1.2 0 0 1 1.2 1.2V7" />
    <path d="m6.5 7 .9 12h9.2l.9-12" />
  </svg>
);

/**
 * 预览底部操作行：主按钮「粘贴到前台」+ ghost 文案按钮（复制 / 翻译 / 收藏）
 * + 右侧 danger「删除」。收藏用 aria-pressed 表达状态，文案恒为「收藏」，
 * 避免按钮文字在两种状态间跳变导致宽度抖动。
 */
function PreviewActions({ item, onToggleFavorite, onDelete, onCopy, onPasteToFront, onTranslate }: {
  item: ClipItem;
  onToggleFavorite: (item: ClipItem) => void;
  onDelete: (item: ClipItem) => void;
  onCopy: (item: ClipItem) => void;
  onPasteToFront: (item: ClipItem) => void;
  onTranslate: (item: ClipItem) => void;
}) {
  function handleCopy() {
    // 走后端 IPC 按 id 取 text+html 写系统剪贴板，保真富文本格式（纯文本项 html=None 走 set_text，行为等价）。
    copyClipToClipboard(item.id).catch(() => {
      // 复制失败静默处理，不影响主流程
    });
    onCopy(item);
  }

  return (
    <ActionBar variant="surface" className="preview-actions">
      <button
        className="btn btn-primary button-sm"
        type="button"
        aria-label="粘贴到前台"
        onClick={() => { onPasteToFront(item); }}
      >
        {PasteIcon}
        粘贴到前台
      </button>
      <button
        className="btn btn-ghost btn-sm button-sm"
        type="button"
        aria-label="复制"
        onClick={handleCopy}
      >
        {CopyIcon}
        复制
      </button>
      {item.kind !== "image" && (
        <button
          className="btn btn-ghost btn-sm button-sm"
          type="button"
          aria-label="一键翻译"
          onClick={() => { onTranslate(item); }}
        >
          {TranslateIcon}
          一键翻译
        </button>
      )}
      <button
        className={item.isFavorite ? "btn btn-ghost btn-sm button-sm on" : "btn btn-ghost btn-sm button-sm"}
        type="button"
        aria-label="收藏"
        aria-pressed={item.isFavorite}
        onClick={() => { onToggleFavorite(item); }}
      >
        {StarIcon}
        收藏
      </button>
      <button
        className="btn btn-danger btn-sm button-sm"
        type="button"
        aria-label="删除"
        onClick={() => { onDelete(item); }}
      >
        {TrashIcon}
        删除
      </button>
    </ActionBar>
  );
}

/**
 * 文本/富文本内容区：富文本经 sanitize 后用 dangerouslySetInnerHTML 渲染保留格式，
 * 纯文本走 React 默认转义渲染。sanitize 在入 DOM 前完成（设计 §五 XSS 红线）。
 */
function PreviewContent({ item }: { item: ClipItem }) {
  if (item.kind === "richtext" && item.htmlContent !== undefined) {
    return (
      <div
        className="preview-content body-3"
        onClick={handleRichLinkClick}
        dangerouslySetInnerHTML={{ __html: sanitizeRichHtml(item.htmlContent) }}
      />
    );
  }
  // 无 HTML 的富文本按「代码片段」呈现：等宽 + 内嵌浅底，和正文拉开但不做成第二张卡片
  if (item.kind === "richtext") {
    return <div className="preview-content body-3 mono">{item.content}</div>;
  }
  return <div className="preview-content body-3">{item.content}</div>;
}

/** 属性表行：只列真实存在的字段，缺失的来源 / 哈希不伪造 */
interface MetaRow {
  label: string;
  value: string;
  sans?: boolean;
}

/** 组装属性表：字符数 / 行数只对文本类有意义，类型与更新时间对所有条目可用 */
function buildMetaRows(item: ClipItem): MetaRow[] {
  const rows: MetaRow[] = [];
  if (item.kind !== "image") {
    rows.push({ label: "字符数", value: String(item.content.length) });
    rows.push({ label: "行数", value: String(item.content.split("\n").length) });
  }
  rows.push({ label: "类型", value: KIND_LABEL[item.kind], sans: true });
  rows.push({ label: "更新时间", value: formatTimestamp(item.lastModifiedUtc) });
  return rows;
}

/** 右侧预览子组件 */
export function ClipPreview({
  item,
  isLoading = false,
  hasAnyClip = true,
  onToggleFavorite,
  onDelete,
  onCopy,
  onPasteToFront,
  onTranslate,
}: ClipPreviewProps) {
  // 大图覆盖层只属于打开它时的那一条：切换条目即收起，避免盖着的图与当前选中项对不上
  const [viewerItemId, setViewerItemId] = useState<string | null>(null);
  useEffect(() => {
    setViewerItemId(null);
  }, [item?.id]);
  const viewerImageId =
    item !== null && item.kind === "image" && item.imageId !== undefined && item.id === viewerItemId
      ? item.imageId
      : null;

  return (
    <div
      role="region"
      aria-label="预览"
      className="clip-preview"
      data-state={isLoading ? "loading" : item === null ? "empty" : "ready"}
    >
      {isLoading ? (
        // 首屏取数未完成：宁可说「正在读取」，也不把上一条内容继续挂在这里
        <div className="preview-blank" aria-busy="true">
          <p className="body-6">正在读取本地历史…</p>
        </div>
      ) : item === null ? (
        // 文案分两种：库里本来就没有内容时，「选中左侧一条记录」指向一个不存在的东西。
        <EmptyState
          icon={EmptyIcon}
          title={hasAnyClip === false ? "还没有可预览的内容" : "没有可预览的内容"}
          description={
            hasAnyClip === false
              ? "复制任意内容后，这里会显示它的完整内容与可用操作。"
              : "选中左侧任意一条记录，这里会显示完整内容与可用操作。"
          }
        />
      ) : (
        <>
          <div className="preview-body">
            <div className="preview-kicker caption-4">
              <span className="badge caption-4">{KIND_LABEL[item.kind]}</span>
              <i className="c-dot" aria-hidden="true" />
              <span className="mono">{formatTimestamp(item.lastModifiedUtc)}</span>
              {item.isFavorite && (
                <>
                  <i className="c-dot" aria-hidden="true" />
                  <span className="badge badge-fav caption-4">已收藏</span>
                </>
              )}
            </div>
            {item.kind === "image" && item.imageId !== undefined ? (
              <ImagePreview
                key={item.imageId}
                imageId={item.imageId}
                hasThumbnail={item.hasThumbnail}
                onOpen={() => setViewerItemId(item.id)}
              />
            ) : (
              <PreviewContent item={item} />
            )}
            <dl className="preview-meta caption-4">
              {buildMetaRows(item).map((row) => (
                // <dl> 里用 Fragment 而不是包一层 div：dt/dd 必须是 dl 的直接子项，
                // 属性表的双列 grid 才能对齐
                <Fragment key={row.label}>
                  <dt>{row.label}</dt>
                  <dd className={row.sans === true ? "sans" : undefined}>{row.value}</dd>
                </Fragment>
              ))}
            </dl>
          </div>
          <PreviewActions
            item={item}
            onToggleFavorite={onToggleFavorite}
            onDelete={onDelete}
            onCopy={onCopy}
            onPasteToFront={onPasteToFront}
            onTranslate={onTranslate}
          />
        </>
      )}
      {viewerImageId !== null && (
        <ImageViewer imageId={viewerImageId} onClose={() => setViewerItemId(null)} />
      )}
    </div>
  );
}
