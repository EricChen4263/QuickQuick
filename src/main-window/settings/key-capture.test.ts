import { describe, it, expect } from "vitest";
import { keyEventToAccelerator } from "./key-capture";

function makeEvent(
  code: string,
  modifiers: { metaKey?: boolean; ctrlKey?: boolean; altKey?: boolean; shiftKey?: boolean } = {},
) {
  return {
    code,
    metaKey: modifiers.metaKey ?? false,
    ctrlKey: modifiers.ctrlKey ?? false,
    altKey: modifiers.altKey ?? false,
    shiftKey: modifiers.shiftKey ?? false,
  };
}

describe("keyEventToAccelerator", () => {
  describe("修饰键映射与固定顺序", () => {
    it("metaKey + 字母键 → 'CmdOrCtrl+A'", () => {
      const result = keyEventToAccelerator(makeEvent("KeyA", { metaKey: true }));
      expect(result).toBe("CmdOrCtrl+A");
    });

    it("ctrlKey + 字母键 → 'Ctrl+B'", () => {
      const result = keyEventToAccelerator(makeEvent("KeyB", { ctrlKey: true }));
      expect(result).toBe("Ctrl+B");
    });

    it("altKey + 字母键 → 'Alt+C'", () => {
      const result = keyEventToAccelerator(makeEvent("KeyC", { altKey: true }));
      expect(result).toBe("Alt+C");
    });

    it("shiftKey + 字母键 → 'Shift+D'", () => {
      const result = keyEventToAccelerator(makeEvent("KeyD", { shiftKey: true }));
      expect(result).toBe("Shift+D");
    });

    it("metaKey + shiftKey + 字母键 → 修饰键顺序为 CmdOrCtrl+Shift", () => {
      const result = keyEventToAccelerator(makeEvent("KeyV", { metaKey: true, shiftKey: true }));
      expect(result).toBe("CmdOrCtrl+Shift+V");
    });

    it("ctrlKey + altKey + 字母键 → 修饰键顺序为 Ctrl+Alt", () => {
      const result = keyEventToAccelerator(makeEvent("KeyX", { ctrlKey: true, altKey: true }));
      expect(result).toBe("Ctrl+Alt+X");
    });

    it("metaKey + ctrlKey + altKey + shiftKey + 字母键 → 全部修饰键按 CmdOrCtrl/Ctrl/Alt/Shift 顺序", () => {
      const result = keyEventToAccelerator(
        makeEvent("KeyZ", { metaKey: true, ctrlKey: true, altKey: true, shiftKey: true }),
      );
      expect(result).toBe("CmdOrCtrl+Ctrl+Alt+Shift+Z");
    });
  });

  describe("主键映射", () => {
    it("KeyA..KeyZ → 去前缀大写字母", () => {
      expect(keyEventToAccelerator(makeEvent("KeyM", { metaKey: true }))).toBe("CmdOrCtrl+M");
      expect(keyEventToAccelerator(makeEvent("KeyZ", { metaKey: true }))).toBe("CmdOrCtrl+Z");
    });

    it("Digit0..Digit9 → 数字字符", () => {
      expect(keyEventToAccelerator(makeEvent("Digit1", { metaKey: true }))).toBe("CmdOrCtrl+1");
      expect(keyEventToAccelerator(makeEvent("Digit0", { metaKey: true }))).toBe("CmdOrCtrl+0");
    });

    it("F1..F12 → 原样", () => {
      expect(keyEventToAccelerator(makeEvent("F1", { metaKey: true }))).toBe("CmdOrCtrl+F1");
      expect(keyEventToAccelerator(makeEvent("F12", { metaKey: true }))).toBe("CmdOrCtrl+F12");
    });

    it("Space → 'Space'", () => {
      expect(keyEventToAccelerator(makeEvent("Space", { metaKey: true }))).toBe("CmdOrCtrl+Space");
    });

    it("Enter → 'Enter'", () => {
      expect(keyEventToAccelerator(makeEvent("Enter", { metaKey: true }))).toBe("CmdOrCtrl+Enter");
    });

    it("Tab → 'Tab'", () => {
      expect(keyEventToAccelerator(makeEvent("Tab", { metaKey: true }))).toBe("CmdOrCtrl+Tab");
    });

    it("Minus → '-'", () => {
      expect(keyEventToAccelerator(makeEvent("Minus", { metaKey: true }))).toBe("CmdOrCtrl+-");
    });

    it("Equal → '='", () => {
      expect(keyEventToAccelerator(makeEvent("Equal", { metaKey: true }))).toBe("CmdOrCtrl+=");
    });

    it("方向键映射 → Up/Down/Left/Right", () => {
      expect(keyEventToAccelerator(makeEvent("ArrowUp", { metaKey: true }))).toBe("CmdOrCtrl+Up");
      expect(keyEventToAccelerator(makeEvent("ArrowDown", { metaKey: true }))).toBe("CmdOrCtrl+Down");
      expect(keyEventToAccelerator(makeEvent("ArrowLeft", { metaKey: true }))).toBe("CmdOrCtrl+Left");
      expect(keyEventToAccelerator(makeEvent("ArrowRight", { metaKey: true }))).toBe("CmdOrCtrl+Right");
    });
  });

  describe("返回 null 的情况", () => {
    it("纯修饰键 code（ShiftLeft）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("ShiftLeft", { shiftKey: true }))).toBeNull();
    });

    it("纯修饰键 code（ShiftRight）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("ShiftRight", { shiftKey: true }))).toBeNull();
    });

    it("纯修饰键 code（MetaLeft）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("MetaLeft", { metaKey: true }))).toBeNull();
    });

    it("纯修饰键 code（MetaRight）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("MetaRight", { metaKey: true }))).toBeNull();
    });

    it("纯修饰键 code（ControlLeft）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("ControlLeft", { ctrlKey: true }))).toBeNull();
    });

    it("纯修饰键 code（ControlRight）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("ControlRight", { ctrlKey: true }))).toBeNull();
    });

    it("纯修饰键 code（AltLeft）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("AltLeft", { altKey: true }))).toBeNull();
    });

    it("纯修饰键 code（AltRight）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("AltRight", { altKey: true }))).toBeNull();
    });

    it("无修饰裸键（字母）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("KeyA"))).toBeNull();
    });

    it("无修饰裸键（数字）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("Digit5"))).toBeNull();
    });

    it("不支持的 code（Backspace）→ null（即便有修饰键）", () => {
      expect(keyEventToAccelerator(makeEvent("Backspace", { metaKey: true }))).toBeNull();
    });

    it("不支持的 code（BracketLeft）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("BracketLeft", { metaKey: true }))).toBeNull();
    });

    it("Escape 键（即使有修饰键）→ null", () => {
      expect(keyEventToAccelerator(makeEvent("Escape", { metaKey: true }))).toBeNull();
    });
  });

  describe("多修饰键组合场景", () => {
    it("CmdOrCtrl+Shift+V（编码器：Meta+Shift+V → 加速键串）", () => {
      const result = keyEventToAccelerator(makeEvent("KeyV", { metaKey: true, shiftKey: true }));
      expect(result).toBe("CmdOrCtrl+Shift+V");
    });

    it("CmdOrCtrl+Shift+T（默认翻译键）", () => {
      const result = keyEventToAccelerator(makeEvent("KeyT", { metaKey: true, shiftKey: true }));
      expect(result).toBe("CmdOrCtrl+Shift+T");
    });

    it("Alt+Shift+F1（功能键 + 两修饰键）", () => {
      const result = keyEventToAccelerator(makeEvent("F1", { altKey: true, shiftKey: true }));
      expect(result).toBe("Alt+Shift+F1");
    });
  });
});
