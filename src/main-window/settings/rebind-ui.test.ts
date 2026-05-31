import { describe, it, expect } from "vitest";
import { validateRebind } from "./rebind";

describe("V3-F3-A09 改键实时校验", () => {
  describe("validateRebind — 占用键拒绝", () => {
    it("新快捷键已在占用列表中，返回 ok:false 且 error 为'已被占用'", () => {
      const occupied = ["CmdOrCtrl+C", "CmdOrCtrl+V"];

      const result = validateRebind("CmdOrCtrl+C", occupied);

      expect(result).toMatchObject({ ok: false, error: "已被占用" });
    });

    it("占用列表含多项时，任意命中均返回 ok:false error:'已被占用'", () => {
      const occupied = ["CmdOrCtrl+C", "CmdOrCtrl+V", "CmdOrCtrl+X"];

      const result = validateRebind("CmdOrCtrl+X", occupied);

      expect(result).toMatchObject({ ok: false, error: "已被占用" });
    });
  });

  describe("validateRebind — 空闲键通过", () => {
    it("新快捷键不在占用列表中，返回 ok:true 且携带 accelerator", () => {
      const occupied = ["CmdOrCtrl+C", "CmdOrCtrl+V"];

      const result = validateRebind("CmdOrCtrl+Shift+H", occupied);

      expect(result.ok).toBe(true);
      if (result.ok) {
        expect(result.accelerator).toBe("CmdOrCtrl+Shift+H");
      }
    });

    it("占用列表为空时，任何键均通过", () => {
      const result = validateRebind("CmdOrCtrl+Q", []);

      expect(result.ok).toBe(true);
      if (result.ok) {
        expect(result.accelerator).toBe("CmdOrCtrl+Q");
      }
    });

    it("大小写不同视为不同键（空闲）", () => {
      const occupied = ["cmdorctrl+c"];

      const result = validateRebind("CmdOrCtrl+C", occupied);

      expect(result.ok).toBe(true);
      if (result.ok) {
        expect(result.accelerator).toBe("CmdOrCtrl+C");
      }
    });
  });
});
