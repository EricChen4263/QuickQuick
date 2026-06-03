import type { Provider } from "../../ipc/ipc-client";
import { SOURCE_LANGUAGES, TARGET_LANGUAGES } from "./languages";

interface DirBarProps {
  sourceLang: string;
  targetLang: string;
  onSourceChange: (code: string) => void;
  onTargetChange: (code: string) => void;
  providers: Provider[];
  selectedProviderId: string;
  onProviderChange: (id: string) => void;
  /** 已完整配置凭据的 provider id 集合；needsKey 源仅在此集合内时才可选 */
  configuredIds?: Set<string>;
}

/**
 * 翻译方向栏：源语下拉 + 目标语下拉 + 翻译源选择器。
 * 纯展示组件，无副作用，所有状态由父组件 TranslatePage 持有。
 * 源语含"自动检测"选项；目标语不含（目标必须为具体语言）。
 */
function DirBar({
  sourceLang,
  targetLang,
  onSourceChange,
  onTargetChange,
  providers,
  selectedProviderId,
  onProviderChange,
  configuredIds = new Set(),
}: DirBarProps) {
  return (
    <div className="dir-bar">
      <span className="lang-selects">
        <span className="wrap">
          <select
            aria-label="源语言"
            className="lang-select"
            value={sourceLang}
            onChange={(e) => onSourceChange(e.target.value)}
          >
            {SOURCE_LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.label}
              </option>
            ))}
          </select>
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </span>

        <span className="wrap">
          <select
            aria-label="目标语言"
            className="lang-select"
            value={targetLang}
            onChange={(e) => onTargetChange(e.target.value)}
          >
            {TARGET_LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.label}
              </option>
            ))}
          </select>
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </span>
      </span>

      <span className="src-select">
        翻译源
        <span className="wrap">
          <select
            aria-label="翻译源"
            value={selectedProviderId}
            onChange={(e) => onProviderChange(e.target.value)}
          >
            {providers.map((p) => (
              // needsKey 源：已在设置页配好凭据（configuredIds 包含）时可选，否则禁用
              <option key={p.id} value={p.id} disabled={p.needsKey && !configuredIds.has(p.id)}>
                {p.name}
              </option>
            ))}
          </select>
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </span>
      </span>
    </div>
  );
}

export default DirBar;
