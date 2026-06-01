import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

vi.mock("../../ipc/ipc-client", () => ({
  getLaunchOnLogin: vi.fn(),
  setLaunchOnLogin: vi.fn(),
  getStayInTray: vi.fn(),
  setStayInTray: vi.fn(),
  getAutoUpdate: vi.fn(),
  setAutoUpdate: vi.fn(),
}));

import {
  getLaunchOnLogin,
  setLaunchOnLogin,
  getStayInTray,
  setStayInTray,
  getAutoUpdate,
  setAutoUpdate,
} from "../../ipc/ipc-client";
import { useGeneralSettings } from "./useGeneralSettings";

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getLaunchOnLogin).mockResolvedValue(true);
  vi.mocked(getStayInTray).mockResolvedValue(true);
  vi.mocked(getAutoUpdate).mockResolvedValue(true);
  vi.mocked(setLaunchOnLogin).mockResolvedValue(undefined);
  vi.mocked(setStayInTray).mockResolvedValue(undefined);
  vi.mocked(setAutoUpdate).mockResolvedValue(undefined);
});

describe("useGeneralSettings", () => {
  describe("mount 初始化", () => {
    it("mount 时并行调用三个 IPC getter", async () => {
      renderHook(() => useGeneralSettings());
      await waitFor(() => {
        expect(getLaunchOnLogin).toHaveBeenCalledTimes(1);
        expect(getStayInTray).toHaveBeenCalledTimes(1);
        expect(getAutoUpdate).toHaveBeenCalledTimes(1);
      });
    });

    it("IPC 返回 true 时三项值均为 true", async () => {
      const { result } = renderHook(() => useGeneralSettings());
      await waitFor(() => {
        expect(result.current.launchOnLogin).toBe(true);
        expect(result.current.stayInTray).toBe(true);
        expect(result.current.autoUpdate).toBe(true);
      });
    });

    it("IPC 返回 false 时三项值均为 false", async () => {
      vi.mocked(getLaunchOnLogin).mockResolvedValue(false);
      vi.mocked(getStayInTray).mockResolvedValue(false);
      vi.mocked(getAutoUpdate).mockResolvedValue(false);
      const { result } = renderHook(() => useGeneralSettings());
      await waitFor(() => {
        expect(result.current.launchOnLogin).toBe(false);
        expect(result.current.stayInTray).toBe(false);
        expect(result.current.autoUpdate).toBe(false);
      });
    });

    it("IPC 失败时保留默认值 true，不崩溃", async () => {
      vi.mocked(getLaunchOnLogin).mockRejectedValue(new Error("IPC error"));
      vi.mocked(getStayInTray).mockRejectedValue(new Error("IPC error"));
      vi.mocked(getAutoUpdate).mockRejectedValue(new Error("IPC error"));
      const { result } = renderHook(() => useGeneralSettings());
      // 等待足够时间让 Promise settle
      await new Promise((res) => setTimeout(res, 10));
      expect(result.current.launchOnLogin).toBe(true);
      expect(result.current.stayInTray).toBe(true);
      expect(result.current.autoUpdate).toBe(true);
    });
  });

  describe("setLaunchOnLogin setter", () => {
    it("调用 IPC setLaunchOnLogin(false) 成功后更新本地 state", async () => {
      const { result } = renderHook(() => useGeneralSettings());
      await waitFor(() => expect(result.current.launchOnLogin).toBe(true));

      await act(async () => {
        await result.current.setLaunchOnLogin(false);
      });

      expect(setLaunchOnLogin).toHaveBeenCalledWith(false);
      expect(result.current.launchOnLogin).toBe(false);
    });

    it("IPC setLaunchOnLogin 失败时不更新 state", async () => {
      vi.mocked(setLaunchOnLogin).mockRejectedValue(new Error("IPC error"));
      const { result } = renderHook(() => useGeneralSettings());
      await waitFor(() => expect(result.current.launchOnLogin).toBe(true));

      await act(async () => {
        await result.current.setLaunchOnLogin(false);
      });

      expect(result.current.launchOnLogin).toBe(true);
    });
  });

  describe("setStayInTray setter", () => {
    it("调用 IPC setStayInTray(false) 成功后更新本地 state", async () => {
      const { result } = renderHook(() => useGeneralSettings());
      await waitFor(() => expect(result.current.stayInTray).toBe(true));

      await act(async () => {
        await result.current.setStayInTray(false);
      });

      expect(setStayInTray).toHaveBeenCalledWith(false);
      expect(result.current.stayInTray).toBe(false);
    });

    it("IPC setStayInTray 失败时不更新 state", async () => {
      vi.mocked(setStayInTray).mockRejectedValue(new Error("IPC error"));
      const { result } = renderHook(() => useGeneralSettings());
      await waitFor(() => expect(result.current.stayInTray).toBe(true));

      await act(async () => {
        await result.current.setStayInTray(false);
      });

      expect(result.current.stayInTray).toBe(true);
    });
  });

  describe("setAutoUpdate setter", () => {
    it("调用 IPC setAutoUpdate(false) 成功后更新本地 state", async () => {
      const { result } = renderHook(() => useGeneralSettings());
      await waitFor(() => expect(result.current.autoUpdate).toBe(true));

      await act(async () => {
        await result.current.setAutoUpdate(false);
      });

      expect(setAutoUpdate).toHaveBeenCalledWith(false);
      expect(result.current.autoUpdate).toBe(false);
    });

    it("IPC setAutoUpdate 失败时不更新 state", async () => {
      vi.mocked(setAutoUpdate).mockRejectedValue(new Error("IPC error"));
      const { result } = renderHook(() => useGeneralSettings());
      await waitFor(() => expect(result.current.autoUpdate).toBe(true));

      await act(async () => {
        await result.current.setAutoUpdate(false);
      });

      expect(result.current.autoUpdate).toBe(true);
    });
  });

  describe("卸载防护", () => {
    it("组件卸载后 IPC 返回不写 state（无 act 警告）", async () => {
      let resolveAll!: () => void;
      const pending = new Promise<void>((res) => {
        resolveAll = res;
      });
      vi.mocked(getLaunchOnLogin).mockReturnValue(
        pending.then(() => true)
      );
      vi.mocked(getStayInTray).mockReturnValue(
        pending.then(() => true)
      );
      vi.mocked(getAutoUpdate).mockReturnValue(
        pending.then(() => true)
      );

      const { unmount } = renderHook(() => useGeneralSettings());
      unmount();
      // IPC 在卸载后 resolve，不应触发 state 写入（无 act 警告）
      resolveAll();
      await new Promise((res) => setTimeout(res, 10));
    });
  });
});
