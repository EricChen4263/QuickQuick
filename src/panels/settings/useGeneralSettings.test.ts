import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useGeneralSettings } from "./useGeneralSettings";

describe("useGeneralSettings", () => {
  it("初始值三项均为 true", () => {
    const { result } = renderHook(() => useGeneralSettings());
    expect(result.current.launchOnLogin).toBe(true);
    expect(result.current.stayInTray).toBe(true);
    expect(result.current.autoUpdate).toBe(true);
  });

  it("setLaunchOnLogin(false) 后 launchOnLogin 变为 false", () => {
    const { result } = renderHook(() => useGeneralSettings());
    act(() => {
      result.current.setLaunchOnLogin(false);
    });
    expect(result.current.launchOnLogin).toBe(false);
    expect(result.current.stayInTray).toBe(true);
    expect(result.current.autoUpdate).toBe(true);
  });

  it("setStayInTray(false) 后 stayInTray 变为 false，其余不变", () => {
    const { result } = renderHook(() => useGeneralSettings());
    act(() => {
      result.current.setStayInTray(false);
    });
    expect(result.current.stayInTray).toBe(false);
    expect(result.current.launchOnLogin).toBe(true);
  });

  it("setAutoUpdate(false) 后 autoUpdate 变为 false，其余不变", () => {
    const { result } = renderHook(() => useGeneralSettings());
    act(() => {
      result.current.setAutoUpdate(false);
    });
    expect(result.current.autoUpdate).toBe(false);
    expect(result.current.launchOnLogin).toBe(true);
  });

  it("可以独立对每项反复 toggle", () => {
    const { result } = renderHook(() => useGeneralSettings());
    act(() => { result.current.setLaunchOnLogin(false); });
    act(() => { result.current.setLaunchOnLogin(true); });
    expect(result.current.launchOnLogin).toBe(true);
  });
});
