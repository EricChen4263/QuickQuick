import { useState, useEffect, useMemo } from "react";
import type { ClipItem } from "../ipc/ipc-client";
import { listClipItems, pasteToFront } from "../ipc/ipc-client";
import { writeToClipboard } from "../panels/translate/browser-api";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { filterClipBySearch, groupClipItems } from "./grouping";
import { advanceSelection } from "./keyboard-nav";
import { PopoverList } from "./PopoverList";
import { PopoverPreview } from "./PopoverPreview";
import { PopoverFooter } from "./PopoverFooter";

/**
 * 从分组结果构建扁平有序列表（收藏 → 今天 → 更早）。
 * B2 键盘流用此列表做 ↑↓ 导航，无需重新计算顺序。
 */
function buildFlatList(groups: ReturnType<typeof groupClipItems>): ClipItem[] {
  return [...groups.favorites, ...groups.today, ...groups.earlier];
}

/**
 * Popover 根组件。
 *
 * 状态：
 *   - items: IPC 加载的全量条目
 *   - query: 搜索框值（受控）
 *   - selectedId: 当前选中条目 ID
 *
 * 键盘交互（由 search input 的 onKeyDown 处理）：
 *   - ↑ / ↓：在 visibleFlatList 中移动选中项（advanceSelection）
 *   - Enter：pasteToFront(selectedId) 成功后 hide 窗口
 *   - Alt+Enter：writeToClipboard(selectedItem.content) 成功后 hide 窗口
 *   - Esc：由 main.tsx 的全局快捷键处理，不在此组件内
 */
export default function ClipPopoverApp() {
  const [items, setItems] = useState<ClipItem[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  useEffect(() => {
    listClipItems()
      .then((loaded) => {
        setItems(loaded);
        if (loaded.length > 0) {
          setSelectedId(loaded[0].id);
        }
      })
      .catch((err: unknown) => {
        setLoadError(err instanceof Error ? err.message : String(err));
      });
  }, []);

  const groups = useMemo(() => {
    const filtered = filterClipBySearch(items, query);
    return groupClipItems(filtered, Date.now());
  }, [items, query]);

  const visibleFlatList = useMemo(() => buildFlatList(groups), [groups]);

  // 过滤后当前 selectedId 可能不在新列表中，自动重置到第一项
  useEffect(() => {
    if (visibleFlatList.length === 0) {
      setSelectedId(null);
      return;
    }
    const stillVisible = visibleFlatList.some((i) => i.id === selectedId);
    if (!stillVisible) {
      setSelectedId(visibleFlatList[0].id);
    }
  }, [visibleFlatList, selectedId]);

  const selectedItem = useMemo(
    () => visibleFlatList.find((i) => i.id === selectedId) ?? null,
    [visibleFlatList, selectedId]
  );

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>): void {
    const flatIds = visibleFlatList.map((i) => i.id);

    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedId(advanceSelection(selectedId, e.key, flatIds));
      return;
    }

    if (e.key === "Enter" && !e.altKey) {
      e.preventDefault();
      if (selectedId === null || selectedItem === null) return;
      pasteToFront(selectedId)
        .then(() => getCurrentWindow().hide())
        .catch((err: unknown) => {
          console.error("[clip-popover] paste failed:", err);
        });
      return;
    }

    if (e.key === "Enter" && e.altKey) {
      e.preventDefault();
      if (selectedItem === null) return;
      // 图片条目 content 为空字符串，写入会静默破坏剪贴板，做 no-op
      if (selectedItem.kind === "image") return;
      writeToClipboard(selectedItem.content)
        .then(() => getCurrentWindow().hide())
        .catch((err: unknown) => {
          console.error("[clip-popover] copy failed:", err);
        });
    }
  }

  if (loadError) {
    return (
      <div className="popover-error">
        加载失败：{loadError}
      </div>
    );
  }

  return (
    <div className="popover-shell">
      <div className="popover-search-row">
        <input
          type="search"
          role="searchbox"
          className="popover-search-input"
          placeholder="搜索剪贴板…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          autoFocus
          aria-label="搜索剪贴板"
        />
      </div>
      <div className="popover-body">
        <PopoverList
          groups={groups}
          selectedId={selectedId}
          onSelect={(item) => setSelectedId(item.id)}
        />
        <PopoverPreview item={selectedItem} />
      </div>
      <PopoverFooter />
    </div>
  );
}
