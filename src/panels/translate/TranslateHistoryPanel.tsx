import { useState } from "react";
import type { TranslateHistoryItem } from "../../ipc/ipc-client";
import EmptyState from "../../components/EmptyState";

interface TranslateHistoryPanelProps {
  items: TranslateHistoryItem[];
  onSelectItem: (item: TranslateHistoryItem) => void;
}

/**
 * 翻译历史右栏：按时间倒序渲染历史条目列表。
 * 点击某条目触发 onSelectItem，由父组件实现回填逻辑。
 * 内部维护 selectedId，驱动 aria-selected 动态更新，修复 ARIA listbox/option 结构。
 */
function TranslateHistoryPanel({ items, onSelectItem }: TranslateHistoryPanelProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);

  function handleSelect(item: TranslateHistoryItem) {
    setSelectedId(item.id);
    onSelectItem(item);
  }

  return (
    <aside className="tx-history" aria-label="翻译历史">
      <div className="pane-head">
        <span className="pane-title">历史</span>
        <span className="count-pill">{items.length}</span>
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
