import { describe, it, expect, beforeEach, vi } from "vitest";

// themeStore 是模块单例，每个测试前必须 _reset() 清理状态防止泄漏
let themeStore: typeof import("./themeStore");

beforeEach(async () => {
  // 重置模块以清除单例状态
  vi.resetModules();
  themeStore = await import("./themeStore");
  themeStore._reset();
});

describe("themeStore — 主题偏好单例", () => {
  describe("初始状态", () => {
    it("默认 pref 为 auto", () => {
      expect(themeStore.getPref()).toBe("auto");
    });

    it("auto 模式下 resolved 跟随系统（jsdom 无 matchMedia，回退 light）", () => {
      const resolved = themeStore.getResolved();
      expect(resolved === "light" || resolved === "dark").toBe(true);
    });
  });

  describe("setPref — 手动切换", () => {
    it("setPref('light') 后 getPref() 返回 light", () => {
      themeStore.setPref("light");
      expect(themeStore.getPref()).toBe("light");
    });

    it("setPref('dark') 后 getPref() 返回 dark", () => {
      themeStore.setPref("dark");
      expect(themeStore.getPref()).toBe("dark");
    });

    it("setPref('light') 后 getResolved() 返回 light", () => {
      themeStore.setPref("light");
      expect(themeStore.getResolved()).toBe("light");
    });

    it("setPref('dark') 后 getResolved() 返回 dark", () => {
      themeStore.setPref("dark");
      expect(themeStore.getResolved()).toBe("dark");
    });

    it("setPref('auto') 后 getPref() 返回 auto", () => {
      themeStore.setPref("dark");
      themeStore.setPref("auto");
      expect(themeStore.getPref()).toBe("auto");
    });
  });

  describe("document.documentElement.dataset.theme 写入", () => {
    it("setPref('dark') 后 dataset.theme 为 dark", () => {
      themeStore.setPref("dark");
      expect(document.documentElement.dataset["theme"]).toBe("dark");
    });

    it("setPref('light') 后 dataset.theme 为 light", () => {
      themeStore.setPref("light");
      expect(document.documentElement.dataset["theme"]).toBe("light");
    });
  });

  describe("localStorage 持久化", () => {
    it("setPref('dark') 写入 localStorage qq-theme-pref", () => {
      themeStore.setPref("dark");
      expect(localStorage.getItem("qq-theme-pref")).toBe("dark");
    });

    it("_reset 后重新 import 从 localStorage 恢复 pref", async () => {
      localStorage.setItem("qq-theme-pref", "dark");
      vi.resetModules();
      const fresh = await import("./themeStore");
      expect(fresh.getPref()).toBe("dark");
      fresh._reset();
    });
  });

  describe("subscribe — 监听变化", () => {
    it("setPref 触发 listener 回调", () => {
      const listener = vi.fn();
      const unsub = themeStore.subscribe(listener);
      themeStore.setPref("dark");
      expect(listener).toHaveBeenCalledTimes(1);
      unsub();
    });

    it("unsubscribe 后不再触发", () => {
      const listener = vi.fn();
      const unsub = themeStore.subscribe(listener);
      unsub();
      themeStore.setPref("dark");
      expect(listener).not.toHaveBeenCalled();
    });

    it("多个 listener 各自独立回调", () => {
      const a = vi.fn();
      const b = vi.fn();
      const unsubA = themeStore.subscribe(a);
      const unsubB = themeStore.subscribe(b);
      themeStore.setPref("light");
      expect(a).toHaveBeenCalledTimes(1);
      expect(b).toHaveBeenCalledTimes(1);
      unsubA();
      unsubB();
    });
  });
});
