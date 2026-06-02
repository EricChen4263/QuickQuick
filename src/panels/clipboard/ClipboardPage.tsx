/**
 * 剪贴板历史页（双栏布局：左侧 340px 列表栏 + 右侧预览区）。
 * 挂载时通过 IPC 取数，支持搜索过滤、类型筛选、键盘流、收藏/删除管理。
 * 对应验收项 V1-F2-A07。
 */

import { useEffect, useState, useCallback } from "react";
import {
  listClipItems,
  deleteClipItem,
  toggleFavoriteClip,
  pasteToFront,
  openAccessibilitySettings,
  type ClipItem,
} from "../../ipc/ipc-client";
import type { HistoryItem } from "../history/search";
import type { HistoryFilter } from "../history/filter";
import { filterBySearch } from "../history/search";
import { filterByType } from "../history/filter";
import { moveHighlight, quickSelectIndex, resolveEnter } from "../history/keyboard";
import { ClipItemRow } from "./ClipItemRow";
import { ClipPreview } from "./ClipPreview";
import { ClipSearchBar } from "./ClipSearchBar";
import { OnboardCard } from "./OnboardCard";
import EmptyState from "../../components/EmptyState";
import "./clipboard.css";

/** localStorage key：用户已关闭引导卡片 */
const ONBOARD_DISMISSED_KEY = "qq-onboard-dismissed";

/** 将 IPC ClipItem 适配为纯逻辑 HistoryItem */
function toHistoryItem(clip: ClipItem): HistoryItem {
  if (clip.kind === "image") {
    return { id: clip.id, text: clip.content, kind: "image" };
  }
  return {
    id: clip.id,
    text: clip.content,
    kind: clip.kind === "richtext" ? "richtext" : "text",
  };
}

/** 空列表图标 SVG */
const EmptyListIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <rect x="8" y="2" width="8" height="4" rx="1" />
    <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
  </svg>
);

interface ClipboardPageProps {
  onTranslateItem?: (content: string) => void;
}

/** 剪贴板历史页根组件 */
function ClipboardPage({ onTranslateItem }: ClipboardPageProps) {
  const [items, setItems] = useState<ClipItem[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [typeFilter, setTypeFilter] = useState<HistoryFilter>("all");
  const [highlightIndex, setHighlightIndex] = useState(0);
  const [onboardDismissed, setOnboardDismissed] = useState(
    () => localStorage.getItem(ONBOARD_DISMISSED_KEY) === "true",
  );
  const [pasteMsg, setPasteMsg] = useState<string | null>(null);

  /**
   * loadItems 接受 cancelled ref，避免卸载后写入已卸载组件的 state。
   * useEffect cleanup 将 cancelled 置 true，async resolve 时 guard 拦截。
   */
  const loadItems = useCallback(
    async (cancelled: { current: boolean }) => {
      try {
        const result = await listClipItems();
        if (cancelled.current) return;
        setItems(result);
        setLoadError(null);
      } catch {
        if (cancelled.current) return;
        setLoadError("加载失败，请稍后重试");
      }
    },
    [],
  );

  useEffect(() => {
    const cancelled = { current: false };
    loadItems(cancelled);
    return () => {
      cancelled.current = true;
    };
  }, [loadItems]);

  // 两步过滤：先搜索词，再类型
  const historyItems = items.map(toHistoryItem);
  const afterSearch = filterBySearch(historyItems, searchQuery);
  const filteredItems = filterByType(afterSearch, typeFilter);

  // 高亮索引 clamp 到有效范围
  const safeHighlight =
    filteredItems.length === 0
      ? -1
      : Math.min(Math.max(highlightIndex, 0), filteredItems.length - 1);

  const highlightedClipItem =
    safeHighlight >= 0
      ? items.find((c) => c.id === filteredItems[safeHighlight].id) ?? null
      : null;

  function handleKeyDown(event: React.KeyboardEvent) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      setHighlightIndex((prev) =>
        moveHighlight(prev, event.key as "ArrowUp" | "ArrowDown", filteredItems.length),
      );
      return;
    }
    if (event.key === "Enter") {
      resolveEnter(safeHighlight, filteredItems);
      return;
    }
    if (event.metaKey || event.ctrlKey) {
      const idx = quickSelectIndex(event.key);
      if (idx !== null) {
        setHighlightIndex(idx);
      }
    }
  }

  async function handleToggleFavorite(item: ClipItem): Promise<void> {
    try {
      await toggleFavoriteClip(item.id, !item.isFavorite);
      const cancelled = { current: false };
      await loadItems(cancelled);
    } catch {
      setOpError("操作失败，请稍后重试");
    }
  }

  async function handleDelete(item: ClipItem): Promise<void> {
    try {
      await deleteClipItem(item.id);
      const cancelled = { current: false };
      await loadItems(cancelled);
    } catch {
      setOpError("操作失败，请稍后重试");
    }
  }

  function handleDismissOnboard() {
    localStorage.setItem(ONBOARD_DISMISSED_KEY, "true");
    setOnboardDismissed(true);
  }

  async function handlePasteToFront(item: ClipItem): Promise<void> {
    setPasteMsg(null);
    try {
      const result = await pasteToFront(item.id);
      if (result.outcome === "write_back_only") {
        setPasteMsg("已复制到剪贴板，请手动粘贴（未授权辅助功能）");
      }
    } catch {
      setOpError("粘贴失败，请稍后重试");
    }
  }

  async function handleOpenSystemSettings(): Promise<void> {
    try {
      await openAccessibilitySettings();
    } catch {
      setOpError("打开系统设置失败，请稍后重试");
    }
  }

  if (loadError !== null) {
    return <div role="alert">{loadError}</div>;
  }

  return (
    <div style={{ display: "grid", gridTemplateColumns: "340px 1fr", height: "100%", minHeight: 0, overflow: "hidden", fontFamily: "var(--font)" }}>
      {opError !== null && (
        <div
          role="alert"
          style={{ gridColumn: "1 / -1", padding: "8px 12px", color: "var(--danger)", background: "var(--surface)" }}
        >
          {opError}
        </div>
      )}
      <div className="clip-list-col">
        <ClipSearchBar
          searchQuery={searchQuery}
          typeFilter={typeFilter}
          onSearchChange={(q) => { setSearchQuery(q); setHighlightIndex(0); }}
          onTypeFilterChange={(f) => { setTypeFilter(f); setHighlightIndex(0); }}
          onKeyDown={handleKeyDown}
        />
        {!onboardDismissed && (
          <OnboardCard
            onDismiss={handleDismissOnboard}
            onOpenSystemSettings={() => { void handleOpenSystemSettings(); }}
          />
        )}
        <div
          className="clip-list"
          role="listbox"
          aria-label="剪贴板历史"
        >
          {filteredItems.length === 0 ? (
            <EmptyState
              icon={EmptyListIcon}
              title="暂无记录"
              description="复制任意内容后将显示在这里"
            />
          ) : (
            filteredItems.map((histItem, idx) => {
              const clipItem = items.find((c) => c.id === histItem.id);
              if (!clipItem) return null;
              return (
                <ClipItemRow
                  key={histItem.id}
                  item={clipItem}
                  isHighlighted={idx === safeHighlight}
                  onToggleFavorite={handleToggleFavorite}
                  onSelect={() => { setHighlightIndex(idx); }}
                />
              );
            })
          )}
        </div>
      </div>
      {pasteMsg !== null && (
        <div
          style={{ gridColumn: "1 / -1", padding: "8px 12px", fontSize: 12, color: "var(--muted)", background: "var(--surface)" }}
        >
          {pasteMsg}
        </div>
      )}
      <ClipPreview
        item={highlightedClipItem}
        onToggleFavorite={handleToggleFavorite}
        onDelete={handleDelete}
        onCopy={(_item) => { /* 复制逻辑在 ClipPreview 内部调用 writeToClipboard 完成 */ }}
        onPasteToFront={(item) => { void handlePasteToFront(item); }}
        onTranslate={(item) => { onTranslateItem?.(item.content); }}
      />
    </div>
  );
}

export default ClipboardPage;
