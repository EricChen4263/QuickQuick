import type { TranslateResult, Provider } from "../../ipc/ipc-client";
import { availableActions, resolveTranslateAction } from "../../translate/translate-actions";
import DirBar from "./DirBar";

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
  error: string | null;
  sourceLang: string;
  targetLang: string;
  providers: Provider[];
  selectedProviderId: string;
  onInputChange: (text: string) => void;
  onTranslate: (textOverride?: string) => void;
  onSourceChange: (code: string) => void;
  onTargetChange: (code: string) => void;
  onAction: (action: string) => void;
  onProviderChange: (id: string) => void;
}

/**
 * 翻译工作区：语言方向栏 + 输入框 + 翻译按钮 + 译文展示 + 操作按钮条。
 * 纯展示组件，无副作用，所有状态由父组件 TranslatePage 持有。
 */
function TranslateWorkspace({
  inputText,
  result,
  isLoading,
  error,
  sourceLang,
  targetLang,
  providers,
  selectedProviderId,
  onInputChange,
  onTranslate,
  onSourceChange,
  onTargetChange,
  onAction,
  onProviderChange,
}: TranslateWorkspaceProps) {
  const actions = availableActions();
  const isTranslateDisabled = inputText.trim().length === 0 || isLoading;
  const charCount = inputText.length;

  return (
    <div className="tx-work">
      {error !== null && (
        <div role="alert" className="tx-error">
          {error}
        </div>
      )}
      <div className="pane-head">
        <span className="pane-title">翻译</span>
      </div>

      <div className="tx-scroll">
        <DirBar
          sourceLang={sourceLang}
          targetLang={targetLang}
          providers={providers}
          selectedProviderId={selectedProviderId}
          onProviderChange={onProviderChange}
          onSourceChange={onSourceChange}
          onTargetChange={onTargetChange}
        />

        <div className="field-label">原文</div>
        <textarea
          className="tx-input"
          value={inputText}
          onChange={(e) => onInputChange(e.target.value)}
          placeholder="请输入要翻译的文本…"
          spellCheck={false}
        />

        <div className="tx-cta">
          <button
            className="btn btn-primary"
            onClick={() => onTranslate()}
            disabled={isTranslateDisabled}
          >
            {isLoading ? "翻译中…" : "翻译"}
          </button>
          <span className="meta">{charCount} 字符</span>
        </div>

        {result !== null && (
          <div className="tx-result">
            <div className="field-label">
              译文 · {result.sourceLang} → {result.targetLang}
            </div>
            <div className="tx-out">{result.translated}</div>

            <div className="tx-actions">
              {actions
                .filter((action) => action !== "save_history")
                .map((action) => {
                  const resolved = resolveTranslateAction(action);
                  if (resolved === null) return null;
                  return (
                    <button
                      key={action}
                      className="btn"
                      onClick={() => onAction(action)}
                      aria-label={ACTION_LABELS[action] ?? action}
                    >
                      {ACTION_LABELS[action] ?? action}
                    </button>
                  );
                })}
            </div>

            <div className="dict-slot" aria-label="词典区">
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.7"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
              </svg>
              词典区位置预留 —— 单词查询时显示音标 / 词性 / 例句（fast-follow）
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default TranslateWorkspace;
