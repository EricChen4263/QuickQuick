import { useState, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import type { CSSProperties } from "react";
import type { ClipItem } from "../ipc/ipc-client";
import { listClipItems, pasteToFront, hideAndReturnFocus, copyClipToClipboard } from "../ipc/ipc-client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { CLIPBOARD_CHANGED_EVENT } from "../ipc/events";
import { groupClipItems } from "./grouping";
import { DEFAULT_SEARCH_MODE, prepareQuery, searchClipsWithPlan } from "./clip-search";
import { advanceSelection, createInputOwnership } from "./keyboard-nav";
import { useAppearancePreferences } from "../panels/settings/useAppearanceSettings";
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
import { useDeferredPreviewId } from "./useDeferredPreview";
import { resolvePreviewState } from "./preview-state";
import { applyModifierKey, NO_MODIFIERS, type ModifierState } from "./footer-hints";

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
 *   - previewSession: 面板会话序号，每次重新打开面板递增，用来重置预览延迟档位
 *   - modifiers: 当前按下的修饰键，仅驱动底部徽标切换
 *
 * 预览区（右侧）不直接跟随 selectedId：选中项经 useDeferredPreviewId 分级延迟后才显示
 * （首个预览默认 1500ms、可由设置项调整；此后切换固定 200ms），避免快速翻页时预览区逐条闪烁；
 * 延迟窗口内渲染中性占位而不是「无选中项」（三态判定见 preview-state.ts）。
 * 底部徽标随按下的修饰键切换分层提示（默认层 / ⌃ 层 / ⌘ 层 / ⌥ 层，见 footer-hints）。
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
  // 悬停与键盘的主导权仲裁：面板打开时由键盘主导，真实 pointer 位移才交给鼠标
  const [ownership] = useState(createInputOwnership);
  // 面板会话序号：每次窗口重新获得焦点（面板被打开）递增，预览延迟档位随之重置
  const [previewSession, setPreviewSession] = useState(0);
  // 外观设置刷新序号：后端在排序/高亮落盘后发 clipboard-changed（列表数据变了），
  // 届时重读一次外观设置，让新样式立刻生效而不是等下次重启。
  const [appearanceReload, setAppearanceReload] = useState(0);
  // 外观设置：面板常驻不重挂载，所以按会话序号重读——改完设置，下一次开面板即生效。
  // 首个预览延迟、图片高度与命中高亮都从这里来，读取失败时沿用默认值。
  const appearance = useAppearancePreferences(`${previewSession}:${appearanceReload}`);
  // 当前按下的修饰键：只驱动底部徽标切换，不参与任何取用/移动判定
  const [modifiers, setModifiers] = useState<ModifierState>(NO_MODIFIERS);

  const inputRef = useRef<HTMLInputElement>(null);
  // ref 暴露最新的 visibleFlatList，避免 onFocusChanged 回调产生 stale closure
  const visibleFlatListRef = useRef<ClipItem[]>([]);
  // 数字直达的高亮反馈定时器；同一时刻最多一个待取用，且任何非数字键输入都会取消它
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

  /**
   * 取消尚未触发的数字直达取用。
   * 触发时机：用户按了非数字键（已在做别的事）、Esc 关窗、组件卸载——
   * 否则 50ms 后会对一个用户早已离开的条目补一次粘贴/复制。
   */
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

  // 窗口失焦时把修饰键状态清空。
  // 用户在按住 ⌃ 期间切走（⌘Tab、点击别的 app），释放那一帧的 keyup 不会再打到本窗口，
  // 没有这条兜底就会一直停在「⌃ 层」徽标上，回到面板也不恢复。
  useEffect(() => {
    function resetModifiers(): void {
      setModifiers(NO_MODIFIERS);
    }
    window.addEventListener("blur", resetModifiers);
    return () => {
      window.removeEventListener("blur", resetModifiers);
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
  // 该事件同时承担「列表数据变了」的信号：排序或命中高亮落盘后后端也发它，
  // 所以这里顺带递增外观刷新序号，让新顺序/新样式立刻生效。
  useEffect(() => {
    const cancelled = { current: false };
    let unlisten: (() => void) | undefined;
    listen(CLIPBOARD_CHANGED_EVENT, () => {
      setAppearanceReload((epoch) => epoch + 1);
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
  // 同时递增面板会话序号：本次打开的首个预览要重新走首个预览档（默认 1500ms，可在设置中改），
  // 而不是沿用上次的 200ms
  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | null = null;

    win.onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        // 面板失焦：按住中的修饰键不会再收到 keyup，必须在离开时清掉
        setModifiers(NO_MODIFIERS);
        return;
      }
      inputRef.current?.focus();
      setQuery("");
      setPreviewSession((session) => session + 1);
      const firstId = visibleFlatListRef.current[0]?.id ?? null;
      // 面板重新打开后选中项由键盘接管：上一次留在指针下的行不得直接成为选中项
      ownership.handToKeyboard();
      setSelectedId(firstId);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  /**
   * popover 侧刻意不接防抖：面板里是几百行的纯内存过滤（无 IPC、无缩略图请求），
   * 延迟 200ms 只会让「敲字 → 看到结果」明显滞后，还会与数字直达的 50ms 反馈时序纠缠；
   * 需要防抖的是主窗——那边一次过滤会重置增量渲染窗口、重挂载上百行。
   */
  const queryPlan = useMemo(
    () => prepareQuery(query, DEFAULT_SEARCH_MODE),
    [query],
  );

  const hits = useMemo(
    () => searchClipsWithPlan(items, queryPlan),
    [items, queryPlan],
  );

  // 条目 id → 命中区间：列表行按 id 取用，不必每行再匹配一次
  const hitRangesById = useMemo(
    () => new Map(hits.map((hit) => [hit.item.id, hit.ranges])),
    [hits],
  );

  // 分组口径必须与排序口径一致，否则跨组全局顺序不成立：
  // 「首次复制」按 created_utc 划分今天/更早，「最近复制」仍按 last_modified_utc（默认行为不变）。
  const groups = useMemo(
    () =>
      groupClipItems(
        hits.map((hit) => hit.item),
        Date.now(),
        appearance.sortOrder === "first_copied" ? "firstCopied" : "lastCopied",
      ),
    [hits, appearance.sortOrder],
  );

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

  // 预览区跟随「延迟后的 ID」，而不是 selectedId：快速连按时中间项来不及到期就被下一项覆盖，
  // 因此预览不会逐条闪过去。
  const displayedId = useDeferredPreviewId(
    selectedItem?.id ?? null,
    previewSession,
    appearance.previewFirstDelayMs,
  );

  // 缩略图高度通过 CSS 自定义属性下发：一张图一行地用内联样式传会散成两处来源，
  // 而它是纯展示尺寸，整块列表共用同一个值。
  const shellStyle = useMemo<CSSProperties>(
    () =>
      ({
        "--popover-thumb-height": `${appearance.imageHeightPx}px`,
      }) as CSSProperties,
    [appearance.imageHeightPx],
  );

  // 按延迟后的 ID 从当前列表取对象而不是缓存条目本身：列表刷新（如收藏状态变化）后
  // 预览用的是新数据，不会停在打开面板那一刻的旧快照。
  const displayedItem = useMemo(
    () => visibleFlatList.find((i) => i.id === displayedId) ?? null,
    [visibleFlatList, displayedId]
  );

  // 「有选中项但预览还没到点」与「真的没有选中项」必须分开渲染：
  // 前者若说「无选中项」，会和左侧列表里明晃晃的高亮行自相矛盾。
  const preview = useMemo(
    () => resolvePreviewState(displayedItem, selectedItem !== null),
    [displayedItem, selectedItem]
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

    // 数字直达也是键盘在选，主导权归键盘：此后静止的鼠标不得把手动选中的目标改掉
    ownership.handToKeyboard();
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
    // 键盘跳首尾：主导权立即收回键盘，鼠标真实移动过之前悬停不得改选中
    ownership.handToKeyboard();
    setSelectedId(flatIds[targetIndex]);
    return true;
  }

  /**
   * 列表移动：↑↓ 与 vim 风格 ⌃N/⌃J 下移、⌃P/⌃K 上移（Maccy 同款键位）。
   * 带 ⌘ 的组合（⌘↑↓）由 handleEdgeJumpKey 接管，不在此处理；⌥ 与方向键组合沿用原有移动行为。
   */
  function handleMoveKey(e: React.KeyboardEvent<HTMLInputElement>, flatIds: string[]): boolean {
    if (e.metaKey) return false;
    let direction: "ArrowUp" | "ArrowDown" | null = null;
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      direction = e.key;
    } else if (e.ctrlKey) {
      direction = ctrlMoveDirection(e.key.toLowerCase());
    }
    if (direction === null) return false;
    e.preventDefault();
    // 键盘移动选中项：主导权立即收回键盘，鼠标真实移动过之前悬停不得改选中
    ownership.handToKeyboard();
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
    // 修饰键按下先同步给底部徽标：这一步只影响展示，不拦截任何按键。
    // 非修饰键不会改变状态（applyModifierKey 原样返回同一引用），因此普通输入不引发重渲染。
    setModifiers((prev) => applyModifierKey(prev, e.key, "down"));

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

    // 非数字键一律取消尚未触发的数字直达取用：用户这一帧已经在打字、导航或确认，
    // 50ms 后再补一次粘贴/复制必然是误操作（例如按 2 后又按 Enter 会粘两次）。
    // 数字键不能在这里取消，否则会把同一次按键刚 arm 的定时器自己清掉。
    // 位置在输入法防御之前：组词期间的按键同样意味着用户在打字，也要取消。
    if (!DIGIT_KEY_PATTERN.test(e.key)) {
      cancelPendingDirectPick();
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

  /** 修饰键释放：只更新底部徽标，松开后回到默认层提示 */
  function handleKeyUp(e: React.KeyboardEvent<HTMLInputElement>): void {
    setModifiers((prev) => applyModifierKey(prev, e.key, "up"));
  }

  /**
   * 搜索词变化：列表与选中项跟着重算，主导权回到键盘。
   * 打字同样会让列表滚动，静止的鼠标不该因为某一行被滚到指针下就抢走选中项。
   */
  function handleQueryChange(e: React.ChangeEvent<HTMLInputElement>): void {
    ownership.handToKeyboard();
    setQuery(e.target.value);
  }

  if (loadError) {
    return (
      <div className="popover-error body-5">
        加载失败：{loadError}
      </div>
    );
  }

  return (
    <div className="popover-shell" style={shellStyle} data-hit-style={appearance.highlightStyle}>
      <div className="popover-search-row">
        <svg
          className="ico ico-16 popover-search-icon"
          viewBox="0 0 24 24"
          aria-hidden="true"
        >
          <circle cx="11" cy="11" r="6.5" />
          <path d="m16.5 16.5 3.5 3.5" />
        </svg>
        <input
          ref={inputRef}
          type="text"
          role="searchbox"
          className="popover-search-input body-2"
          placeholder="搜索剪贴板…"
          value={query}
          onChange={handleQueryChange}
          onKeyDown={handleKeyDown}
          onKeyUp={handleKeyUp}
          autoFocus
          aria-label="搜索剪贴板"
        />
        {/* 唤起本面板的全局热键（后端 HotkeyConfig 默认 CmdOrCtrl+Shift+C）。
            面板已经打开，这枚键帽是「下次怎么再叫它出来」的备忘，不参与交互。 */}
        <kbd className="qq-kbd popover-search-kbd" title="唤起快捷键">
          ⌘⇧C
        </kbd>
        {queryPlan.notice !== null && (
          // 模式降级这类「用户需要知道」的状态交给读屏播报，不静默吞掉
          <div className="popover-search-notice caption-5" role="status">
            {queryPlan.notice}
          </div>
        )}
      </div>
      <div className="popover-body">
        <PopoverList
          groups={groups}
          selectedId={selectedId}
          hitRanges={hitRangesById}
          onSelect={(item) => setSelectedId(item.id)}
          ownership={ownership}
        />
        <PopoverPreview state={preview} />
      </div>
      <PopoverFooter modifiers={modifiers} />
    </div>
  );
}
