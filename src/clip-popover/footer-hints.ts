/**
 * Popover 底部快捷键徽标的纯逻辑：当前修饰键状态 → 该显示的提示集合。
 *
 * 面板的可用键位是分层的（默认层 + ⌃ / ⌘ / ⌥ 三个组合层），静态 footer 只列默认层的
 * 四条，用户无法发现数字直达、⌃N/⌃P、⌘↑↓、⌃W/U/H 这些键位。徽标改为跟随「此刻按下的
 * 修饰键」切换：按住 ⌃ 就只看到 ⌃ 层真实可用的键，按住时也不打断原有的取用动作。
 */

/** 当前按下的修饰键集合 */
export interface ModifierState {
  shift: boolean;
  ctrl: boolean;
  alt: boolean;
  meta: boolean;
}

/** 一条底部提示：键帽文本 + 说明 */
export interface FooterHint {
  kbd: string;
  label: string;
}

/**
 * 没有按下任何修饰键的状态。
 * 作为共享常量返回同一引用，调用方 setState 传回它时 React 会直接跳过重渲染。
 */
export const NO_MODIFIERS: ModifierState = {
  shift: false,
  ctrl: false,
  alt: false,
  meta: false,
};

/** KeyboardEvent.key 里的修饰键取值 */
const MODIFIER_KEYS = ["Shift", "Control", "Alt", "Meta"] as const;

type ModifierKey = (typeof MODIFIER_KEYS)[number];

function isModifierKey(key: string): key is ModifierKey {
  return (MODIFIER_KEYS as readonly string[]).includes(key);
}

/** 修饰键名 → ModifierState 里的布尔字段 */
function modifierFlag(key: ModifierKey): keyof ModifierState {
  switch (key) {
    case "Shift":
      return "shift";
    case "Control":
      return "ctrl";
    case "Alt":
      return "alt";
    case "Meta":
      return "meta";
  }
}

/**
 * 修饰键按下 / 释放的状态转移。
 *
 * 只认修饰键自身的按键（Shift / Control / Alt / Meta）；普通字符键与方向键不改变状态，
 * 状态本来就没变化时返回同一个 state 引用，避免每次按键都触发一次重渲染。
 *
 * @param state - 当前修饰键状态
 * @param key   - KeyboardEvent.key
 * @param phase - "down" 表示按下，"up" 表示释放
 * @returns 新状态；未发生变化时为原 state
 */
export function applyModifierKey(
  state: ModifierState,
  key: string,
  phase: "down" | "up"
): ModifierState {
  if (!isModifierKey(key)) return state;
  const flag = modifierFlag(key);
  const isPressed = phase === "down";
  if (state[flag] === isPressed) return state;
  return { ...state, [flag]: isPressed };
}

/**
 * 默认层（没有按修饰键）：最常走的取用路径 + 数字直达。
 * 数字直达是 discoverability 缺口的重点——它只能在无修饰键时生效，所以只出现在这里。
 */
const DEFAULT_HINTS: readonly FooterHint[] = [
  { kbd: "↵", label: "粘贴" },
  { kbd: "⌥↵", label: "复制" },
  { kbd: "1–0", label: "直达" },
  { kbd: "↑↓", label: "选择" },
  { kbd: "esc", label: "关闭" },
];

/** ⌃ 层：vim 风格上下移动 + 搜索框文本编辑 */
const CTRL_HINTS: readonly FooterHint[] = [
  { kbd: "⌃N/⌃P", label: "上下" },
  { kbd: "⌃W", label: "删词" },
  { kbd: "⌃H", label: "删字符" },
  { kbd: "⌃U", label: "清空" },
];

/**
 * ⌘ 层：首尾跳转。
 * 取用动作在 ⌘ 下依然可用，一并列出，免得用户以为按着 ⌘ 就粘不了。
 */
const META_HINTS: readonly FooterHint[] = [
  { kbd: "⌘↑↓", label: "首尾" },
  { kbd: "↵", label: "粘贴" },
  { kbd: "⌥↵", label: "复制" },
];

/** ⌥ 层：单按 ⌥ 最容易被当成「没生效」，因此把 ⌥↵ 复制放在最前 */
const ALT_HINTS: readonly FooterHint[] = [
  { kbd: "⌥↵", label: "复制" },
  { kbd: "↑↓", label: "选择" },
];

/**
 * 按当前修饰键选出该显示的徽标集合。
 *
 * 优先级 ⌘ > ⌃ > ⌥：
 * - 同时按住多个修饰键时，面板的按键分发本身也是 ⌘ 优先（`⌘↑↓` 只认 metaKey，
 *   移动逻辑见到 metaKey 直接放行），⌃ 组合会被 ⌘ 组合遮住，所以只有优先级最高的
 *   那一层是「真实可用」的；
 * - ⇧ 没有独立层：面板里 ⇧ 不参与任何专属组合（`⇧↑↓` 仍然只是移动选中项），
 *   给它单列一组会造出并不存在的期望，因此与无修饰键同组。
 *
 * @param modifiers - 当前按下的修饰键
 * @returns 该显示的提示，顺序即展示顺序
 */
export function selectHints(modifiers: ModifierState): readonly FooterHint[] {
  if (modifiers.meta) return META_HINTS;
  if (modifiers.ctrl) return CTRL_HINTS;
  if (modifiers.alt) return ALT_HINTS;
  return DEFAULT_HINTS;
}
