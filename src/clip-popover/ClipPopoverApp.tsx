import { useState, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import type { ClipItem } from "../ipc/ipc-client";
import { listClipItems, pasteToFront, hideAndReturnFocus, copyClipToClipboard } from "../ipc/ipc-client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { CLIPBOARD_CHANGED_EVENT } from "../ipc/events";
import { filterClipBySearch, groupClipItems } from "./grouping";
import { advanceSelection } from "./keyboard-nav";
import {
  isImeComposing,
  directPickIndex,
  isDirectPickAllowed,
  applyDeleteWord,
  applyDeleteChar,
  ctrlMoveDirection,
  firstIndex,
  lastIndex,
  type TextEdit,
} from "../panels/history/keyboard";
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
 * 数字直达的高亮反馈时长：先把目标条目亮起，让用户确认「按对了」，再执行取用。
 */
const DIRECT_PICK_FLASH_MS = 50;

/** 数字直达只认单个数字字符（1–9 与 0），其余按键不进入该语义 */
const DIGIT_KEY_PATTERN = /^[0-9]$/;

/**
 * Popover 根组件。
 *
 * 状态：
 *   - items: IPC 加载的全量条目
 *   - query: 搜索框值（受控）
 *   - selectedId: 当前选中条目 ID
 *
 * 键盘交互（由 search input 的 onKeyDown 处理）：
 *   - ↑ / ↓、⌃N/⌃J（下）与 ⌃P/⌃K（上）：在 visibleFlatList 中移动选中项（advanceSelection）
 *   - ⌘↑ / ⌘↓：跳到第一条 / 最后一条
 *   - 1–0：搜索框为空且无修饰键时数字直达，选中并粘贴第 1–10 条
 *   - ⌃W 删前一个词、⌃H 删前一个字符、⌃U 清空搜索框
 *   - Enter：pasteToFront(selectedId) 成功后 hide 窗口
 *   - Alt+Enter：copyClipToClipboard(selectedItem.id) 成功后 hide 窗口（富文本保真）
 *   - Esc：关闭窗口并把焦点还给上一个外部 app（hideAndReturnFocus）
 *   - 输入法组词期间：除 Esc 外一律不拦截，方向键与 Enter 归输入法选候选词
 */
export default function ClipPopoverApp() {
  const [items, setItems] = useState<ClipItem[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const inputRef = useRef<HTMLInputElement>(null);
  // ref 暴露最新的 visibleFlatList，避免 onFocusChanged 回调产生 stale closure
  const visibleFlatListRef = useRef<ClipItem[]>([]);
  // 数字直达的高亮反馈定时器；同一时刻只允许一个待取用
  const directPickTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // 文本编辑后待恢复的光标位置（受控 input 重渲染会把光标推到末尾）
  const pendingCaretRef = useRef<number | null>(null);

  // 受控 input 的 value 一旦变化，React 会把光标移到末尾；
  // ⌃W/⌃H/⌃U 编辑后必须把光标恢复到编辑位置，否则连续删词会跳回行尾。
  useLayoutEffect(() => {
    const caret = pendingCaretRef.current;
    if (caret === null) return;
    pendingCaretRef.current = null;
    inputRef.current?.setSelectionRange(caret, caret);
  }, [query]);

  /** 取消尚未触发的数字直达取用（Esc 关窗、组件卸载时调用，避免关窗后又粘一次） */
  function cancelPendingDirectPick(): void {
    if (directPickTimerRef.current === null) return;
    clearTimeout(directPickTimerRef.current);
    directPickTimerRef.current = null;
  }

  // 卸载时清掉反馈定时器，避免对已卸载组件执行粘贴
  useEffect(() => {
    return () => {
      cancelPendingDirectPick();
    };
  }, []);

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

  /** 执行取用并关窗：Enter 确认与数字直达共用同一条粘贴路径 */
  function confirmPasteById(id: string): void {
    pasteToFront(id)
      .then(() => getCurrentWindow().hide())
      .catch((err: unknown) => {
        console.error("[clip-popover] paste failed:", err);
      });
  }

  /**
   * 数字直达：搜索框为空且未按修饰键时，1–9 取第 1–9 条、0 取第 10 条。
   * 先高亮目标条目并保持 DIRECT_PICK_FLASH_MS 作为按键反馈，再执行取用；
   * 序号超出可见条数时只吞掉按键，不降级去取邻近条目。
   *
   * @returns 是否已按数字直达语义处理该按键
   */
  function handleDirectPickKey(e: React.KeyboardEvent<HTMLInputElement>): boolean {
    // 只有单个数字键才进入数字直达；Enter/方向键等仍由后续分支处理
    if (!DIGIT_KEY_PATTERN.test(e.key)) return false;
    if (!isDirectPickAllowed(query, e)) return false;
    // 数字直达模式下数字键归面板所有，不再作为搜索字符写入输入框
    e.preventDefault();

    const index = directPickIndex(e.key, visibleFlatList.length);
    const target = index === null ? null : visibleFlatList[index];
    if (target === null) return true;

    setSelectedId(target.id);
    cancelPendingDirectPick();
    directPickTimerRef.current = setTimeout(() => {
      directPickTimerRef.current = null;
      confirmPasteById(target.id);
    }, DIRECT_PICK_FLASH_MS);
    return true;
  }

  /** ⌘↑ 跳第一条、⌘↓ 跳最后一条 */
  function handleEdgeJumpKey(e: React.KeyboardEvent<HTMLInputElement>, flatIds: string[]): boolean {
    if (!e.metaKey) return false;
    let targetIndex: number;
    if (e.key === "ArrowUp") targetIndex = firstIndex(flatIds.length);
    else if (e.key === "ArrowDown") targetIndex = lastIndex(flatIds.length);
    else return false;
    e.preventDefault();
    // 空列表没有可跳转目标，但按键仍归面板（不落回输入框默认行为）
    if (targetIndex < 0) return true;
    setSelectedId(flatIds[targetIndex]);
    return true;
  }

  /**
   * 列表移动：↑↓ 与 vim 风格 ⌃N/⌃J 下移、⌃P/⌃K 上移（Maccy 同款键位）。
   * 带 ⌘ 的组合（⌘↑↓）由 handleEdgeJumpKey 接管，不在此处理。
   */
  function handleMoveKey(e: React.KeyboardEvent<HTMLInputElement>, flatIds: string[]): boolean {
    if (e.metaKey || e.altKey) return false;
    let direction: "ArrowUp" | "ArrowDown" | null = null;
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      direction = e.key;
    } else if (e.ctrlKey) {
      direction = ctrlMoveDirection(e.key.toLowerCase());
    }
    if (direction === null) return false;
    e.preventDefault();
    setSelectedId(advanceSelection(selectedId, direction, flatIds));
    return true;
  }

  /** 把文本编辑结果写回搜索框，并记下需要在重渲染后恢复的光标位置 */
  function applyQueryEdit(edit: TextEdit): void {
    if (edit.value === query) {
      // 值没变（例如在行首按 ⌃W）不会触发重渲染，直接同步光标即可
      inputRef.current?.setSelectionRange(edit.caret, edit.caret);
      return;
    }
    pendingCaretRef.current = edit.caret;
    setQuery(edit.value);
  }

  /**
   * 搜索框文本编辑：⌃W 删前一个词、⌃H 删前一个字符、⌃U 清空。
   * 只改搜索内容，不动列表选中项（选中项由列表自身的校正逻辑处理）。
   */
  function handleEditKey(e: React.KeyboardEvent<HTMLInputElement>): boolean {
    if (!e.ctrlKey) return false;
    const caret = inputRef.current?.selectionStart ?? query.length;
    let edit: TextEdit;
    switch (e.key.toLowerCase()) {
      case "w":
        edit = applyDeleteWord(query, caret);
        break;
      case "h":
        edit = applyDeleteChar(query, caret);
        break;
      case "u":
        edit = { value: "", caret: 0 };
        break;
      default:
        return false;
    }
    e.preventDefault();
    applyQueryEdit(edit);
    return true;
  }

  /** Enter 粘贴、⌥Enter 复制 */
  function handleConfirmKey(e: React.KeyboardEvent<HTMLInputElement>): boolean {
    if (e.key !== "Enter") return false;
    e.preventDefault();

    if (!e.altKey) {
      if (selectedId === null || selectedItem === null) return true;
      confirmPasteById(selectedId);
      return true;
    }

    if (selectedItem === null) return true;
    // 走后端 IPC 按 id 取内容写系统剪贴板：文本保真富文本格式，图片解码原图 PNG 后写 set_image。
    copyClipToClipboard(selectedItem.id)
      .then(() => getCurrentWindow().hide())
      .catch((err: unknown) => {
        console.error("[clip-popover] copy failed:", err);
      });
    return true;
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>): void {
    // Esc 必须排在输入法防御之前：关面板是用户的显式中断，不属于候选词交互。
    // WKWebView 会把 Esc 键原生"清空 search input"行为吞掉，不冒泡到 document。
    // 在 onKeyDown 处显式拦截并关窗，才能让 Esc 在输入框已获焦时正常工作。
    // 走 hideAndReturnFocus 而非裸 hide：关闭面板同时把焦点还给上一个外部 app（方案 C）。
    if (e.key === "Escape") {
      e.preventDefault();
      // 面板要关了，尚未触发的数字直达反馈不再取用
      cancelPendingDirectPick();
      hideAndReturnFocus().catch((err: unknown) => {
        console.error("[clip-popover] hideAndReturnFocus on Esc failed:", err);
      });
      return;
    }

    // 输入法组词期间（中文拼音等）方向键用于选候选词、Enter 用于确认候选词，
    // 一律不拦截：一旦 preventDefault 就会把候选词选择抢成列表导航。
    if (isImeComposing(e.nativeEvent)) return;

    const flatIds = visibleFlatList.map((i) => i.id);

    if (handleEdgeJumpKey(e, flatIds)) return;
    if (handleMoveKey(e, flatIds)) return;
    if (handleDirectPickKey(e)) return;
    if (handleEditKey(e)) return;
    handleConfirmKey(e);
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
