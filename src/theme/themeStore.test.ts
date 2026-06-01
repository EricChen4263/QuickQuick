import { describe, it, expect, beforeEach, vi } from "vitest";

// mock ipc-client，必须在 import themeStore 之前声明
vi.mock("../ipc/ipc-client", () => ({
  getTheme: vi.fn(),
  setTheme: vi.fn(),
}));

import * as ipcClient from "../ipc/ipc-client";

// themeStore 是模块单例，每个测试前必须 _reset() 清理状态防止泄漏
let themeStore: typeof import("./themeStore");

beforeEach(async () => {
  vi.clearAllMocks();
  // getTheme 默认返回 "auto"，避免 hydrate 干扰无关测试
  vi.mocked(ipcClient.getTheme).mockResolvedValue("auto");
  vi.mocked(ipcClient.setTheme).mockResolvedValue(undefined);

  vi.resetModules();
  // 重置后重新 mock，resetModules 会清除 mock 注册
  vi.mock("../ipc/ipc-client", () => ({
    getTheme: vi.fn().mockResolvedValue("auto"),
    setTheme: vi.fn().mockResolvedValue(undefined),
  }));
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
      // 先写 localStorage 再重新 import，确保 init() 读到正确值
      localStorage.setItem("qq-theme-pref", "dark");
      vi.resetModules();
      // getTheme 返回与 localStorage 一致的值，避免 hydrate 覆盖
      vi.doMock("../ipc/ipc-client", () => ({
        getTheme: vi.fn().mockResolvedValue("dark"),
        setTheme: vi.fn().mockResolvedValue(undefined),
      }));
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

  describe("IPC 双轨 — writePref 调用 setTheme", () => {
    it("setPref('dark') 时调用 setTheme('dark')", async () => {
      const { setTheme } = await import("../ipc/ipc-client");
      themeStore.setPref("dark");
      // fire-and-forget，需等 microtask 完成
      await Promise.resolve();
      expect(setTheme).toHaveBeenCalledWith("dark");
    });

    it("setTheme 失败不阻断本地状态更新", async () => {
      const { setTheme } = await import("../ipc/ipc-client");
      vi.mocked(setTheme).mockRejectedValue(new Error("IPC error"));
      themeStore.setPref("light");
      await Promise.resolve();
      // pref 仍已更新
      expect(themeStore.getPref()).toBe("light");
    });
  });

  describe("IPC 双轨 — hydrateFromIpc 竞争防御", () => {
    it("hydrate 完成时 pref 未被手动改则采用 IPC 值", async () => {
      // 在 resetModules 外层先建好 Promise，这样工厂捕获的 resolve 引用可被外层持有
      let resolveGetTheme: ((v: string) => void) | null = null;
      const pendingTheme = new Promise<string>((res) => {
        resolveGetTheme = res;
      });

      vi.resetModules();
      vi.doMock("../ipc/ipc-client", () => ({
        getTheme: vi.fn().mockReturnValue(pendingTheme),
        setTheme: vi.fn().mockResolvedValue(undefined),
      }));
      const fresh = await import("./themeStore");
      fresh._reset();
      // pref 仍是默认 auto，hydrate 尚未返回
      resolveGetTheme!("dark");
      await new Promise((res) => setTimeout(res, 0));
      expect(fresh.getPref()).toBe("dark");
      fresh._reset();
    });

    it("hydrate 期间用户手动改 pref，hydrate 结果被丢弃", async () => {
      let resolveGetTheme: ((v: string) => void) | null = null;
      const pendingTheme = new Promise<string>((res) => {
        resolveGetTheme = res;
      });

      vi.resetModules();
      vi.doMock("../ipc/ipc-client", () => ({
        getTheme: vi.fn().mockReturnValue(pendingTheme),
        setTheme: vi.fn().mockResolvedValue(undefined),
      }));
      const fresh = await import("./themeStore");
      fresh._reset();
      // 用户在 hydrate 返回前手动改 pref
      fresh.setPref("light");
      // hydrate 完成，返回 "dark"
      resolveGetTheme!("dark");
      await new Promise((res) => setTimeout(res, 0));
      // 竞争防御：手动改过了，hydrate 结果应被丢弃
      expect(fresh.getPref()).toBe("light");
      fresh._reset();
    });
  });
});
