import { useState, useEffect, useRef, useCallback } from "react";
import {
  getLaunchOnLogin,
  setLaunchOnLogin as ipcSetLaunchOnLogin,
  getStayInTray,
  setStayInTray as ipcSetStayInTray,
  getAutoUpdate,
  setAutoUpdate as ipcSetAutoUpdate,
} from "../../ipc/ipc-client";

/** 通用面板里可能失败的开关（用作错误来源标识，与页面级错误条按来源隔离配合）。 */
export type GeneralSaveSource = "launchOnLogin" | "stayInTray" | "autoUpdate";


interface GeneralSettings {
  launchOnLogin: boolean;
  stayInTray: boolean;
  autoUpdate: boolean;
  /**
   * **按开关分别记录**的保存失败（来源 → 后端错误原文）；缺项表示该开关没有未处理的失败。
   *
   * 必须按来源分开：三个开关共用一个槽位时，A 开关失败后 B 开关成功会把 A 的失败原因
   * 一起清掉——用户只会看到错误闪一下。设置写失败必须能被看到：settings.json 损坏时
   * 后端会拒写并给出修复指引，只 console.warn 等于让开关「点了没反应」。
   */
  saveErrors: Partial<Record<GeneralSaveSource, string>>;
  setLaunchOnLogin: (v: boolean) => Promise<void>;
  setStayInTray: (v: boolean) => Promise<void>;
  setAutoUpdate: (v: boolean) => Promise<void>;
}

/** 把 IPC reject 值转成可展示的文案（后端返回的是中文 String）。 */
function toErrorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
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
  const [saveErrors, setSaveErrors] = useState<Partial<Record<GeneralSaveSource, string>>>({});
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

  /** 记录某个开关的保存失败（只覆盖它自己那一条）。 */
  const setSourceError = useCallback((source: GeneralSaveSource, message: string): void => {
    setSaveErrors((prev) => ({ ...prev, [source]: message }));
  }, []);

  /** 清除某个开关的保存失败（同来源成功时调用，不动别的开关的错误）。 */
  const clearSourceError = useCallback((source: GeneralSaveSource): void => {
    setSaveErrors((prev) => {
      if (!(source in prev)) return prev;
      const next = { ...prev };
      delete next[source];
      return next;
    });
  }, []);

  const setLaunchOnLogin = useCallback(async (v: boolean): Promise<void> => {
    try {
      await ipcSetLaunchOnLogin(v);
      setLaunchOnLoginState(v);
      clearSourceError("launchOnLogin");
    } catch (err) {
      setSourceError("launchOnLogin", toErrorMessage(err));
    }
  }, []);

  const setStayInTray = useCallback(async (v: boolean): Promise<void> => {
    try {
      await ipcSetStayInTray(v);
      setStayInTrayState(v);
      clearSourceError("stayInTray");
    } catch (err) {
      setSourceError("stayInTray", toErrorMessage(err));
    }
  }, []);

  const setAutoUpdate = useCallback(async (v: boolean): Promise<void> => {
    try {
      await ipcSetAutoUpdate(v);
      setAutoUpdateState(v);
      clearSourceError("autoUpdate");
    } catch (err) {
      setSourceError("autoUpdate", toErrorMessage(err));
    }
  }, []);

  return {
    launchOnLogin,
    stayInTray,
    autoUpdate,
    saveErrors,
    setLaunchOnLogin,
    setStayInTray,
    setAutoUpdate,
  };
}
