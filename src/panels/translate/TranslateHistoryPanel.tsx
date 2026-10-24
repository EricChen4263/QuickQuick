import { useState } from "react";
import type { TranslateHistoryItem } from "../../ipc/ipc-client";
import EmptyState from "../../components/EmptyState";

interface TranslateHistoryPanelProps {
  items: TranslateHistoryItem[];
  onSelectItem: (item: TranslateHistoryItem) => void;
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

/**
 * 翻译历史右栏：按时间倒序渲染历史条目列表。
 * 点击某条目触发 onSelectItem，由父组件实现回填逻辑。
 * 内部维护 selectedId，驱动 aria-selected 动态更新，修复 ARIA listbox/option 结构。
 *
 * 取数语义是「最近一页」而非全量：后端最多返回 TRANSLATE_HISTORY_PAGE_LIMIT 条，
 * 因此条数达到上限时补一行提示，避免用户把「200」误读成历史总数。
 */
function TranslateHistoryPanel({ items, onSelectItem }: TranslateHistoryPanelProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);

  function handleSelect(item: TranslateHistoryItem) {
    setSelectedId(item.id);
    onSelectItem(item);
  }

  const isPageFull = items.length >= TRANSLATE_HISTORY_PAGE_LIMIT;

  return (
    <aside className="tx-history" aria-label="翻译历史">
      <div className="tx-hist-head">
        <div className="pane-head">
          <span className="pane-title">历史</span>
          <span className="count-pill">{items.length}</span>
        </div>
        {isPageFull && (
          <p className="tx-hist-limit" data-testid="history-limit-hint">
            仅显示最近 {TRANSLATE_HISTORY_PAGE_LIMIT} 条
          </p>
        )}
      </div>

      <div
        className="tx-hist-list"
        role="listbox"
        aria-label="翻译历史列表"
      >
        {items.length === 0 ? (
          <EmptyState
            icon={
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.7"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <path d="m5 8 6 6" />
                <path d="m4 14 6-6 2-3" />
                <path d="M2 5h12" />
                <path d="M7 2h1" />
                <path d="m22 22-5-10-5 10" />
                <path d="M14 18h6" />
              </svg>
            }
            title="暂无翻译历史"
            description="翻译后的记录会出现在这里"
          />
        ) : (
          items.map((item) => (
            <button
              key={item.id}
              className="hist-row"
              data-testid={`history-item-${item.id}`}
              role="option"
              aria-selected={item.id === selectedId}
              onClick={() => handleSelect(item)}
            >
              <div className="hist-src">{item.sourceText}</div>
              <div className="hist-dst">{item.translatedText}</div>
              <div className="hist-tag">
                <span className="lp">{item.sourceLang} → {item.targetLang}</span>
                <span>{item.providerId}</span>
              </div>
            </button>
          ))
        )}
      </div>
    </aside>
  );
}

export default TranslateHistoryPanel;
