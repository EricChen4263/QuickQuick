import { describe, it, expect } from "vitest";
import { settingsSections, addExcludedApp, removeExcludedApp } from "./sections";

describe("V3-F3-A10 设置子项栏", () => {
  describe("settingsSections — 六项纵向", () => {
    it("返回恰好六项", () => {
      const sections = settingsSections();

      expect(sections).toHaveLength(6);
    });

    it("包含 general / hotkey / translate-source / privacy / storage / about", () => {
      const sections = settingsSections();

      expect(sections).toContain("general");
      expect(sections).toContain("hotkey");
      expect(sections).toContain("translate-source");
      expect(sections).toContain("privacy");
      expect(sections).toContain("storage");
      expect(sections).toContain("about");
    });

    it("顺序固定：general 在最前，about 在最后", () => {
      const sections = settingsSections();

      expect(sections[0]).toBe("general");
      expect(sections[5]).toBe("about");
    });
  });

  describe("addExcludedApp — App 排除名单管理", () => {
    it("向空列表添加一个 app，返回含该 app 的新列表", () => {
      const list: string[] = [];

      const result = addExcludedApp(list, "Safari");

      expect(result).toEqual(["Safari"]);
    });

    it("不改变原数组（不可变）", () => {
      const list = ["Safari"];

      addExcludedApp(list, "Chrome");

      expect(list).toEqual(["Safari"]);
    });

    it("重复添加同一 app 去重，不产生重复项", () => {
      const list = ["Safari"];

      const result = addExcludedApp(list, "Safari");

      expect(result).toEqual(["Safari"]);
      expect(result).toHaveLength(1);
      expect(result).not.toBe(list);
    });

    it("添加不同 app 正常追加", () => {
      const list = ["Safari"];

      const result = addExcludedApp(list, "Chrome");

      expect(result).toEqual(["Safari", "Chrome"]);
    });
  });

  describe("removeExcludedApp — App 排除名单移除", () => {
    it("移除存在的 app，返回不含该项的新列表", () => {
      const list = ["Safari", "Chrome"];

      const result = removeExcludedApp(list, "Safari");

      expect(result).toEqual(["Chrome"]);
    });

    it("不改变原数组（不可变）", () => {
      const list = ["Safari", "Chrome"];

      removeExcludedApp(list, "Safari");

      expect(list).toEqual(["Safari", "Chrome"]);
    });

    it("移除不存在的 app，返回与原列表相同内容的新列表", () => {
      const list = ["Safari"];

      const result = removeExcludedApp(list, "Firefox");

      expect(result).toEqual(["Safari"]);
      expect(result).not.toBe(list);
    });

    it("移除后列表为空时返回空数组", () => {
      const list = ["Safari"];

      const result = removeExcludedApp(list, "Safari");

      expect(result).toEqual([]);
    });
  });
});
