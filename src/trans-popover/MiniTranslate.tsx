import React from "react";
import { ActionBar } from "../components/ActionBar";
import type {
  TransFallbackReason,
  TransSourceKind,
  TranslateResult,
} from "../ipc/ipc-client";

/** 来源标签（D4）：不标来源时用户会把「译的是剪贴板旧内容」误当成选中文字。 */
const SOURCE_LABEL: Record<TransSourceKind, string> = {
  selection: "选中",
  clipboard: "剪贴板",
};

/**
 * 降级说明文案。
 *
 * 只有「取词通道本身不可用」才值得解释：无选中是常态（用户就是随手按了热键），
 * 解释它只会变成噪音；未授权与平台不支持则是用户能采取动作的差异点。
 */
const FALLBACK_HINT: Partial<Record<TransFallbackReason, string>> = {
  untrusted: "未授权辅助功能，已改为译剪贴板内容",
  unsupported: "当前系统不支持选中取词，已改为译剪贴板内容",
};

/** 来源标签文案；浮窗的译文态与「无可译内容」态共用同一份口径。 */
export function sourceLabel(source: TransSourceKind): string {
  return SOURCE_LABEL[source];
}

/** 降级说明文案；没有需要解释的原因时返回 undefined（不渲染提示行）。 */
export function fallbackHint(reason: TransFallbackReason | null): string | undefined {
  return reason ? FALLBACK_HINT[reason] : undefined;
}

interface MiniTranslateProps {
  result: TranslateResult;
  /** 待译文本的实际来源，决定「来源：选中 / 来源：剪贴板」标签。 */
  source: TransSourceKind;
  /** 降级到剪贴板的原因；为空时不额外提示。 */
  fallbackReason?: TransFallbackReason | null;
  onCopy: () => void;
  onSpeak: () => void;
  onExpand: () => void;
}

/**
 * 迷你翻译结果卡：方向行（含来源标签）+ 降级说明 + 译文区 + 操作行。
 * 纯展示组件，无副作用，所有动作由父组件 TransPopoverApp 传入。
 */
function MiniTranslate({
  result,
  source,
  fallbackReason,
  onCopy,
  onSpeak,
  onExpand,
}: MiniTranslateProps): React.ReactElement {
  const hint = fallbackHint(fallbackReason ?? null);

  return (
    <div className="mini-translate">
      <div className="mini-dir">
        <span>
          {result.sourceLang} → {result.targetLang}
        </span>
        <span className="mini-source">来源：{sourceLabel(source)}</span>
      </div>

      {hint !== undefined && <div className="mini-degrade">{hint}</div>}

      <div className="mini-body">
        {result.translated}
      </div>

      <ActionBar variant="glass" className="mini-actions">
        <button type="button" className="btn" aria-label="复制" onClick={onCopy}>
          复制
        </button>
        <button type="button" className="btn" aria-label="朗读" onClick={onSpeak}>
          朗读
        </button>
        <button type="button" className="btn" aria-label="展开" onClick={onExpand}>
          展开
        </button>
      </ActionBar>
    </div>
  );
}

export default MiniTranslate;
