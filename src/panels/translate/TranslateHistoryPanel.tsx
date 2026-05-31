import type { TranslateHistoryItem } from "../../ipc/ipc-client";

/** 空历史占位文案（具名常量） */
const EMPTY_HISTORY_PLACEHOLDER = "暂无翻译历史";

interface TranslateHistoryPanelProps {
  items: TranslateHistoryItem[];
  onSelectItem: (item: TranslateHistoryItem) => void;
}

/**
 * 翻译历史右栏：按时间倒序渲染历史条目列表。
 * 点击某条目触发 onSelectItem，由父组件实现回填逻辑。
 */
function TranslateHistoryPanel({ items, onSelectItem }: TranslateHistoryPanelProps) {
  if (items.length === 0) {
    return (
      <aside
        aria-label="翻译历史"
        style={{ width: "240px", borderLeft: "1px solid var(--qq-border, #e0e0e0)", padding: "12px", overflowY: "auto" }}
      >
        <p style={{ color: "var(--qq-text-muted)", fontSize: "13px" }}>
          {EMPTY_HISTORY_PLACEHOLDER}
        </p>
      </aside>
    );
  }

  return (
    <aside
      aria-label="翻译历史"
      style={{ width: "240px", borderLeft: "1px solid var(--qq-border, #e0e0e0)", padding: "12px", overflowY: "auto" }}
    >
      <ul style={{ listStyle: "none", margin: 0, padding: 0, display: "flex", flexDirection: "column", gap: "8px" }}>
        {items.map((item) => (
          <li
            key={item.id}
            data-testid={`history-item-${item.id}`}
            onClick={() => onSelectItem(item)}
            style={{ cursor: "pointer", padding: "8px", borderRadius: "4px", background: "var(--qq-surface, #f5f5f5)" }}
          >
            <p style={{ margin: "0 0 4px", fontSize: "13px", fontWeight: 500 }}>{item.sourceText}</p>
            <p style={{ margin: 0, fontSize: "12px", color: "var(--qq-text-muted, #666)" }}>{item.translatedText}</p>
          </li>
        ))}
      </ul>
    </aside>
  );
}

export default TranslateHistoryPanel;
