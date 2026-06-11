import React from "react";
import { ActionBar } from "../components/ActionBar";
import type { TranslateResult } from "../ipc/ipc-client";

interface MiniTranslateProps {
  result: TranslateResult;
  onCopy: () => void;
  onSpeak: () => void;
  onExpand: () => void;
}

/**
 * 迷你翻译结果卡：方向行 + 译文区 + 操作行。
 * 纯展示组件，无副作用，所有动作由父组件 TransPopoverApp 传入。
 */
function MiniTranslate({ result, onCopy, onSpeak, onExpand }: MiniTranslateProps): React.ReactElement {
  return (
    <div className="mini-translate">
      <div className="mini-dir">
        {result.sourceLang} → {result.targetLang}
      </div>

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
