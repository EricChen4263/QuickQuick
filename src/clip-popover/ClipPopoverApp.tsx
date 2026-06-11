import { useState, useEffect, useMemo, useRef } from "react";
import type { ClipItem } from "../ipc/ipc-client";
import { listClipItems, pasteToFront, hideAndReturnFocus, copyClipToClipboard } from "../ipc/ipc-client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { CLIPBOARD_CHANGED_EVENT } from "../ipc/events";
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
 *   - Alt+Enter：copyClipToClipboard(selectedItem.id) 成功后 hide 窗口（富文本保真）
 *   - Esc：由 main.tsx 的全局快捷键处理，不在此组件内
 */
export default function ClipPopoverApp() {
  const [items, setItems] = useState<ClipItem[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const inputRef = useRef<HTMLInputElement>(null);
  // ref 暴露最新的 visibleFlatList，避免 onFocusChanged 回调产生 stale closure
  const visibleFlatListRef = useRef<ClipItem[]>([]);

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

  // 订阅后端 clipboard-changed 事件，有新条目或条目置顶时重新拉取列表。
  // 弹窗窗口常驻（hide/show 复用、组件不重挂载），无此订阅则列表停在打开时的旧快照。
  // 采用与 ClipboardPage 相同的 cancelled+unlisten 范式，防止卸载后泄漏。
  // 只 setItems，不抢占用户当前选中：选中项校正交由下方 useEffect 处理
  // （选中项还在则保留、不在了回第一项）。
  useEffect(() => {
    const cancelled = { current: false };
    let unlisten: (() => void) | undefined;
    listen(CLIPBOARD_CHANGED_EVENT, () => {
      listClipItems()
        .then((loaded) => {
          if (cancelled.current) return;
          setItems(loaded);
        })
        .catch((err: unknown) => {
          console.error("[clip-popover] clipboard-changed 刷新失败:", err);
        });
    })
      .then((fn) => {
        if (cancelled.current) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err: unknown) => {
        console.error("[clip-popover] clipboard-changed 监听注册失败:", err);
      });
    return () => {
      cancelled.current = true;
      unlisten?.();
    };
  }, []);

  // 每次窗口获得焦点时：聚焦输入框、重置搜索、选中回第一项
  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | null = null;

    win.onFocusChanged(({ payload: focused }) => {
      if (!focused) return;
      inputRef.current?.focus();
      setQuery("");
      const firstId = visibleFlatListRef.current[0]?.id ?? null;
      setSelectedId(firstId);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  const groups = useMemo(() => {
    const filtered = filterClipBySearch(items, query);
    return groupClipItems(filtered, Date.now());
  }, [items, query]);

  const visibleFlatList = useMemo(() => buildFlatList(groups), [groups]);

  // 同步 ref，使 onFocusChanged 回调始终能读到最新列表（避免 stale closure）
  visibleFlatListRef.current = visibleFlatList;

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
      // 走后端 IPC 按 id 取内容写系统剪贴板：文本保真富文本格式，图片解码原图 PNG 后写 set_image。
      copyClipToClipboard(selectedItem.id)
        .then(() => getCurrentWindow().hide())
        .catch((err: unknown) => {
          console.error("[clip-popover] copy failed:", err);
        });
      return;
    }

    if (e.key === "Escape") {
      // WKWebView 会把 Esc 键原生"清空 search input"行为吞掉，不冒泡到 document。
      // 在 onKeyDown 处显式拦截并关窗，才能让 Esc 在输入框已获焦时正常工作。
      // 走 hideAndReturnFocus 而非裸 hide：关闭面板同时把焦点还给上一个外部 app（方案 C）。
      e.preventDefault();
      hideAndReturnFocus().catch((err: unknown) => {
        console.error("[clip-popover] hideAndReturnFocus on Esc failed:", err);
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
          ref={inputRef}
          type="text"
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
