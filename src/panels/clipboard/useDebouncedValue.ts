/**
 * 搜索输入防抖 hook。
 *
 * 只防抖「用于过滤的搜索词」，输入框自身仍是即时受控值：
 * 打字反馈（含输入法组词）不受延迟影响，只有结果集刷新被推迟。
 */

import { useEffect, useState } from "react";

/** 搜索防抖时长（毫秒）：略高于一次连续击键的间隔，又不至于让人觉得卡顿 */
export const SEARCH_DEBOUNCE_MS = 200;

/**
 * 值防抖：value 连续变化时只在它停止变化 delayMs 之后提交最新值。
 *
 * @param value - 原始值（每次变化都会重置计时）
 * @param delayMs - 防抖时长，可注入便于假定时器测试；<= 0 时退化为直通
 * @returns 防抖后的值
 */
export function useDebouncedValue<T>(value: T, delayMs: number = SEARCH_DEBOUNCE_MS): T {
  const [debounced, setDebounced] = useState<T>(value);

  useEffect(() => {
    if (delayMs <= 0) {
      setDebounced(value);
      return;
    }
    const timer = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(timer);
  }, [value, delayMs]);

  return debounced;
}
