import { describe, it, expect } from "vitest";
import { topLevelEntries, subViewsOf, resolveNav } from "./nav";

describe("V3-F3-A08 主窗口导航路由", () => {
  describe("topLevelEntries", () => {
    it("一级入口恰为三项", () => {
      const entries = topLevelEntries();
      expect(entries).toHaveLength(3);
    });

    it("一级入口内容为 clipboard / translate / settings", () => {
      const entries = topLevelEntries();
      expect(entries).toEqual(["clipboard", "translate", "settings"]);
    });

    it("一级入口不含 history", () => {
      const entries = topLevelEntries();
      expect(entries).not.toContain("history");
    });
  });

  describe("subViewsOf — 历史均为二级", () => {
    it("clipboard 的二级视图含 history（剪贴板历史二级）", () => {
      const subs = subViewsOf("clipboard");
      expect(subs).toContain("history");
    });

    it("translate 的二级视图含 history（翻译历史二级）", () => {
      const subs = subViewsOf("translate");
      expect(subs).toContain("history");
    });

    it("settings 的二级视图不含 history", () => {
      const subs = subViewsOf("settings");
      expect(subs).not.toContain("history");
    });

    it("clipboard 的二级视图含 list", () => {
      const subs = subViewsOf("clipboard");
      expect(subs).toContain("list");
    });

    it("translate 的二级视图含 workspace", () => {
      const subs = subViewsOf("translate");
      expect(subs).toContain("workspace");
    });
  });

  describe("resolveNav — 路由解析", () => {
    it("默认路由到 clipboard 的默认子视图", () => {
      const state = resolveNav("clipboard");
      expect(state.top).toBe("clipboard");
      expect(state.sub).toBe("list");
    });

    it("translate + history 解析为翻译历史二级", () => {
      const state = resolveNav("translate", "history");
      expect(state.top).toBe("translate");
      expect(state.sub).toBe("history");
    });

    it("clipboard + history 解析为剪贴板历史二级", () => {
      const state = resolveNav("clipboard", "history");
      expect(state.top).toBe("clipboard");
      expect(state.sub).toBe("history");
    });

    it("无效 sub 回退到一级默认子视图", () => {
      const state = resolveNav("clipboard", "nonexistent");
      expect(state.top).toBe("clipboard");
      expect(state.sub).toBe("list");
    });

    it("settings 路由正确", () => {
      const state = resolveNav("settings");
      expect(state.top).toBe("settings");
      expect(state.sub).toBe("general");
    });
  });
});
