import { describe, it, expect } from "vitest";
import {
  applyModifierKey,
  selectHints,
  NO_MODIFIERS,
  type ModifierState,
} from "./footer-hints";

/** 构造修饰键状态：只写需要按下的那几个 */
function modifiers(pressed: Partial<ModifierState> = {}): ModifierState {
  return { ...NO_MODIFIERS, ...pressed };
}

/** 把提示集合压成 "kbd label" 形式的可读断言目标 */
function kbdLabels(state: ModifierState): string[] {
  return selectHints(state).map((hint) => `${hint.kbd} ${hint.label}`);
}

describe("applyModifierKey 修饰键状态转移", () => {
  it("按下 ⌃ 后 ctrl 为 true，其余仍为 false", () => {
    const next = applyModifierKey(NO_MODIFIERS, "Control", "down");
    expect(next).toEqual({ shift: false, ctrl: true, alt: false, meta: false });
  });

  it("释放 ⌃ 后回到全部 false", () => {
    const pressed = applyModifierKey(NO_MODIFIERS, "Control", "down");
    expect(applyModifierKey(pressed, "Control", "up")).toEqual(NO_MODIFIERS);
  });

  it("Shift / Alt / Meta 分别写各自的字段位", () => {
    const shift = applyModifierKey(NO_MODIFIERS, "Shift", "down");
    const alt = applyModifierKey(NO_MODIFIERS, "Alt", "down");
    const meta = applyModifierKey(NO_MODIFIERS, "Meta", "down");
    expect(shift.shift).toBe(true);
    expect(alt.alt).toBe(true);
    expect(meta.meta).toBe(true);
    expect(shift.ctrl || alt.ctrl || meta.ctrl).toBe(false);
  });

  it("普通字符键不改变状态，返回同一引用（避免每次输入都重渲染）", () => {
    expect(applyModifierKey(NO_MODIFIERS, "a", "down")).toBe(NO_MODIFIERS);
    expect(applyModifierKey(NO_MODIFIERS, "ArrowDown", "down")).toBe(NO_MODIFIERS);
    expect(applyModifierKey(NO_MODIFIERS, "Enter", "up")).toBe(NO_MODIFIERS);
  });

  it("状态本来就是这个值时不产生新对象", () => {
    const pressed = applyModifierKey(NO_MODIFIERS, "Control", "down");
    expect(applyModifierKey(pressed, "Control", "down")).toBe(pressed);
    expect(applyModifierKey(NO_MODIFIERS, "Control", "up")).toBe(NO_MODIFIERS);
  });
});

describe("selectHints 徽标集合随修饰键切换", () => {
  it("无修饰键时显示默认层，且包含数字直达", () => {
    const hints = kbdLabels(NO_MODIFIERS);
    expect(hints).toEqual(["↵ 粘贴", "⌥↵ 复制", "1–0 直达", "↑↓ 选择", "esc 关闭"]);
    // 数字直达是 discoverability 的主要缺口，默认层必须能看见
    expect(hints.some((hint) => hint.includes("直达"))).toBe(true);
  });

  it("按住 ⌃ 时只显示 ⌃ 层：上下移动与三个文本编辑键", () => {
    expect(kbdLabels(modifiers({ ctrl: true }))).toEqual([
      "⌃N/⌃P 上下",
      "⌃W 删词",
      "⌃H 删字符",
      "⌃U 清空",
    ]);
  });

  it("按住 ⌘ 时显示首尾跳转 + 仍可用的取用动作", () => {
    expect(kbdLabels(modifiers({ meta: true }))).toEqual(["⌘↑↓ 首尾", "↵ 粘贴", "⌥↵ 复制"]);
  });

  it("按住 ⌥ 时把 ⌥↵ 复制放最前，避免误导", () => {
    expect(kbdLabels(modifiers({ alt: true }))).toEqual(["⌥↵ 复制", "↑↓ 选择"]);
  });

  it("单按 ⇧ 没有专属键位，仍显示默认层（不造出不存在的期望）", () => {
    expect(kbdLabels(modifiers({ shift: true }))).toEqual(kbdLabels(NO_MODIFIERS));
  });

  it("⌃ + ⌘ 同时按下时 ⌘ 优先：只显示 ⌘ 层", () => {
    expect(kbdLabels(modifiers({ ctrl: true, meta: true }))).toEqual(kbdLabels(modifiers({ meta: true })));
  });

  it("⌃ + ⌥ 同时按下时 ⌃ 优先：只显示 ⌃ 层", () => {
    expect(kbdLabels(modifiers({ ctrl: true, alt: true }))).toEqual(kbdLabels(modifiers({ ctrl: true })));
  });
});
