import { useState, useEffect, useRef, useCallback } from "react";
import {
  getLaunchOnLogin,
  setLaunchOnLogin as ipcSetLaunchOnLogin,
  getStayInTray,
  setStayInTray as ipcSetStayInTray,
  getAutoUpdate,
  setAutoUpdate as ipcSetAutoUpdate,
} from "../../ipc/ipc-client";

interface GeneralSettings {
  launchOnLogin: boolean;
  stayInTray: boolean;
  autoUpdate: boolean;
  setLaunchOnLogin: (v: boolean) => Promise<void>;
  setStayInTray: (v: boolean) => Promise<void>;
  setAutoUpdate: (v: boolean) => Promise<void>;
}

/**
 * 通用设置 hook，从 IPC 读取并写回 settings.json。
 *
 * 设计决策：
 * - mount 时 Promise.all 并行拉取三项，避免串行延迟。
 * - cancelled ref 防卸载后写 state（React 并发模式下 StrictMode 也可能触发）。
 * - setter：IPC 成功后更新本地 state；IPC 失败则不更新（保持原值）。
 * - 对外接口形状不变，GeneralPanel 零改动。
 */
export function useGeneralSettings(): GeneralSettings {
  const [launchOnLogin, setLaunchOnLoginState] = useState(true);
  const [stayInTray, setStayInTrayState] = useState(true);
  const [autoUpdate, setAutoUpdateState] = useState(true);
  const cancelledRef = useRef(false);

  useEffect(() => {
    cancelledRef.current = false;

    Promise.all([getLaunchOnLogin(), getStayInTray(), getAutoUpdate()])
      .then(([launch, tray, update]) => {
        if (cancelledRef.current) return;
        setLaunchOnLoginState(launch);
        setStayInTrayState(tray);
        setAutoUpdateState(update);
      })
      .catch(() => {
        console.warn("[useGeneralSettings] IPC init failed, using defaults");
      });

    return () => {
      cancelledRef.current = true;
    };
  }, []);

  const setLaunchOnLogin = useCallback(async (v: boolean): Promise<void> => {
    try {
      await ipcSetLaunchOnLogin(v);
      setLaunchOnLoginState(v);
    } catch {
      console.warn("[useGeneralSettings] setLaunchOnLogin IPC failed");
    }
  }, []);

  const setStayInTray = useCallback(async (v: boolean): Promise<void> => {
    try {
      await ipcSetStayInTray(v);
      setStayInTrayState(v);
    } catch {
      console.warn("[useGeneralSettings] setStayInTray IPC failed");
    }
  }, []);

  const setAutoUpdate = useCallback(async (v: boolean): Promise<void> => {
    try {
      await ipcSetAutoUpdate(v);
      setAutoUpdateState(v);
    } catch {
      console.warn("[useGeneralSettings] setAutoUpdate IPC failed");
    }
  }, []);

  return {
    launchOnLogin,
    stayInTray,
    autoUpdate,
    setLaunchOnLogin,
    setStayInTray,
    setAutoUpdate,
  };
}
