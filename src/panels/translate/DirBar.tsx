import type { Provider } from "../../ipc/ipc-client";

interface DirBarProps {
  sourceLang: string;
  targetLang: string;
  providers: Provider[];
  selectedProviderId: string;
  onProviderChange: (id: string) => void;
}

/**
 * 翻译方向栏：语言方向药丸 + 翻译源选择器。
 * 纯展示组件，无副作用，所有状态由父组件 TranslatePage 持有。
 */
function DirBar({ sourceLang, targetLang, providers, selectedProviderId, onProviderChange }: DirBarProps) {
  return (
    <div className="dir-bar">
      <span className="lang-pill">
        {sourceLang}
        <svg
          className="swap"
          viewBox="0 0 24 24"
          width="15"
          height="15"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M8 3 4 7l4 4" />
          <path d="M4 7h16" />
          <path d="m16 21 4-4-4-4" />
          <path d="M20 17H4" />
        </svg>
        {targetLang}
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
              // v1 未提供 key 配置入口，故 needsKey 源暂禁用；里程碑3 若加 key 配置再解禁
              <option key={p.id} value={p.id} disabled={p.needsKey}>
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
