import { useEffect, useState, useCallback } from "react";
import {
  getExcludeList,
  setExcludeList,
  getExcludeMode,
  setExcludeMode,
  getIgnoreRegexes,
  setIgnoreRegexes,
  getPauseCapture,
  setPauseCapture,
  getSkipSensitive,
  setSkipSensitive,
} from "../../ipc/ipc-client";
import type { ExcludeMode } from "../../ipc/ipc-client";
import {
  addExcludedApp,
  removeExcludedApp,
  addIgnorePattern,
  removeIgnorePattern,
} from "../../main-window/settings/sections";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";
import SettingToggle from "./SettingToggle";

/** SVG × 图标，用于 chip 删除按钮 */
function CloseIcon() {
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
      <path d="M2 2l8 8M10 2l-8 8" />
    </svg>
  );
}

/** 名单模式的中文名（两态按钮文案） */
const MODE_LABEL: Record<ExcludeMode, string> = {
  blacklist: "黑名单",
  whitelist: "白名单",
};

/**
 * 名单模式说明：把方向差异说清楚。
 *
 * 「黑名单」与「白名单」两个词本身不解释方向，用户需要一句话知道
 * 「名单里的应用到底记还是不记」。
 */
const MODE_DESCRIPTION: Record<ExcludeMode, string> = {
  blacklist: "黑名单：名单内应用的内容不记录",
  whitelist: "白名单：只记录名单内应用的内容",
};

/**
 * 名单输入框说明：随模式变化，避免用户按旧心智填条目。
 *
 * 白名单那句必须点明「识别不到来源应用的内容也不记」：此时规则不是「名单外的跳过」，
 * 而是「证明不了它在名单内就不记」。macOS 拿不到来源身份（如某些辅助进程写入、
 * 或身份缓存被重置）时会静默不记录，不写清用户会以为「这个 app 的内容全丢了」。
 */
const LIST_DESCRIPTION: Record<ExcludeMode, string> = {
  blacklist: "处于前台时不记录剪贴板的应用",
  whitelist:
    "只有这些应用处于前台时才记录剪贴板；识别不到来源应用的内容也不会被记录",
};

/** 内容忽略正则的说明：写清匹配对象与失败方向 */
const REGEX_DESCRIPTION =
  "文本内容命中任一正则就不入库；图片没有文本，不参与匹配。按 Rust regex 语法填写，写错的那条只失效，不会拦掉其它内容。";

/** 正则输入框占位符：给一个能直接用的例子比抽象说明更有用 */
const REGEX_PLACEHOLDER = "正则，如 ^\\d{16}$";

interface ModeOptionProps {
  mode: ExcludeMode;
  current: ExcludeMode;
  onSelect: (mode: ExcludeMode) => void;
}

/** 两态模式切换里的单个选项（radiogroup 语义，键盘可达） */
function ModeOption({ mode, current, onSelect }: ModeOptionProps) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={mode === current}
      aria-label={MODE_LABEL[mode]}
      className="mode-option"
      onClick={() => onSelect(mode)}
    >
      {MODE_LABEL[mode]}
    </button>
  );
}

/** 隐私子项面板：管理名单模式/App 名单/内容正则 + 暂停/跳过敏感开关 */
function PrivacyPanel() {
  const [excludeList, setExcludeListState] = useState<string[]>([]);
  const [excludeMode, setExcludeModeState] = useState<ExcludeMode>("blacklist");
  const [regexPatterns, setRegexPatterns] = useState<string[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [regexInput, setRegexInput] = useState("");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);
  const [pauseCaptureState, setPauseCaptureState] = useState(false);
  const [skipSensitiveState, setSkipSensitiveState] = useState(true);

  const fetchExcludeList = useCallback(async (cancelled: { current: boolean }) => {
    try {
      const list = await getExcludeList();
      if (cancelled.current) return;
      setExcludeListState(list);
      setLoadError(null);
    } catch {
      if (cancelled.current) return;
      setLoadError("排除名单加载失败，请稍后重试");
    }
  }, []);

  useEffect(() => {
    const cancelled = { current: false };
    fetchExcludeList(cancelled);

    Promise.all([
      getPauseCapture(),
      getSkipSensitive(),
      getExcludeMode(),
      getIgnoreRegexes(),
    ])
      .then(([paused, skip, mode, patterns]) => {
        if (cancelled.current) return;
        setPauseCaptureState(paused);
        setSkipSensitiveState(skip);
        setExcludeModeState(mode);
        setRegexPatterns(patterns);
      })
      .catch(() => {
        if (cancelled.current) return;
        console.warn("隐私开关初始化失败，保留默认值");
      });

    return () => {
      cancelled.current = true;
    };
  }, [fetchExcludeList]);

  async function handleModeChange(next: ExcludeMode) {
    if (next === excludeMode) return;
    try {
      // 只切换方向：已填的应用条目与内容正则由后端一并保留，前端不动它们
      await setExcludeMode(next);
      setExcludeModeState(next);
      setOpError(null);
    } catch {
      setOpError("切换名单模式失败，请稍后重试");
    }
  }

  async function handleAdd() {
    const trimmed = inputValue.trim();
    if (trimmed.length === 0) return;

    const newList = addExcludedApp(excludeList, trimmed);
    try {
      await setExcludeList(newList);
      setExcludeListState(newList);
      setInputValue("");
      setOpError(null);
    } catch {
      setOpError("添加失败，请稍后重试");
    }
  }

  async function handleRemove(app: string) {
    const newList = removeExcludedApp(excludeList, app);
    try {
      await setExcludeList(newList);
      setExcludeListState(newList);
      setOpError(null);
    } catch {
      setOpError("删除失败，请稍后重试");
    }
  }

  async function handleAddRegex() {
    const trimmed = regexInput.trim();
    if (trimmed.length === 0) return;

    const newPatterns = addIgnorePattern(regexPatterns, trimmed);
    try {
      await setIgnoreRegexes(newPatterns);
      setRegexPatterns(newPatterns);
      setRegexInput("");
      setOpError(null);
    } catch {
      setOpError("添加正则失败，请稍后重试");
    }
  }

  async function handleRemoveRegex(pattern: string) {
    const newPatterns = removeIgnorePattern(regexPatterns, pattern);
    try {
      await setIgnoreRegexes(newPatterns);
      setRegexPatterns(newPatterns);
      setOpError(null);
    } catch {
      setOpError("删除正则失败，请稍后重试");
    }
  }

  async function handlePauseCaptureChange(v: boolean) {
    try {
      await setPauseCapture(v);
      setPauseCaptureState(v);
      setOpError(null);
    } catch {
      setOpError("暂停监听设置失败，请稍后重试");
    }
  }

  async function handleSkipSensitiveChange(v: boolean) {
    try {
      await setSkipSensitive(v);
      setSkipSensitiveState(v);
      setOpError(null);
    } catch {
      setOpError("跳过敏感内容设置失败，请稍后重试");
    }
  }

  if (loadError !== null) {
    return (
      <div>
        <div role="alert" style={{ color: "var(--danger)" }}>{loadError}</div>
      </div>
    );
  }

  return (
    <div>
      <PanelHeader title="隐私" subtitle="以下规则决定哪些剪贴板内容不被记录" />

      <SettingGroup>
        <SettingToggle
          label="暂停剪贴板监听"
          description="临时停止记录所有剪贴板内容"
          checked={pauseCaptureState}
          onChange={(v) => { void handlePauseCaptureChange(v); }}
        />
        <SettingToggle
          label="跳过敏感内容"
          description="自动过滤密码管理器等敏感数据"
          checked={skipSensitiveState}
          onChange={(v) => { void handleSkipSensitiveChange(v); }}
        />
      </SettingGroup>

      <SettingGroup>
        <div className="set-row">
          <div className="grow">
            <div className="label">App 名单模式</div>
            <div className="desc">{MODE_DESCRIPTION[excludeMode]}</div>
          </div>
          <div className="mode-switch" role="radiogroup" aria-label="App 名单模式">
            <ModeOption
              mode="blacklist"
              current={excludeMode}
              onSelect={(m) => { void handleModeChange(m); }}
            />
            <ModeOption
              mode="whitelist"
              current={excludeMode}
              onSelect={(m) => { void handleModeChange(m); }}
            />
          </div>
        </div>

        <div className="set-row">
          <div className="grow">
            <div className="label">App 名单</div>
            <div className="desc">{LIST_DESCRIPTION[excludeMode]}</div>
          </div>
          <input
            type="text"
            className="set-input"
            value={inputValue}
            placeholder="应用名称（如 1Password）"
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") void handleAdd(); }}
          />
          <button className="btn" aria-label="添加应用" onClick={() => void handleAdd()}>
            添加
          </button>
        </div>
        <div className="chip-row">
          {excludeList.map((app) => (
            <span key={app} className="chip">
              {app}
              <button
                type="button"
                aria-label={`删除 ${app}`}
                onClick={() => void handleRemove(app)}
              >
                <CloseIcon />
              </button>
            </span>
          ))}
          {excludeList.length === 0 && (
            <span style={{ fontSize: 12, color: "var(--muted)" }}>暂无排除应用</span>
          )}
        </div>
      </SettingGroup>

      <SettingGroup>
        <div className="set-row">
          <div className="grow">
            <div className="label">内容忽略正则</div>
            <div className="desc">{REGEX_DESCRIPTION}</div>
          </div>
          <input
            type="text"
            className="set-input"
            value={regexInput}
            placeholder={REGEX_PLACEHOLDER}
            onChange={(e) => setRegexInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") void handleAddRegex(); }}
          />
          <button className="btn" aria-label="添加正则" onClick={() => void handleAddRegex()}>
            添加
          </button>
        </div>
        <div className="chip-row">
          {regexPatterns.map((pattern) => (
            <span key={pattern} className="chip">
              <code>{pattern}</code>
              <button
                type="button"
                aria-label={`删除正则 ${pattern}`}
                onClick={() => void handleRemoveRegex(pattern)}
              >
                <CloseIcon />
              </button>
            </span>
          ))}
          {regexPatterns.length === 0 && (
            <span style={{ fontSize: 12, color: "var(--muted)" }}>暂无内容规则</span>
          )}
        </div>
      </SettingGroup>

      {opError !== null && (
        <div role="alert" style={{ color: "var(--danger)", marginTop: 8, fontSize: 12 }}>
          {opError}
        </div>
      )}
    </div>
  );
}

export default PrivacyPanel;
