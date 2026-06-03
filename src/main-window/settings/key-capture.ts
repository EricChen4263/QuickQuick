/** key-capture.ts
 *
 * 从 KeyboardEvent 的原始字段提取 Tauri 全局快捷键加速键串。
 * 纯函数，无副作用，便于单独测试。
 */

const MODIFIER_CODES = new Set([
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
]);

/** 从 e.code 归一出主键名称，不支持则返回 null。 */
function resolveMainKey(code: string): string | null {
  if (/^Key([A-Z])$/.test(code)) {
    return code.slice(3);
  }
  if (/^Digit(\d)$/.test(code)) {
    return code.slice(5);
  }
  if (/^F([1-9]|1[0-2])$/.test(code)) {
    return code;
  }
  const named: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Tab: "Tab",
    Minus: "-",
    Equal: "=",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
  };
  return named[code] ?? null;
}

export interface KeyEventLike {
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  code: string;
}

/**
 * 将 KeyboardEvent（或同结构对象）转换为 Tauri 加速键串。
 *
 * 修饰键固定顺序：CmdOrCtrl → Ctrl → Alt → Shift。
 * 校验：必须「≥1 个修饰键 且 有有效主键」，否则返回 null。
 *
 * @example
 * keyEventToAccelerator({ metaKey: true, shiftKey: true, ctrlKey: false, altKey: false, code: "KeyV" })
 * // => "CmdOrCtrl+Shift+V"
 */
export function keyEventToAccelerator(e: KeyEventLike): string | null {
  if (MODIFIER_CODES.has(e.code)) {
    return null;
  }

  const mainKey = resolveMainKey(e.code);
  if (mainKey === null) {
    return null;
  }

  const modifiers: string[] = [];
  if (e.metaKey) modifiers.push("CmdOrCtrl");
  if (e.ctrlKey) modifiers.push("Ctrl");
  if (e.altKey) modifiers.push("Alt");
  if (e.shiftKey) modifiers.push("Shift");

  if (modifiers.length === 0) {
    return null;
  }

  return [...modifiers, mainKey].join("+");
}
