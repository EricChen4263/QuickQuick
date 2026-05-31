/**
 * 剪贴板历史页（双栏布局：左侧列表 + 右侧预览）。
 * 挂载时通过 IPC 取数，支持搜索过滤、类型筛选、键盘流、收藏/删除管理。
 * 对应验收项 V1-F2-A07。
 */

import { useEffect, useState, useCallback } from "react";
import {
  listClipItems,
  deleteClipItem,
  toggleFavoriteClip,
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

/** 将 IPC ClipItem 适配为纯逻辑 HistoryItem */
function toHistoryItem(clip: ClipItem): HistoryItem {
  return {
    id: clip.id,
    text: clip.content,
    kind: clip.kind === "richtext" ? "richtext" : "text",
  };
}

/** 剪贴板历史页根组件 */
function ClipboardPage() {
  const [items, setItems] = useState<ClipItem[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [typeFilter, setTypeFilter] = useState<HistoryFilter>("all");
  const [highlightIndex, setHighlightIndex] = useState(0);

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
      setHighlightIndex((prev) => moveHighlight(prev, event.key as "ArrowUp" | "ArrowDown", filteredItems.length));
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

  if (loadError !== null) {
    return <div role="alert">{loadError}</div>;
  }

  return (
    <div style={{ display: "flex", height: "100%", fontFamily: "var(--qq-font)", flexDirection: "column" }}>
      {opError !== null && (
        <div role="alert" style={{ padding: "8px 12px", color: "var(--qq-danger, #c0392b)", background: "var(--qq-surface)" }}>
          {opError}
        </div>
      )}
      <div style={{ display: "flex", flex: 1, minHeight: 0 }}>
        <div style={{ display: "flex", flexDirection: "column", flex: "0 0 320px" }}>
          <ClipSearchBar
            searchQuery={searchQuery}
            typeFilter={typeFilter}
            onSearchChange={(q) => { setSearchQuery(q); setHighlightIndex(0); }}
            onTypeFilterChange={(f) => { setTypeFilter(f); setHighlightIndex(0); }}
            onKeyDown={handleKeyDown}
          />
          <div style={{ flex: 1, overflowY: "auto" }}>
            {filteredItems.length === 0 ? (
              <p style={{ padding: "16px", color: "var(--qq-text-muted)" }}>
                {EMPTY_LIST_PLACEHOLDER}
              </p>
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
                    onDelete={handleDelete}
                  />
                );
              })
            )}
          </div>
        </div>
        <ClipPreview item={highlightedClipItem} />
      </div>
    </div>
  );
}

/** 列表为空时的占位文案 */
const EMPTY_LIST_PLACEHOLDER = "暂无剪贴板记录";

export default ClipboardPage;
