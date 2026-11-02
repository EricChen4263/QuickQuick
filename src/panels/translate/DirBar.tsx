import { useMemo } from "react";
import type { Provider } from "../../ipc/ipc-client";
import { Select } from "../../components/Select";
import type { SelectOption } from "../../components/Select";
import { SOURCE_LANGUAGES, TARGET_LANGUAGES } from "./languages";

interface DirBarProps {
  sourceLang: string;
  targetLang: string;
  onSourceChange: (code: string) => void;
  onTargetChange: (code: string) => void;
  providers: Provider[];
  selectedProviderId: string;
  onProviderChange: (id: string) => void;
  /** 已完整配置凭据的 provider id 集合；可配置源（needsConfig）仅在此集合内时才可选 */
  configuredIds?: Set<string>;
}

/** 语言常量 → Select 选项；两处语言列表结构相同，统一映射避免重复。 */
function toLangOptions(langs: readonly { code: string; label: string }[]): SelectOption[] {
  return langs.map((l) => ({ value: l.code, label: l.label }));
}

const SOURCE_OPTIONS = toLangOptions(SOURCE_LANGUAGES);
const TARGET_OPTIONS = toLangOptions(TARGET_LANGUAGES);

/**
 * 翻译方向栏：源语下拉 + 目标语下拉 + 翻译源选择器。
 * 纯展示组件，无副作用，所有状态由父组件 TranslatePage 持有。
 *
 * 源语含"自动检测"选项；目标语不含（目标必须为具体语言）。
 * 三处下拉用自定义 Select，绕开 Overlay 标题栏下原生弹窗的坐标错位。
 *
 * 两个语言下拉被 .lang-selects 的容器底色包成一个分段控件，中间的箭头是纯装饰
 * （不进无障碍树、不可点）。真正改变方向的「交换」动作由工作区的操作行发起，
 * 因为它需要同时写回源语与目标语两个值。
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
  // 翻译源选项：可配置源（needsConfig，含 Ollama 等无 key 但有必填字段）仅在已配置时可选，否则禁用；
  // 非官方源在名称后加「⚠ 非官方」标注，提示其随对方改版可能失效（设计文档§三.决策3）。
  const providerOptions = useMemo<SelectOption[]>(
    () =>
      providers.map((p) => ({
        value: p.id,
        label: p.isUnofficial ? `${p.name}  ⚠ 非官方` : p.name,
        disabled: p.needsConfig && !configuredIds.has(p.id),
      })),
    [providers, configuredIds]
  );

  return (
    <div className="dir-bar">
      <div className="lang-selects">
        <Select
          ariaLabel="源语言"
          value={sourceLang}
          options={SOURCE_OPTIONS}
          onChange={onSourceChange}
        />
        {/* 方向箭头：表意"源语 → 目标语"，纯装饰，不进无障碍树、不可点 */}
        <svg className="ico lang-dir-arrow" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M4 12h15" />
          <path d="m14.5 7 5 5-5 5" />
        </svg>
        <Select
          ariaLabel="目标语言"
          value={targetLang}
          options={TARGET_OPTIONS}
          onChange={onTargetChange}
        />
      </div>

      <div className="tx-src-select">
        <span className="caption-4">翻译源</span>
        <Select
          ariaLabel="翻译源"
          value={selectedProviderId}
          options={providerOptions}
          onChange={onProviderChange}
        />
      </div>
    </div>
  );
}

export default DirBar;
