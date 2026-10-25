/**
 * 剪贴板历史页（双栏布局：左侧列表栏 + 右侧预览区）。
 * 挂载时通过 IPC 取数，支持搜索过滤、类型筛选、键盘流、收藏/删除管理。
 * 对应验收项 V1-F2-A07。
 *
 * 列表按增量渲染：首屏只渲染首块，滚动接近底部再追加，选中项越出窗口时扩窗。
 * 搜索与类型筛选仍在内存里对全量匹配，窗口只治理 DOM 行数，不裁剪可达范围。
 */

import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { CLIPBOARD_CHANGED_EVENT } from "../../ipc/events";
import {
  listClipItems,
  deleteClipItem,
  toggleFavoriteClip,
  pasteToFront,
  copyClipToClipboard,
  openAccessibilitySettings,
  getHotkeys,
  getLaunchOnLogin,
  getOnboardingStatus,
  setLaunchOnLogin,
  type ClipItem,
} from "../../ipc/ipc-client";
import type { HistoryItem } from "../history/search";
import type { HistoryFilter } from "../history/filter";
import { filterByType } from "../history/filter";
import {
  DEFAULT_SEARCH_MODE,
  clipSearchText,
  prepareQuery,
  searchClipsWithPlan,
} from "../../clip-popover/clip-search";
import { SEARCH_DEBOUNCE_MS, useDebouncedValue } from "./useDebouncedValue";
import {
  moveHighlight,
  quickSelectIndex,
  resolveEnter,
  isImeComposing,
  firstIndex,
  lastIndex,
} from "../history/keyboard";
import {
  RENDER_BLOCK_SIZE,
  initialRenderCount,
  growRenderCount,
  nextRenderCount,
  shouldResetRenderCount,
  isNearBottom,
} from "./render-window";
import { ClipItemRow } from "./ClipItemRow";
import { ClipPreview } from "./ClipPreview";
import { ClipSearchBar } from "./ClipSearchBar";
import {
  OnboardCard,
  advanceGuide,
  createGuideState,
  finishGuide,
  formatAccelerator,
  isGuideVisible,
  observeAccessibilityTrust,
  skipGuideStep,
  type GuideState,
} from "./OnboardCard";
import EmptyState from "../../components/EmptyState";
import { useAppearancePreferences } from "../settings/useAppearanceSettings";
import "./clipboard.css";

/** localStorage key：首次引导已结束（走完或关闭），此后不再出现 */
const ONBOARD_FINISHED_KEY = "qq-onboard-finished";

/**
 * 旧版单卡片引导的关闭标记。
 *
 * 命中即视为「已完成」：已经关过旧引导的用户不该被新版向导再问一遍。
 */
const ONBOARD_LEGACY_DISMISSED_KEY = "qq-onboard-dismissed";

/** 读取引导是否已结束（含旧版关闭标记的兼容）。 */
function readGuideFinished(): boolean {
  return (
    localStorage.getItem(ONBOARD_FINISHED_KEY) === "true" ||
    localStorage.getItem(ONBOARD_LEGACY_DISMISSED_KEY) === "true"
  );
}

/** 将 IPC ClipItem 适配为纯逻辑 HistoryItem（文本口径与搜索/高亮一致） */
function toHistoryItem(clip: ClipItem): HistoryItem {
  if (clip.kind === "image") {
    return { id: clip.id, text: clipSearchText(clip), kind: "image" };
  }
  return {
    id: clip.id,
    text: clipSearchText(clip),
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
  // 外观设置刷新序号：clipboard-changed 到达时递增，命中高亮样式随之重读
  const [appearanceReload, setAppearanceReload] = useState(0);
  // 命中高亮样式：主窗列表与弹窗共用同一设置，读取失败时用默认档（背景 + 加粗）
  const appearance = useAppearancePreferences(appearanceReload);
  const [items, setItems] = useState<ClipItem[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [typeFilter, setTypeFilter] = useState<HistoryFilter>("all");
  const [highlightIndex, setHighlightIndex] = useState(0);
  // 首次引导向导：状态机在当前步骤，guideFinished 是持久化后的终态开关
  const [guide, setGuide] = useState<GuideState>(() => createGuideState(false));
  const [guideFinished, setGuideFinished] = useState(readGuideFinished);
  // 自启偏好：后端 AutostartConfig::default() 就是「开」，读不到时保持这个默认
  const [launchOnLogin, setLaunchOnLoginPreference] = useState(true);
  // 历史面板热键的展示文本，读不到配置时用默认热键
  const [hotkeyLabel, setHotkeyLabel] = useState(() => formatAccelerator(""));
  const [pasteMsg, setPasteMsg] = useState<string | null>(null);
  // 列表渲染窗口大小（不是「已加载条数」）：只决定 DOM 里渲染多少行
  const [renderCount, setRenderCount] = useState(RENDER_BLOCK_SIZE);
  const listRef = useRef<HTMLDivElement>(null);
  // 已同步过滚动的选中索引，用于避免「追加渲染」引起的被动滚动回到高亮行
  const scrolledHighlightRef = useRef(-1);

  // 组件生命周期级 cancelled ref，供 handleToggleFavorite / handleDelete 共享。
  // 卸载时 cleanup 置 true，loadItems 内的 guard 据此跳过已卸载组件的 setState。
  // 注意：setup 里复位为 false，处理 StrictMode 卸载-重挂后 ref 残留 true 的问题。
  const cancelledRef = useRef(false);
  useEffect(() => {
    cancelledRef.current = false;
    return () => {
      cancelledRef.current = true;
    };
  }, []);

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

  // 订阅后端 clipboard-changed 事件，有新条目或条目置顶时重新拉取列表。
  // 采用与 App.tsx route 监听相同的 cancelled+unlisten 范式，防止卸载后泄漏。
  // 该事件同时承担「列表数据变了」的信号：排序或命中高亮落盘后后端也发它，
  // 所以这里顺带递增外观刷新序号，让新样式立刻生效而不是等重启。
  useEffect(() => {
    const cancelled = { current: false };
    let unlisten: (() => void) | undefined;
    listen(CLIPBOARD_CHANGED_EVENT, () => {
      setAppearanceReload((epoch) => epoch + 1);
      loadItems(cancelled);
    })
      .then((fn) => {
        if (cancelled.current) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err: unknown) => {
        console.error("[QuickQuick] clipboard-changed 监听注册失败:", err);
      });
    return () => {
      cancelled.current = true;
      unlisten?.();
    };
  }, [loadItems]);

  /**
   * 辅助功能授权观测：挂载时读一次，窗口重新获得焦点时再读一次。
   *
   * 后者是「授权后自动恢复」的关键：用户从系统设置点完「允许」回到面板，
   * 焦点事件触发的这次读取会把授权翻成已授权，向导随即收起授权步骤。
   * 读取失败按未授权处理：向导照常请求授权，粘贴也仍会走降级分支，不误导用户。
   */
  useEffect(() => {
    const cancelled = { current: false };
    const refreshTrust = async (): Promise<void> => {
      try {
        const status = await getOnboardingStatus();
        if (cancelled.current) return;
        setGuide((prev) => observeAccessibilityTrust(prev, status.action === "proceed"));
      } catch {
        /* 读不到授权状态时保持当前观测值，不把「未知」当成「已授权」 */
      }
    };
    void refreshTrust();
    const onWindowFocus = (): void => {
      void refreshTrust();
    };
    window.addEventListener("focus", onWindowFocus);
    return () => {
      cancelled.current = true;
      window.removeEventListener("focus", onWindowFocus);
    };
  }, []);

  /**
   * 自启确认步骤的数据：读当前 autostart 偏好与历史面板热键。
   *
   * 两个都只在向导里用，读失败不阻断面板：自启保持后端默认的「开」，
   * 热键退回默认 ⌘⇧C 展示（真实绑定仍由后端热键注册决定，失败不改变它）。
   */
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const enabled = await getLaunchOnLogin();
        if (!cancelled) setLaunchOnLoginPreference(enabled);
      } catch {
        /* 保持默认开，与后端 AutostartConfig::default() 一致 */
      }
      try {
        const hotkeys = await getHotkeys();
        if (!cancelled) setHotkeyLabel(formatAccelerator(hotkeys.history));
      } catch {
        /* 保持默认热键展示 */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // 向导走到终态（完成 / 关闭）即落盘：此后不再出现，重开面板也不会复活。
  useEffect(() => {
    if (isGuideVisible(guide)) return;
    localStorage.setItem(ONBOARD_FINISHED_KEY, "true");
    setGuideFinished(true);
  }, [guide]);

  // 搜索框值即时更新，过滤用值防抖提交：200ms 内的连续击键不触发重新搜索。
  // 输入框仍是受控的即时值，打字与输入法组词反馈不受延迟影响。
  const debouncedQuery = useDebouncedValue(searchQuery, SEARCH_DEBOUNCE_MS);

  // 查询计划随防抖后的词更新：分词与正则编译一次，逐条目复用
  const queryPlan = useMemo(
    () => prepareQuery(debouncedQuery, DEFAULT_SEARCH_MODE),
    [debouncedQuery],
  );

  // 两步过滤：先按搜索模块筛出命中条目并拿到高亮区间，再用类型筛选收窄
  const hits = useMemo(() => searchClipsWithPlan(items, queryPlan), [items, queryPlan]);
  const historyItems = useMemo(() => hits.map((hit) => toHistoryItem(hit.item)), [hits]);
  const filteredItems = useMemo(
    () => filterByType(historyItems, typeFilter),
    [historyItems, typeFilter],
  );

  // id → 命中区间：渲染层按 id 取用，避免每行再匹配一次；类型筛选不改变命中区间
  const hitRangesById = useMemo(
    () => new Map(hits.map((hit) => [hit.item.id, hit.ranges])),
    [hits],
  );

  // id → 条目索引：列表渲染与取用都按 id 反查，避免每行对 items 做一次线性查找
  const clipById = useMemo(() => new Map(items.map((c) => [c.id, c])), [items]);

  // 高亮索引 clamp 到有效范围
  const safeHighlight =
    filteredItems.length === 0
      ? -1
      : Math.min(Math.max(highlightIndex, 0), filteredItems.length - 1);

  const highlightedClipItem =
    safeHighlight >= 0
      ? clipById.get(filteredItems[safeHighlight].id) ?? null
      : null;

  // 实际渲染行数：窗口大小收敛到过滤结果总数
  const visibleCount = Math.min(renderCount, filteredItems.length);

  // 过滤条件键：任一项变化都意味着结果集换了，旧窗口必须作废。
  // 键里放的是「实际生效的源条件」（类型筛选 + 防抖后的搜索词），不是派生量（命中条数）：
  // 两个不同搜索词的命中条数可能完全相同，那时结果集内容已经变了，窗口照样必须重置。
  const filterKey = `${typeFilter}\u0000${debouncedQuery}`;
  const prevFilterKeyRef = useRef(filterKey);

  // 过滤条件变化 → 窗口重置回首屏块。
  // 不重置的话「搜完再清空」会留下一个巨大的窗口，等于没治理 DOM 行数。
  useEffect(() => {
    const isFilterChanged = shouldResetRenderCount(prevFilterKeyRef.current, filterKey);
    if (!isFilterChanged) return;
    prevFilterKeyRef.current = filterKey;
    setRenderCount(initialRenderCount(filteredItems.length));
  }, [filterKey, filteredItems.length]);

  // 选中项越出窗口 → 把窗口扩到能包含它。
  // 键盘导航遍历的是完整过滤结果，⌘↓ 跳末条、连按 ↓、删除后索引回落都可能一步跳出
  // 已渲染范围；不扩窗的话高亮行不在 DOM 里，表现为「高亮消失且滚不过去」。
  useEffect(() => {
    setRenderCount((prev) => nextRenderCount(prev, safeHighlight, filteredItems.length));
  }, [safeHighlight, filteredItems.length]);

  // 列表被删空（filteredItems 由非空变空）→ 清空选中并把窗口收回首屏块。
  //
  // 为什么只在「非空变空」时收：93ef41b 之后窗口不会因数据为空被压成 0（那会让数据到达的
  // 第一个提交渲染 0 行），但反过来也有坑——窗口本来就可能被滚到底扩到几百条，
  // 若删空后不清，重新有数据时会**一次性渲染几百行**，增量渲染等于白做。
  // 用 ref 记录上一次的条数：挂载时 prev 也是 0，所以挂载不会误触发。
  //
  // 与扩窗 effect 的先后不影响结果：扩窗那条在数据为空时返回 prev 原样，
  // 收到这里写入的 100 就是 100，不会覆盖回去。
  // 真正承担行为的是上面那句 `setHighlightIndex(0)`——少了它，回填后 highlightIndex
  // 还停在旧位置（例如删空前 ⌘↓ 停在 299），扩窗 effect 会为包住它把窗口一次扩回几百行。
  // 用例「删空后清空选中并把窗口收回首屏块」同时锁住这两点。
  const prevFilteredCountRef = useRef(filteredItems.length);
  useEffect(() => {
    const prevCount = prevFilteredCountRef.current;
    prevFilteredCountRef.current = filteredItems.length;
    if (prevCount > 0 && filteredItems.length === 0) {
      // 列表空了就没有「选中项」可言：不清的话，回填后 highlightIndex 还停在旧位置
      // （例如删空前 ⌘↓ 停在 299），窗口会为了包住它一次扩回几百行——增量渲染等于白做。
      setHighlightIndex(0);
      setRenderCount(initialRenderCount(0));
    }
  }, [filteredItems.length]);

  // 选中项变化后把高亮行滚入可视区（block:nearest 只在越界时滚动，不打断已可见项）。
  // 扩窗是异步的：选中项那一行还没进 DOM 时先记下未同步状态，等扩窗引发的
  // renderCount 变化再补一次滚动。用 ref 记录已同步索引，避免追加渲染时把列表拽回高亮行。
  useEffect(() => {
    if (safeHighlight < 0 || scrolledHighlightRef.current === safeHighlight) return;
    const selectedRow = listRef.current?.querySelector('[aria-selected="true"]') ?? null;
    if (selectedRow === null) return;
    selectedRow.scrollIntoView({ block: "nearest" });
    scrolledHighlightRef.current = safeHighlight;
  }, [safeHighlight, renderCount]);

  /**
   * 滚动接近底部时追加一块渲染窗口。
   * 判定放在纯函数里，组件只负责取容器的几何信息，便于脱离 DOM 单测。
   */
  function handleListScroll(event: React.UIEvent<HTMLDivElement>): void {
    const el = event.currentTarget;
    if (!isNearBottom(el.scrollTop, el.scrollHeight, el.clientHeight)) return;
    setRenderCount((prev) => growRenderCount(prev, filteredItems.length));
  }

  /**
   * 按条目 id 执行「粘贴到前台」。
   * 键盘（Enter / ⌘1-9）与鼠标点击共用这一条取用路径，
   * 降级提示、错误提示、IPC 调用都只在 handlePasteToFront 里发生一次。
   */
  function pasteItemById(id: string | null): void {
    if (id === null) return;
    const target = clipById.get(id);
    if (!target) return;
    void handlePasteToFront(target);
  }

  /**
   * 按条目 id 只复制到系统剪贴板（⌥Enter 语义，与 popover 的 ⌥Enter 一致）。
   * 走 copyClipToClipboard 而非浏览器剪贴板 API：由后端按 id 取 text+html，保持富文本保真。
   */
  function copyItemById(id: string | null): void {
    if (id === null) return;
    copyClipToClipboard(id).catch(() => {
      setOpError("复制失败，请稍后重试");
    });
  }

  function handleKeyDown(event: React.KeyboardEvent) {
    // 输入法组词期间（中文拼音选候选词）所有方向键与直达键都归输入法，
    // 这里不拦截任何键，否则会把候选词选择抢成列表导航。
    if (isImeComposing(event.nativeEvent)) return;

    // ⌘↑ / ⌘↓ 跳首条 / 末条（与 popover 同键位）。
    // 目标索引可能远在已渲染窗口之外，扩窗由选中项同步 effect 负责。
    if (event.metaKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      const target =
        event.key === "ArrowUp" ? firstIndex(filteredItems.length) : lastIndex(filteredItems.length);
      if (target < 0) return;
      setHighlightIndex(target);
      return;
    }

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      setHighlightIndex((prev) =>
        moveHighlight(prev, event.key as "ArrowUp" | "ArrowDown", filteredItems.length),
      );
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const targetId = resolveEnter(safeHighlight, filteredItems)?.id ?? null;
      if (event.altKey) {
        // ⌥Enter 与 popover 同语义：只复制，不模拟粘贴
        copyItemById(targetId);
        return;
      }
      // Enter 直接取用高亮条目；没有有效高亮（空列表 / 越界）时不做任何事
      pasteItemById(targetId);
      return;
    }
    if (event.metaKey || event.ctrlKey) {
      const idx = quickSelectIndex(event.key);
      if (idx === null) return;
      event.preventDefault();
      // ⌘1-9 是快速粘贴：序号落在列表内就直接取用，不存在的序号不降级取邻近条目
      pasteItemById(filteredItems[idx]?.id ?? null);
    }
  }

  async function handleToggleFavorite(item: ClipItem): Promise<void> {
    try {
      await toggleFavoriteClip(item.id, !item.isFavorite);
      await loadItems(cancelledRef);
    } catch {
      setOpError("操作失败，请稍后重试");
    }
  }

  async function handleDelete(item: ClipItem): Promise<void> {
    try {
      await deleteClipItem(item.id);
      await loadItems(cancelledRef);
    } catch {
      setOpError("操作失败，请稍后重试");
    }
  }

  /** 向导「下一步 / 完成」：推进状态机；落到终态时由持久化 effect 落盘。 */
  function handleGuideAdvance(): void {
    setGuide((prev) => advanceGuide(prev));
  }

  /** 向导「跳过 / 稍后」：跳过当前步骤（欢迎页跳过是空操作，由状态机守卫）。 */
  function handleGuideSkip(): void {
    setGuide((prev) => skipGuideStep(prev));
  }

  /** 关闭 ×：结束整个向导，此后不再出现。 */
  function handleGuideDismiss(): void {
    setGuide((prev) => finishGuide(prev));
  }

  /**
   * 自启确认步骤切换：先更新界面，**写盘失败必须回滚**并显示后端原文。
   *
   * 只提示不回滚等于「界面撒谎」：开关停在用户点的位置，实际偏好没变，
   * 下次重启又变回去，用户无从判断到底生效了没有。与设置页通用面板的处理保持一致。
   */
  function handleLaunchOnLoginChange(enabled: boolean): void {
    const previous = launchOnLogin;
    setLaunchOnLoginPreference(enabled);
    void setLaunchOnLogin(enabled).catch((err: unknown) => {
      setLaunchOnLoginPreference(previous);
      setOpError(err instanceof Error ? err.message : String(err));
    });
  }

  async function handlePasteToFront(item: ClipItem): Promise<void> {
    setPasteMsg(null);
    try {
      const result = await pasteToFront(item.id);
      if (result.outcome === "write_back_only") {
        setPasteMsg(
          "已复制到剪贴板，请手动粘贴。未获辅助功能授权；若已授权（如刚更新过版本），请重新授权后重试。",
        );
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
    <div className="clip-page" data-hit-style={appearance.highlightStyle}>
      {opError !== null && (
        <div
          role="alert"
          style={{ gridColumn: "1 / -1", padding: "8px 12px", color: "var(--danger)", background: "var(--surface)" }}
        >
          {opError}
        </div>
      )}
      {pasteMsg !== null && (
        <div
          role="status"
          style={{ gridColumn: "1 / -1", display: "flex", alignItems: "center", gap: 12, padding: "8px 12px", fontSize: 12, color: "var(--muted)", background: "var(--surface)" }}
        >
          <span>{pasteMsg}</span>
          <button
            type="button"
            onClick={() => { void handleOpenSystemSettings(); }}
            style={{ flexShrink: 0, padding: "4px 10px", fontSize: 12, color: "var(--fg)", background: "var(--surface-2)", border: "1px solid var(--border)", borderRadius: 6, cursor: "pointer" }}
          >
            打开辅助功能设置
          </button>
        </div>
      )}
      <div className="clip-list-col">
        <ClipSearchBar
          searchQuery={searchQuery}
          typeFilter={typeFilter}
          notice={queryPlan.notice}
          onSearchChange={(q) => { setSearchQuery(q); setHighlightIndex(0); }}
          onTypeFilterChange={(f) => { setTypeFilter(f); setHighlightIndex(0); }}
          onKeyDown={handleKeyDown}
        />
        {!guideFinished && isGuideVisible(guide) && (
          <OnboardCard
            state={guide}
            launchOnLogin={launchOnLogin}
            hotkeyLabel={hotkeyLabel}
            onAdvance={handleGuideAdvance}
            onSkip={handleGuideSkip}
            onDismiss={handleGuideDismiss}
            onOpenSystemSettings={() => { void handleOpenSystemSettings(); }}
            onLaunchOnLoginChange={handleLaunchOnLoginChange}
          />
        )}
        <div
          ref={listRef}
          className="clip-list"
          role="listbox"
          aria-label="剪贴板历史"
          onScroll={handleListScroll}
        >
          {filteredItems.length === 0 ? (
            <EmptyState
              icon={EmptyListIcon}
              title="暂无记录"
              description="复制任意内容后将显示在这里"
            />
          ) : (
            filteredItems.slice(0, visibleCount).map((histItem, idx) => {
              const clipItem = clipById.get(histItem.id);
              if (!clipItem) return null;
              return (
                <ClipItemRow
                  key={histItem.id}
                  item={clipItem}
                  isHighlighted={idx === safeHighlight}
                  hitRanges={hitRangesById.get(histItem.id)}
                  onToggleFavorite={handleToggleFavorite}
                  onSelect={() => { setHighlightIndex(idx); }}
                />
              );
            })
          )}
        </div>
      </div>
      <ClipPreview
        item={highlightedClipItem}
        onToggleFavorite={handleToggleFavorite}
        onDelete={handleDelete}
        onCopy={(_item) => { /* 复制逻辑在 ClipPreview 内部调用 copyClipToClipboard(id) 完成，带富文本保真 */ }}
        onPasteToFront={(item) => { void handlePasteToFront(item); }}
        onTranslate={(item) => { onTranslateItem?.(item.content); }}
      />
    </div>
  );
}

export default ClipboardPage;
