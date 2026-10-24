import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { SEARCH_DEBOUNCE_MS, useDebouncedValue } from "./useDebouncedValue";

describe("useDebouncedValue", () => {
  beforeEach(() => {
    // 只伪造定时器：保留真实微任务与 Date，避免 IPC promise / 时间戳判断被牵连
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("默认防抖时长为 200ms", () => {
    expect(SEARCH_DEBOUNCE_MS).toBe(200);
  });

  it("初始值立即返回当前值", () => {
    const { result } = renderHook(() => useDebouncedValue("initial"));
    expect(result.current).toBe("initial");
  });

  it("值变化后 199ms 内不提交，满 200ms 才提交", () => {
    const { result, rerender } = renderHook(({ value }) => useDebouncedValue(value), {
      initialProps: { value: "a" },
    });

    rerender({ value: "ab" });
    act(() => {
      vi.advanceTimersByTime(SEARCH_DEBOUNCE_MS - 1);
    });
    expect(result.current).toBe("a");

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current).toBe("ab");
  });

  it("连续变化只提交最后一次，中间值不落地", () => {
    const { result, rerender } = renderHook(({ value }) => useDebouncedValue(value), {
      initialProps: { value: "" },
    });

    rerender({ value: "h" });
    act(() => {
      vi.advanceTimersByTime(100);
    });
    rerender({ value: "he" });
    act(() => {
      vi.advanceTimersByTime(100);
    });
    rerender({ value: "hel" });
    // 距最后一次变化还不到防抖时长
    expect(result.current).toBe("");

    act(() => {
      vi.advanceTimersByTime(SEARCH_DEBOUNCE_MS);
    });
    expect(result.current).toBe("hel");
  });

  it("delayMs 可注入：非默认时长同样生效", () => {
    const { result, rerender } = renderHook(({ value }) => useDebouncedValue(value, 50), {
      initialProps: { value: "a" },
    });

    rerender({ value: "b" });
    act(() => {
      vi.advanceTimersByTime(49);
    });
    expect(result.current).toBe("a");

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current).toBe("b");
  });

  it("delayMs <= 0 时直通，不延迟", () => {
    const { result, rerender } = renderHook(({ value }) => useDebouncedValue(value, 0), {
      initialProps: { value: "a" },
    });

    rerender({ value: "b" });
    expect(result.current).toBe("b");
  });
});
