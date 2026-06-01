import { useState, useEffect } from "react";
import { getPref, setPref, subscribe, type ThemePref } from "./themeStore";

export interface UseThemeResult {
  pref: ThemePref;
  setPref: (pref: ThemePref) => void;
}

/**
 * 订阅 themeStore，返回当前 pref 及 setter。
 * re-render 由 store subscribe 驱动，无需轮询。
 */
export function useTheme(): UseThemeResult {
  const [pref, setLocalPref] = useState<ThemePref>(() => getPref());

  useEffect(() => {
    // 挂载时同步一次，防止 SSR/HMR 场景下 init 后 state 过时
    setLocalPref(getPref());
    const unsub = subscribe(() => {
      setLocalPref(getPref());
    });
    return unsub;
  }, []);

  return { pref, setPref };
}
