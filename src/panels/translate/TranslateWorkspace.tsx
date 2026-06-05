import type { TranslateResult, Provider } from "../../ipc/ipc-client";
import { availableActions, resolveTranslateAction } from "../../translate/translate-actions";
import DirBar from "./DirBar";

/** 译文操作按钮的中文标签（具名常量，避免魔术字符串） */
const ACTION_LABELS: Record<string, string> = {
  copy: "复制",
  speak: "朗读",
  // save_history 已自动写入，展示"刷新历史"语义由父组件处理，此处禁用
  save_history: "刷新历史",
};

/**
 * 非官方源失败时的降级提示文案（设计文档§三.决策3）。
 * 与原始错误并列展示，引导用户切换其它源而非笼统报错。
 */
const UNOFFICIAL_DEGRADE_HINT =
  "该源为非官方接口，可能已失效，可切换其它翻译源重试。";

interface TranslateWorkspaceProps {
  inputText: string;
  result: TranslateResult | null;
  isLoading: boolean;
  error: string | null;
  sourceLang: string;
  targetLang: string;
  providers: Provider[];
  selectedProviderId: string;
  /** 已完整配置凭据的 provider id 集合，透传给 DirBar 解禁已配置的 keyed 源 */
  configuredIds: Set<string>;
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
  configuredIds,
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
  // 当前选中源是否为非官方接口：失败时据此追加降级提示（设计文档§三.决策3）。
  const isSelectedUnofficial = providers.some(
    (p) => p.id === selectedProviderId && p.isUnofficial
  );

  return (
    <div className="tx-work">
      <div className="pane-head">
        <span className="pane-title">翻译</span>
      </div>

      <div className="tx-scroll">
        <DirBar
          sourceLang={sourceLang}
          targetLang={targetLang}
          providers={providers}
          selectedProviderId={selectedProviderId}
          configuredIds={configuredIds}
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

        {error !== null && (
          <div className="tx-result">
            <div
              role="alert"
              className="tx-out"
              style={{ color: "var(--danger)" }}
            >
              {error}
              {isSelectedUnofficial && (
                <div className="tx-degrade-hint">{UNOFFICIAL_DEGRADE_HINT}</div>
              )}
            </div>
          </div>
        )}

        {error === null && isLoading && (
          // 翻译进行中：在用户视线所在的结果区给出明确进度反馈，
          // 并借三态优先级（isLoading 先于 result）盖掉上一次的旧译文，
          // 避免用户误以为「点了没反应」。role=status 供无障碍朗读进度。
          <div className="tx-result">
            <div role="status" className="tx-loading">
              翻译中…
            </div>
          </div>
        )}

        {error === null && !isLoading && result !== null && (
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
          </div>
        )}
      </div>
    </div>
  );
}

export default TranslateWorkspace;
