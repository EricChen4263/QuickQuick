import { useState, useEffect, useMemo } from "react";
import type { ClipItem } from "../ipc/ipc-client";
import { listClipItems } from "../ipc/ipc-client";
import { filterClipBySearch, groupClipItems } from "./grouping";
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
 * Popover 根组件（Batch B1）
 *
 * 状态持有：
 *   - items: 从 IPC 加载的全量条目
 *   - query: 搜索框值（受控）
 *   - selectedId: 当前选中条目 ID
 *
 * B2 衔接：
 *   - visibleFlatList（由 buildFlatList 得到）是键盘 ↑↓ 的遍历序列
 *   - selectedId / setSelectedId 直接传入 PopoverList；B2 键盘 handler 调同一 setter
 *   - 粘贴动作：import { pasteToFront } from "../ipc/ipc-client"，调 pasteToFront(selectedId)
 *   - 复制动作：import { writeToClipboard } from "../ipc/ipc-client"（若已有）或走 Tauri clipboard API
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
