import type { DictEntry } from "../../ipc/ipc-client";

interface DictEntryViewProps {
  entry: DictEntry;
}

/**
 * 词典词条展示组件：按 DictEntry 结构化渲染音标、按词性分组释义、例句、发音、变形。
 *
 * 各可选区块（音标 / 例句 / 发音 / 变形）仅在有值时渲染，避免空区块占位。
 * 释义按词性（pos）分组：有词性时渲染词性标签，无词性时只列释义。
 */
function DictEntryView({ entry }: DictEntryViewProps) {
  const { phonetic, definitions, examples, audio, inflections } = entry;

  return (
    <div className="dict-entry">
      {phonetic !== null && phonetic.length > 0 && (
        <div className="dict-phonetic" data-testid="dict-phonetic">
          {phonetic}
        </div>
      )}

      <ul className="dict-defs">
        {definitions.map((def, defIndex) => (
          <li key={defIndex} className="dict-def-group">
            {def.pos !== null && def.pos.length > 0 && (
              <span className="dict-pos" data-testid="dict-pos">
                {def.pos}
              </span>
            )}
            <ul className="dict-meanings">
              {def.meanings.map((meaning, meaningIndex) => (
                <li key={meaningIndex} className="dict-meaning">
                  {meaning}
                </li>
              ))}
            </ul>
          </li>
        ))}
      </ul>

      {examples.length > 0 && (
        <div className="dict-examples" data-testid="dict-examples">
          <div className="dict-section-label">例句</div>
          <ul className="dict-example-list">
            {examples.map((example, exampleIndex) => (
              <li key={exampleIndex} className="dict-example">
                {example}
              </li>
            ))}
          </ul>
        </div>
      )}

      {inflections.length > 0 && (
        <div className="dict-inflections" data-testid="dict-inflections">
          <span className="dict-section-label">变形</span>
          <span className="dict-inflection-words">{inflections.join("、")}</span>
        </div>
      )}

      {audio !== null && audio.length > 0 && (
        <audio
          className="dict-audio"
          data-testid="dict-audio"
          src={audio}
          controls
        />
      )}
    </div>
  );
}

export default DictEntryView;
