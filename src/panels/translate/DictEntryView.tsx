import { useRef } from "react";
import type { DictEntry } from "../../ipc/ipc-client";

interface DictEntryViewProps {
  entry: DictEntry;
}

/** 发音按钮图标（Phosphor 语汇：喇叭 + 两道扩散弧） */
function SpeakerIcon() {
  return (
    <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M11 5 6.6 8.8H3.5v6.4h3.1L11 19z" />
      <path d="M15 9.6a3.2 3.2 0 0 1 0 4.8" />
      <path d="M17.8 7a7 7 0 0 1 0 10" />
    </svg>
  );
}

/**
 * 词典词条展示组件：按 DictEntry 结构化渲染音标、按词性分组释义、例句、发音、变形。
 *
 * 各可选区块（音标 / 例句 / 发音 / 变形）仅在有值时渲染，避免空区块占位。
 * 释义按词性（pos）分组：有词性时渲染词性标签，无词性时只列释义。
 *
 * 本组件同时被主窗翻译页与翻译浮窗（src/trans-popover）复用：
 * 结构与类名保持不变（.dict-entry / .dict-def-group / .dict-section-label / …），
 * 两个入口的排版差异全部由各自的 CSS 前缀决定（.view-translate / .mini-translate）。
 *
 * 发音用「图标按钮 + 隐藏的 <audio>」而不是原生 audio 控件：原生控件会带来
 * 第二种按钮外观。浮窗那边仍保留原生播放器，按钮由 .mini-translate .dict-speak 隐藏。
 */
function DictEntryView({ entry }: DictEntryViewProps) {
  const { phonetic, definitions, examples, audio, inflections } = entry;
  const audioRef = useRef<HTMLAudioElement | null>(null);

  const hasPhonetic = phonetic !== null && phonetic.length > 0;
  const hasAudio = audio !== null && audio.length > 0;

  /** 从头播放词条音频。无音频设备 / 测试环境下播放会被拒绝，忽略即可。 */
  function playAudio(): void {
    const el = audioRef.current;
    if (el === null) return;
    el.currentTime = 0;
    try {
      void el.play();
    } catch {
      // 播放失败不影响词条本身，用户可再点一次
    }
  }

  return (
    <div className="dict-entry">
      {(hasPhonetic || hasAudio) && (
        <div className="dict-head">
          {hasPhonetic && (
            <span className="dict-phonetic title-1 mono" data-testid="dict-phonetic">
              {phonetic}
            </span>
          )}
          {hasAudio && (
            <>
              <button
                type="button"
                className="icon-btn dict-speak"
                onClick={playAudio}
                aria-label="播放发音"
              >
                <SpeakerIcon />
              </button>
              <audio ref={audioRef} className="dict-audio" data-testid="dict-audio" src={audio} preload="none" />
            </>
          )}
        </div>
      )}

      <ul className="dict-defs">
        {definitions.map((def, defIndex) => (
          <li key={defIndex} className="dict-def-group">
            {def.pos !== null && def.pos.length > 0 && (
              <span className="dict-pos badge mono caption-4" data-testid="dict-pos">
                {def.pos}
              </span>
            )}
            <ul className="dict-meanings">
              {def.meanings.map((meaning, meaningIndex) => (
                <li key={meaningIndex} className="dict-meaning body-3">
                  {meaning}
                </li>
              ))}
            </ul>
          </li>
        ))}
      </ul>

      {examples.length > 0 && (
        <div className="dict-examples" data-testid="dict-examples">
          <span className="dict-section-label caption-4">例句</span>
          <ul className="dict-example-list">
            {examples.map((example, exampleIndex) => (
              <li key={exampleIndex} className="dict-example body-6">
                {example}
              </li>
            ))}
          </ul>
        </div>
      )}

      {inflections.length > 0 && (
        <div className="dict-inflections" data-testid="dict-inflections">
          <span className="dict-section-label caption-4">变形</span>
          <span className="dict-inflection-words mono caption-1">
            {inflections.join("、")}
          </span>
        </div>
      )}
    </div>
  );
}

export default DictEntryView;
