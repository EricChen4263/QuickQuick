import { useState } from "react";
import type { TranslateHistoryItem } from "../../ipc/ipc-client";
import EmptyState from "../../components/EmptyState";

interface TranslateHistoryPanelProps {
  items: TranslateHistoryItem[];
  onSelectItem: (item: TranslateHistoryItem) => void;
  /**
   * providerId → 展示名。历史条目只带 id，直接渲染会出现「deepl-free」这类内部标识。
   * 可选：不传时回退到 id 原文（浮窗/测试等拿不到 provider 列表的场合仍然可用）。
   */
  providerNames?: Record<string, string>;
}

/**
 * 后端单次返回的翻译历史条数上限。
 *
 * 与 `src-tauri/src/translate/history.rs` 的 `TRANSLATE_HISTORY_PAGE_LIMIT` 必须一致：
 * `list_translate_history` 返回的是裸数组、不带分页元信息，Tauri 命令的契约无法在
 * 编译期共享常量（同 `TRANSLATE_HISTORY_CHANGED_EVENT` 的既有约定），改动需两端同步。
 * 这里只用它判断「拿到的已经是一页的边界值」，从而决定是否提示「仅显示最近 N 条」。
 */
export const TRANSLATE_HISTORY_PAGE_LIMIT = 200;

/** 历史空态的插画：翻译字形（Phosphor 语汇，24 网格） */
function HistoryEmptyIcon() {
  return (
    <svg className="ico" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4 6h9" />
      <path d="M8.5 4v2" />
      <path d="M10.5 6c-.4 3.5-3.1 6.4-6.5 7.7" />
      <path d="M5.2 10.2c1.2 2.6 3.3 4.3 5.6 5.2" />
      <path d="m13.6 19.5 3.7-9 3.7 9" />
      <path d="M15 16.5h4.6" />
    </svg>
  );
}

/**
 * 翻译历史右栏：按时间倒序渲染历史条目列表。
 * 点击某条目触发 onSelectItem，由父组件实现回填逻辑。
 * 内部维护 selectedId，驱动 aria-selected 动态更新，修复 ARIA listbox/option 结构。
 *
 * 视觉上这条侧栏刻意保持透明（页面底色 --bg）：本页只允许「工作区」是抬升面，
 * 行在 hover / 选中时才抬到 --surface，避免出现两块并排的白卡。
 *
 * 取数语义是「最近一页」而非全量：后端最多返回 TRANSLATE_HISTORY_PAGE_LIMIT 条，
 * 因此条数达到上限时补一行提示，避免用户把「200」误读成历史总数。
 */
function TranslateHistoryPanel({ items, onSelectItem, providerNames }: TranslateHistoryPanelProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);

  function handleSelect(item: TranslateHistoryItem) {
    setSelectedId(item.id);
    onSelectItem(item);
  }

  const isPageFull = items.length >= TRANSLATE_HISTORY_PAGE_LIMIT;

  return (
    <aside className="tx-history" aria-label="翻译历史">
      <div className="tx-hist-head">
        <span className="title-4">历史</span>
        <span className="count-pill caption-4 num">{items.length}</span>
      </div>

      {isPageFull && (
        <p className="tx-hist-limit caption-5" data-testid="history-limit-hint">
          仅显示最近 {TRANSLATE_HISTORY_PAGE_LIMIT} 条
        </p>
      )}

      <div className="tx-hist-list" role="listbox" aria-label="翻译历史列表">
        {items.length === 0 ? (
          <EmptyState
            icon={<HistoryEmptyIcon />}
            title="暂无翻译历史"
            description="翻译过的句子会自动存到这里，点一条就能把它送回工作区。"
          />
        ) : (
          items.map((item) => (
            <button
              key={item.id}
              type="button"
              className="hist-row"
              data-testid={`history-item-${item.id}`}
              role="option"
              aria-selected={item.id === selectedId}
              onClick={() => handleSelect(item)}
            >
              <span className="hist-src body-5">{item.sourceText}</span>
              <span className="hist-dst caption-5">{item.translatedText}</span>
              <span className="hist-tag caption-5">
                <span className="lp badge caption-4 mono">
                  {item.sourceLang} → {item.targetLang}
                </span>
                <span className="hist-prov">{providerNames?.[item.providerId] ?? item.providerId}</span>
              </span>
            </button>
          ))
        )}
      </div>
    </aside>
  );
}

export default TranslateHistoryPanel;
