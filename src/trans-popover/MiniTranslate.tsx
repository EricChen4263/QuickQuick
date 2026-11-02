import React from "react";
import { ActionBar } from "../components/ActionBar";
import DictEntryView from "../panels/translate/DictEntryView";
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
 * 译文区内容：词典结果渲染结构化词条而非纯文本摘要——浮窗只有 320×200 可视高度，
 * 「音标 + 词性分组 + 例句」比一整段摘要更快被扫读；plain 结果维持纯文本。
 * 摘要（result.translated）在两类结果下都只用于复制 / 朗读，故这里不重复展示。
 */
function renderBody(result: TranslateResult): React.ReactElement {
  if (result.kind === "dict") {
    return <DictEntryView entry={result.entry} />;
  }
  return <>{result.translated}</>;
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
        <span className="caption-4">
          {result.sourceLang} → {result.targetLang}
        </span>
        <span className="mini-source badge caption-5">来源：{sourceLabel(source)}</span>
      </div>

      {hint !== undefined && <div className="mini-degrade caption-5">{hint}</div>}

      {/* 纯文本译文用 .body-2（16/24）：320px 窄窗里 18/28 一行只放得下 13 个字，
          一段真实译文要占 5–6 行，正在读的那一行还会被窗口底边切掉半截。
          16/24 是「一眼扫完」的上限，也是全站正文档；词典结果维持 14px 密集排版。 */}
      <div className={result.kind === "dict" ? "mini-body" : "mini-body body-2"}>
        {renderBody(result)}
      </div>

      <ActionBar variant="glass" className="mini-actions">
        <button
          type="button"
          className="btn btn-sm button-xsm"
          aria-label="复制"
          onClick={onCopy}
        >
          <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
            <rect x="9" y="9" width="11" height="11" rx="2.5" />
            <path d="M15 6.5A2.5 2.5 0 0 0 12.5 4h-6A2.5 2.5 0 0 0 4 6.5v6A2.5 2.5 0 0 0 6.5 15" />
          </svg>
          复制
        </button>
        <button
          type="button"
          className="btn btn-sm button-xsm"
          aria-label="朗读"
          onClick={onSpeak}
        >
          <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M11 5.5 6.5 9H3.5v6h3L11 18.5z" />
            <path d="M15 9.2a4 4 0 0 1 0 5.6" />
            <path d="M17.8 6.6a7.5 7.5 0 0 1 0 10.8" />
          </svg>
          朗读
        </button>
        <button
          type="button"
          className="btn btn-sm button-xsm"
          aria-label="展开"
          onClick={onExpand}
        >
          <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M13.5 4.5h6v6" />
            <path d="M19.5 4.5 12 12" />
            <path d="M17 14v4.5a1.5 1.5 0 0 1-1.5 1.5h-10A1.5 1.5 0 0 1 4 18.5v-10A1.5 1.5 0 0 1 5.5 7H10" />
          </svg>
          展开
        </button>
      </ActionBar>
    </div>
  );
}

export default MiniTranslate;
