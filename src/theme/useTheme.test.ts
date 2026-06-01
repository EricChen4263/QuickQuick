import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";

let useThemeModule: typeof import("./useTheme");
let storeModule: typeof import("./themeStore");

beforeEach(async () => {
  vi.resetModules();
  storeModule = await import("./themeStore");
  storeModule._reset();
  useThemeModule = await import("./useTheme");
});

describe("useTheme hook", () => {
  it("初始 pref 与 store 一致（默认 auto）", () => {
    const { result } = renderHook(() => useThemeModule.useTheme());
    expect(result.current.pref).toBe("auto");
  });

  it("setPref('dark') 后 pref 更新为 dark", () => {
    const { result } = renderHook(() => useThemeModule.useTheme());
    act(() => {
      result.current.setPref("dark");
    });
    expect(result.current.pref).toBe("dark");
  });

  it("setPref('light') 后 pref 更新为 light", () => {
    const { result } = renderHook(() => useThemeModule.useTheme());
    act(() => {
      result.current.setPref("light");
    });
    expect(result.current.pref).toBe("light");
  });

  it("外部 store.setPref 触发 hook re-render", () => {
    const { result } = renderHook(() => useThemeModule.useTheme());
    act(() => {
      storeModule.setPref("dark");
    });
    expect(result.current.pref).toBe("dark");
  });
});
