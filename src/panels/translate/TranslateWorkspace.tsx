import type { TranslateResult } from "../../ipc/ipc-client";
import { availableActions, resolveTranslateAction } from "../../translate/translate-actions";

/** 译文操作按钮的中文标签（具名常量，避免魔术字符串） */
const ACTION_LABELS: Record<string, string> = {
  copy: "复制",
  speak: "朗读",
  switch_target: "切换目标语",
  switch_source_retranslate: "换源重译",
  // save_history 已自动写入，展示"刷新历史"语义由父组件处理，此处禁用
  save_history: "刷新历史",
};

interface TranslateWorkspaceProps {
  inputText: string;
  result: TranslateResult | null;
  isLoading: boolean;
  onInputChange: (text: string) => void;
  onTranslate: () => void;
  onAction: (action: string) => void;
}

/**
 * 翻译工作区：输入框 + 翻译按钮 + 原文/译文上下对照 + 操作按钮条。
 * 函数组件，无副作用，纯展示+事件上抛。
 */
function TranslateWorkspace({
  inputText,
  result,
  isLoading,
  onInputChange,
  onTranslate,
  onAction,
}: TranslateWorkspaceProps) {
  const actions = availableActions();
  const isTranslateDisabled = inputText.trim().length === 0 || isLoading;

  return (
    <div style={{ display: "flex", flexDirection: "column", flex: 1, padding: "16px", gap: "12px" }}>
      <textarea
        value={inputText}
        onChange={(e) => onInputChange(e.target.value)}
        placeholder="请输入要翻译的文本…"
        rows={4}
        style={{ resize: "vertical", fontFamily: "var(--qq-font)", fontSize: "14px", padding: "8px" }}
      />

      <button
        onClick={onTranslate}
        disabled={isTranslateDisabled}
        style={{ alignSelf: "flex-start" }}
      >
        {isLoading ? "翻译中…" : "翻译"}
      </button>

      {result !== null && (
        <div>
          <p style={{ color: "var(--qq-text-muted)", fontSize: "12px", margin: "0 0 4px" }}>
            {result.sourceLang} → {result.targetLang}
          </p>
          <p style={{ margin: "0 0 8px", fontFamily: "var(--qq-font)" }}>
            {result.translated}
          </p>
          <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
            {actions
              .filter((action) => action !== "save_history")
              .map((action) => {
                const resolved = resolveTranslateAction(action);
                if (resolved === null) return null;
                return (
                  <button
                    key={action}
                    onClick={() => onAction(action)}
                    aria-label={ACTION_LABELS[action] ?? action}
                  >
                    {ACTION_LABELS[action] ?? action}
                  </button>
                );
              })}
          </div>
        </div>
      )}
    </div>
  );
}

export default TranslateWorkspace;
