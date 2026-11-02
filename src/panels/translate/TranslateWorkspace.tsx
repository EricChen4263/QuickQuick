import type { ReactElement } from "react";
import type { TranslateResult, Provider } from "../../ipc/ipc-client";
import { ActionBar } from "../../components/ActionBar";
import { availableActions, resolveTranslateAction } from "../../translate/translate-actions";
import DirBar from "./DirBar";
import DictEntryView from "./DictEntryView";

/** 译文操作按钮的中文标签（具名常量，避免魔术字符串） */
const ACTION_LABELS: Record<string, string> = {
  copy: "复制",
  speak: "朗读",
  // save_history 已在后端自动写入历史，无独立按钮可点，故不渲染（见下方 filter）
  save_history: "刷新历史",
};

/**
 * 非官方源失败时的降级提示文案（设计文档§三.决策3）。
 * 与原始错误并列展示，引导用户切换其它源而非笼统报错。
 */
const UNOFFICIAL_DEGRADE_HINT =
  "该源为非官方接口，可能已失效，可切换其它翻译源重试。";

/** 官方源翻译失败时的下一步：不含「非官方」字样，避免与降级提示混淆。 */
const TRANSLATE_FAILED_HINT =
  "换一个翻译源再试，或在设置里检查这个源的接口配置。";

/** 译文操作（复制 / 朗读）失败时的下一步：此时译文还在，不必重新翻译。 */
const ACTION_FAILED_HINT = "再试一次；仍然不行时可以直接选中译文手动复制。";

/** 耗时展示：亚秒给毫秒，超过 1 秒给两位小数的秒（412 ms / 1.08 s）。 */
function formatElapsed(ms: number): string {
  return ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(2)} s`;
}

/** 复制图标：两层圆角纸片 */
function CopyIcon(): ReactElement {
  return (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <rect x="9" y="9" width="11" height="11" rx="2.5" />
      <path d="M15 9V6.5A2.5 2.5 0 0 0 12.5 4H6.5A2.5 2.5 0 0 0 4 6.5v6A2.5 2.5 0 0 0 6.5 15H9" />
    </svg>
  );
}

/** 朗读图标：喇叭 + 扩散弧 */
function SpeakerIcon(): ReactElement {
  return (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M11 5 6.6 8.8H3.5v6.4h3.1L11 19z" />
      <path d="M15 9.6a3.2 3.2 0 0 1 0 4.8" />
      <path d="M17.8 7a7 7 0 0 1 0 10" />
    </svg>
  );
}

/** 未翻译时的引导图标：翻译字形 */
function TranslateIcon(): ReactElement {
  return (
    <svg className="ico ico-20" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4 6h9" />
      <path d="M8.5 4v2" />
      <path d="M10.5 6c-.4 3.5-3.1 6.4-6.5 7.7" />
      <path d="M5.2 10.2c1.2 2.6 3.3 4.3 5.6 5.2" />
      <path d="m13.6 19.5 3.7-9 3.7 9" />
      <path d="M15 16.5h4.6" />
    </svg>
  );
}

/** 告警图标：圆圈感叹号 */
function AlertIcon(): ReactElement {
  return (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.8v4.6" />
      <path d="M12 16.2h.01" />
    </svg>
  );
}

/** 操作 → 图标；未列出的动作（save_history）不会渲染，无需图标。 */
function actionIcon(action: string): ReactElement | null {
  if (action === "copy") return <CopyIcon />;
  if (action === "speak") return <SpeakerIcon />;
  return null;
}

/**
 * 结果区里的失败提示：第一行是原始错误（谁、因为什么失败），
 * 第二行是下一步能做什么。两段都由调用处决定，因为「失败的是什么」需要分开说。
 */
function ResultAlert({ error, hint }: { error: string; hint: string }): ReactElement {
  return (
    <div role="alert" className="qq-note qq-note--danger">
      <span className="tx-alert-icon">
        <AlertIcon />
      </span>
      <div className="tx-alert-body">
        <p className="body-5">{error}</p>
        <p className="caption-5">{hint}</p>
      </div>
    </div>
  );
}

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
  /**
   * 上一次翻译的纯 IPC 耗时（毫秒），用于结果区元信息行。
   * 可选：不传时不展示耗时（如点历史条目回填，没有发生新的请求）。
   */
  elapsedMs?: number | null;
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
 *
 * 结果区是「一个位置、四种内容」：加载中（骨架）/ 有结果 / 失败 / 还没开始。
 * 四种互斥，避免同屏并存多种状态互相抢注意力。
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
  elapsedMs = null,
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
  const providerName = providers.find((p) => p.id === selectedProviderId)?.name ?? "";

  /**
   * 可对调时用的「源语」：显式源语优先。
   * 源语是「自动检测」时后端才知道真实语言，这里退回上一次结果检测到的语言——
   * 这也正是用户点「交换」时想表达的意思（把 en→zh 变成 zh→en）。
   * 一次结果都还没有过就没有可换的语言，按钮保持禁用。
   */
  const swapSource = sourceLang === "auto" ? result?.sourceLang ?? null : sourceLang;
  const canSwap = swapSource !== null && swapSource !== targetLang;

  function handleSwap(): void {
    if (!canSwap || swapSource === null) return;
    onSourceChange(targetLang);
    onTargetChange(swapSource);
  }

  const swapTitle = canSwap
    ? "对调源语言和目标语言"
    : "还没有可对调的源语：先翻译一次，或直接把源语选成具体语言";

  return (
    <div className="tx-work">
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

      <div className="tx-scroll">
        {/* ---------------------------------------------------------- 原文 ---- */}
        <div>
          <div className="tx-src-head">
            <label className="caption-4" htmlFor="tx-input">
              原文
            </label>
          </div>

          <div className="tx-field">
            <textarea
              id="tx-input"
              className="tx-input body-3"
              value={inputText}
              onChange={(e) => onInputChange(e.target.value)}
              placeholder="粘贴或输入要翻译的文字…"
              spellCheck={false}
            />
            <span className="tx-count caption-5 mono">{charCount} 字符</span>
          </div>

          <ActionBar variant="surface" className="tx-cta">
            <button
              className="btn btn-primary button-sm"
              onClick={() => onTranslate()}
              disabled={isTranslateDisabled}
            >
              {isLoading ? "翻译中…" : "翻译"}
            </button>
            <button
              className="btn btn-ghost btn-sm button-sm"
              onClick={() => onInputChange("")}
              disabled={charCount === 0 || isLoading}
            >
              清空
            </button>
            <button
              className="btn btn-ghost btn-sm button-sm"
              onClick={handleSwap}
              disabled={!canSwap || isLoading}
              aria-label="交换源语言和目标语言"
              title={swapTitle}
            >
              交换
            </button>
          </ActionBar>
        </div>

        <hr className="tx-rule" />

        {/* ---------------------------------------------------------- 结果 ---- */}
        <div className="tx-result">
          {isLoading ? (
            <div role="status" className="tx-loading">
              {/* 加载态用骨架屏而不是转圈：形状预示「这里马上出现一段话」 */}
              <div className="tx-skel" aria-hidden="true">
                <span className="skeleton tx-skel-line" />
                <span className="skeleton tx-skel-line" />
                <span className="skeleton tx-skel-line" />
              </div>
              <p className="tx-skel-note caption-5">
                翻译中…
                {providerName === "" ? null : <> · {providerName}</>}
              </p>
            </div>
          ) : result !== null ? (
            <>
              <div className="tx-kicker caption-4">
                {providerName === "" ? null : (
                  <>
                    <span>{providerName}</span>
                    <span className="tx-kicker-sep" aria-hidden="true">
                      ·
                    </span>
                  </>
                )}
                {elapsedMs === null ? null : (
                  <>
                    <span className="mono">{formatElapsed(elapsedMs)}</span>
                    <span className="tx-kicker-sep" aria-hidden="true">
                      ·
                    </span>
                  </>
                )}
                <span className="mono">
                  {result.sourceLang} → {result.targetLang}
                </span>
              </div>

              {result.kind === "dict" ? (
                <DictEntryView entry={result.entry} />
              ) : (
                <p className="title-1 tx-out">{result.translated}</p>
              )}

              {error !== null && (
                <ResultAlert error={error} hint={ACTION_FAILED_HINT} />
              )}

              <ActionBar variant="surface" className="tx-actions">
                {actions
                  // save_history 由后端在 translate_text 时自动写入，没有对应的独立动作
                  .filter((action) => action !== "save_history")
                  .map((action) => {
                    const resolved = resolveTranslateAction(action);
                    if (resolved === null) return null;
                    return (
                      <button
                        key={action}
                        className="btn btn-ghost btn-sm button-sm"
                        onClick={() => onAction(action)}
                        aria-label={ACTION_LABELS[action] ?? action}
                      >
                        {actionIcon(action)}
                        {ACTION_LABELS[action] ?? action}
                      </button>
                    );
                  })}
              </ActionBar>
            </>
          ) : error !== null ? (
            <ResultAlert
              error={error}
              hint={isSelectedUnofficial ? UNOFFICIAL_DEGRADE_HINT : TRANSLATE_FAILED_HINT}
            />
          ) : (
            <div className="tx-idle">
              <TranslateIcon />
              <div className="tx-idle-body">
                <p className="tx-idle-title body-5">译文会出现在这里</p>
                <p className="caption-5">
                  输入或粘贴一段文字，选好语言对再按「翻译」。翻好的句子会自动存进右侧历史。
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default TranslateWorkspace;
