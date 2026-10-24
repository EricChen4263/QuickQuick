/**
 * 首次引导向导（多步）及其状态机。
 *
 * 步骤顺序：欢迎 → 热键介绍 → 辅助功能授权 → 自启确认，最后进入终态。
 * 状态机是纯函数（见本文件下半部分），组件只负责把状态画出来并上报事件，
 * 「步骤推进 / 跳过 / 授权后自动收起 / 终态不再出现」因此都能脱离 DOM 单测。
 *
 * 「授权后自动恢复」的落点：用户去系统设置点完允许回到面板，父组件把新的授权观测值
 * 喂给 observeAccessibilityTrust，授权步骤立即收起并前进到自启确认——收起的是这一步，
 * 不是整个向导（后面还有自启确认要问）。授权一旦为真，前进也不会再回到该步骤。
 */

/** 向导步骤，`done` 是终态（引导不再渲染）。 */
export type GuideStep = "welcome" | "hotkey" | "accessibility" | "autostart" | "done";

/** 可停留的步骤（不含终态），顺序即前进方向。 */
export const GUIDE_STEPS = ["welcome", "hotkey", "accessibility", "autostart"] as const;

/** 向导状态：当前步骤 + 最近一次观测到的辅助功能授权状态。 */
export interface GuideState {
  step: GuideStep;
  accessibilityTrusted: boolean;
}

/** 首次进入向导：停在欢迎页，并记下当前授权观测值。 */
export function createGuideState(accessibilityTrusted: boolean): GuideState {
  return { step: "welcome", accessibilityTrusted };
}

/** 序列中的下一步；`done` 是吸收态。 */
function nextStep(step: GuideStep): GuideStep {
  switch (step) {
    case "welcome":
      return "hotkey";
    case "hotkey":
      return "accessibility";
    case "accessibility":
      return "autostart";
    default:
      return "done";
  }
}

/**
 * 前进一步。
 *
 * 已授权时跳过辅助功能授权步骤：授权过的用户不该再被问一遍「去系统设置授权」。
 * 跳过发生在前进到该步骤的瞬间，而不是进向导时一次算好——用户可能在向导开着的时候
 * 才去系统设置点了允许。
 */
export function advanceGuide(state: GuideState): GuideState {
  const next = nextStep(state.step);
  const step: GuideStep =
    next === "accessibility" && state.accessibilityTrusted ? nextStep(next) : next;
  return { ...state, step };
}

/**
 * 跳过当前步骤（「稍后」/「跳过」语义）。
 *
 * 欢迎页与终态不可跳过——否则一进来就能把向导跳没，等于没有引导。
 */
export function skipGuideStep(state: GuideState): GuideState {
  if (state.step === "welcome" || state.step === "done") return state;
  return advanceGuide(state);
}

/**
 * 观测辅助功能授权状态。
 *
 * 关键分支：从未授权变为已授权、且正停在授权步骤时，该步骤立即收起并前进到自启确认。
 * 用户刚在系统设置里点完「允许」，不需要再回面板点一次「下一步」。
 */
export function observeAccessibilityTrust(
  state: GuideState,
  trusted: boolean,
): GuideState {
  const updated: GuideState = { ...state, accessibilityTrusted: trusted };
  if (trusted && state.step === "accessibility") {
    return { ...updated, step: "autostart" };
  }
  return updated;
}

/** 结束引导（点「完成」或关闭按钮）：进入终态，此后不再出现。 */
export function finishGuide(state: GuideState): GuideState {
  return { ...state, step: "done" };
}

/** 引导是否应渲染。 */
export function isGuideVisible(state: GuideState): boolean {
  return state.step !== "done";
}

interface OnboardCardProps {
  state: GuideState;
  /** 当前开机自启偏好，由父组件从 autostart IPC 读到（默认开，与后端一致）。 */
  launchOnLogin: boolean;
  /** 历史面板热键展示文本，如 "⌘⇧C"；缺省用默认热键。 */
  hotkeyLabel?: string;
  onAdvance: () => void;
  onSkip: () => void;
  onDismiss: () => void;
  onOpenSystemSettings: () => void;
  onLaunchOnLoginChange: (enabled: boolean) => void;
}

/** 默认历史面板热键（后端 HotkeyConfig 默认 CmdOrCtrl+Shift+C 的展示形态）。 */
const DEFAULT_HOTKEY_LABEL = "⌘⇧C";

/** 加速键修饰键名 → 面板展示符号。 */
const MODIFIER_SYMBOLS: Record<string, string> = {
  CmdOrCtrl: "⌘",
  Command: "⌘",
  Cmd: "⌘",
  Super: "⌘",
  Ctrl: "⌃",
  Control: "⌃",
  Alt: "⌥",
  Option: "⌥",
  Shift: "⇧",
};

/**
 * 把 Tauri 口径的加速键字符串（如 "CmdOrCtrl+Shift+C"）转成面板展示形态（如 "⌘⇧C"）。
 *
 * 逐段映射修饰键后直接拼接：加速键的段序本身就是从修饰键到主键，不需要再排序。
 * 空串或无法识别的段原样保留，读不到配置时退回默认热键，不让展示层抛错。
 */
export function formatAccelerator(accelerator: string): string {
  const trimmed = accelerator.trim();
  if (trimmed === "") return DEFAULT_HOTKEY_LABEL;
  return trimmed
    .split("+")
    .map((part) => MODIFIER_SYMBOLS[part] ?? part)
    .join("");
}

/** 向导根组件：按状态机当前步骤渲染内容，渲染条件由 isGuideVisible 决定。 */
export function OnboardCard({
  state,
  launchOnLogin,
  hotkeyLabel = DEFAULT_HOTKEY_LABEL,
  onAdvance,
  onSkip,
  onDismiss,
  onOpenSystemSettings,
  onLaunchOnLoginChange,
}: OnboardCardProps) {
  if (!isGuideVisible(state)) return null;

  return (
    <div className="onboard" role="region" aria-label="首次引导">
      <div className="grow">
        <StepProgress step={state.step} />
        <StepBody
          step={state.step}
          launchOnLogin={launchOnLogin}
          hotkeyLabel={hotkeyLabel}
          onAdvance={onAdvance}
          onSkip={onSkip}
          onOpenSystemSettings={onOpenSystemSettings}
          onLaunchOnLoginChange={onLaunchOnLoginChange}
        />
      </div>
      <button className="x" type="button" aria-label="关闭" onClick={onDismiss}>
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.9"
          strokeLinecap="round"
          aria-hidden="true"
        >
          <path d="M18 6 6 18M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
}

/** 步骤指示器：四个圆点 + 「第 N / 4 步」。 */
function StepProgress({ step }: { step: GuideStep }) {
  const currentIndex = GUIDE_STEPS.indexOf(step as (typeof GUIDE_STEPS)[number]);
  return (
    <div className="onboard-steps">
      {GUIDE_STEPS.map((each, index) => (
        <i
          key={each}
          className={index <= currentIndex ? "on" : undefined}
          aria-hidden="true"
        />
      ))}
      <span className="onboard-meta">
        {`第 ${Math.max(currentIndex, 0) + 1} / ${GUIDE_STEPS.length} 步`}
      </span>
    </div>
  );
}

interface StepBodyProps {
  step: GuideStep;
  launchOnLogin: boolean;
  hotkeyLabel: string;
  onAdvance: () => void;
  onSkip: () => void;
  onOpenSystemSettings: () => void;
  onLaunchOnLoginChange: (enabled: boolean) => void;
}

/** 按步骤渲染标题、说明与操作按钮。 */
function StepBody({
  step,
  launchOnLogin,
  hotkeyLabel,
  onAdvance,
  onSkip,
  onOpenSystemSettings,
  onLaunchOnLoginChange,
}: StepBodyProps) {
  switch (step) {
    case "welcome":
      return (
        <>
          <h4>欢迎使用 QuickQuick</h4>
          <p>
            三步完成设置：认识热键 → 开启辅助功能（自动粘贴需要它）→ 确认开机自启。
          </p>
          <div className="row">
            <button className="btn btn-primary" type="button" onClick={onAdvance}>
              下一步
            </button>
            {/*
              欢迎页直接给授权入口：已经知道自己要开辅助功能的老用户不必连点两次「下一步」。
              这也是父页面既有的行为契约（首个界面就能一键跳到系统设置）。
            */}
            <button className="btn btn-ghost" type="button" onClick={onOpenSystemSettings}>
              前往系统设置
            </button>
          </div>
        </>
      );
    case "hotkey":
      return (
        <>
          <h4>
            按 <kbd>{hotkeyLabel}</kbd> 随时唤出面板
          </h4>
          <p>
            ↑↓ 选择、Enter 粘贴、⌥Enter 只复制、Esc 关闭；⌘1–9 直达前九条。
          </p>
          <div className="row">
            <button className="btn btn-primary" type="button" onClick={onAdvance}>
              下一步
            </button>
            <button className="btn btn-ghost" type="button" onClick={onSkip}>
              跳过
            </button>
          </div>
        </>
      );
    case "accessibility":
      return (
        <>
          <h4>开启「辅助功能」以启用自动粘贴</h4>
          <p>
            授权后可模拟 ⌘V 自动回写选中条目；未授权时降级为仅复制到剪贴板，功能不中断。
            在系统设置里勾选 QuickQuick 后会立即自动进入下一步。
          </p>
          <div className="row">
            <button className="btn btn-primary" type="button" onClick={onOpenSystemSettings}>
              前往系统设置
            </button>
            <button className="btn btn-ghost" type="button" onClick={onSkip}>
              稍后
            </button>
          </div>
        </>
      );
    case "autostart":
      return (
        <>
          <h4>确认开机自启</h4>
          <p>登录时自动启动 QuickQuick，让热键随时可用；之后可随时在设置里关闭。</p>
          <label className="onboard-check">
            <input
              type="checkbox"
              checked={launchOnLogin}
              onChange={(event) => {
                onLaunchOnLoginChange(event.currentTarget.checked);
              }}
            />
            登录时自动启动 QuickQuick
          </label>
          <div className="row">
            <button className="btn btn-primary" type="button" onClick={onAdvance}>
              完成
            </button>
            <button className="btn btn-ghost" type="button" onClick={onSkip}>
              跳过
            </button>
          </div>
        </>
      );
    default:
      return null;
  }
}
