/**
 * 外观设置的 React 接入口。
 *
 * 两个 hook 共用一个加载函数：
 * - `useAppearancePreferences`：只读，供弹窗与主窗列表消费（首帧是默认值，
 *   读到后替换；弹窗每次重新打开都会重读，改完设置下一次开面板即生效）；
 * - `useAppearanceSettings`：设置页用，附带四个写回 IPC 的 setter。
 *
 * 读取失败一律沿用当前值（不把界面打回默认），并留下 warn 日志——
 * 外观设置丢了不该让面板白屏，但也不该静默得让人查不出原因。
 */

import { useCallback, useEffect, useRef, useState } from "react";
import {
  getAppearanceSettings,
  setHighlightStyle as ipcSetHighlightStyle,
  setImageHeight as ipcSetImageHeight,
  setPreviewFirstDelay as ipcSetPreviewFirstDelay,
  setSortOrder as ipcSetSortOrder,
  type AppearanceSettings,
  type HighlightStyle,
  type SortOrder,
} from "../../ipc/ipc-client";
import { DEFAULT_APPEARANCE_SETTINGS, normalizeAppearanceSettings } from "./appearance-settings";

/**
 * 只读外观设置。
 *
 * @param reloadKey - 变化时重新读一次；弹窗传入「面板会话序号」，
 *   使每次重新打开都能拿到最新设置（弹窗窗口常驻，组件不会重挂载）
 * @returns 归一后的外观设置；读取失败时是默认值
 */
export function useAppearancePreferences(reloadKey: unknown = null): AppearanceSettings {
  const [settings, setSettings] = useState<AppearanceSettings>(DEFAULT_APPEARANCE_SETTINGS);

  useEffect(() => {
    const cancelled = { current: false };
    getAppearanceSettings()
      .then((raw) => {
        if (cancelled.current) return;
        setSettings(normalizeAppearanceSettings(raw));
      })
      .catch((err: unknown) => {
        console.warn("[useAppearancePreferences] 外观设置读取失败，沿用当前值:", err);
      });
    return () => {
      cancelled.current = true;
    };
  }, [reloadKey]);

  return settings;
}

/** 设置页需要的读写接口。 */
export interface AppearanceSettingsHandle extends AppearanceSettings {
  setSortOrder: (order: SortOrder) => Promise<void>;
  setImageHeightPx: (px: number) => Promise<void>;
  setPreviewFirstDelayMs: (ms: number) => Promise<void>;
  setHighlightStyle: (style: HighlightStyle) => Promise<void>;
}

/**
 * 设置页用的读写外观设置。
 *
 * setter 语义与 `useGeneralSettings` 一致：IPC 成功才更新本地 state；
 * 失败时把错误抛回调用方（设置页据此显示「保存失败」），不假装成功。
 */
export function useAppearanceSettings(): AppearanceSettingsHandle {
  const [settings, setSettings] = useState<AppearanceSettings>(DEFAULT_APPEARANCE_SETTINGS);
  const cancelledRef = useRef(false);

  useEffect(() => {
    cancelledRef.current = false;
    getAppearanceSettings()
      .then((raw) => {
        if (cancelledRef.current) return;
        setSettings(normalizeAppearanceSettings(raw));
      })
      .catch((err: unknown) => {
        console.warn("[useAppearanceSettings] 外观设置读取失败，沿用默认值:", err);
      });
    return () => {
      cancelledRef.current = true;
    };
  }, []);

  const setSortOrder = useCallback(async (order: SortOrder): Promise<void> => {
    await ipcSetSortOrder(order);
    setSettings((prev) => ({ ...prev, sortOrder: order }));
  }, []);

  const setImageHeightPx = useCallback(async (px: number): Promise<void> => {
    await ipcSetImageHeight(px);
    setSettings((prev) => ({ ...prev, imageHeightPx: px }));
  }, []);

  const setPreviewFirstDelayMs = useCallback(async (ms: number): Promise<void> => {
    await ipcSetPreviewFirstDelay(ms);
    setSettings((prev) => ({ ...prev, previewFirstDelayMs: ms }));
  }, []);

  const setHighlightStyle = useCallback(async (style: HighlightStyle): Promise<void> => {
    await ipcSetHighlightStyle(style);
    setSettings((prev) => ({ ...prev, highlightStyle: style }));
  }, []);

  return {
    ...settings,
    setSortOrder,
    setImageHeightPx,
    setPreviewFirstDelayMs,
    setHighlightStyle,
  };
}
