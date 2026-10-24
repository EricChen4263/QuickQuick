import { useEffect, useRef, useState } from "react";
import { previewDelayMs } from "./preview-delay";

/**
 * 把「当前选中条目」推迟成「此刻该显示的预览条目」。
 *
 * 行为约定：
 * - 面板每次打开（`sessionKey` 变化）都从「没有预览」重新开始，首个预览重新等 1500ms；
 * - 首个预览出现后，切换条目等 200ms：快速连按时每个中间条目都会把自己那个定时器
 *   清掉重排，只有停在某一条满 200ms 才会真正显示，中间项不会闪；
 * - 目标条目消失（例如搜索把列表过滤空）时立即清空——没有「再看一眼」的意义；
 * - 定时器随依赖变化与组件卸载一起清理，不对已卸载组件 setState。
 *
 * @param targetId   - 当前选中条目 ID，null 表示没有选中项
 * @param sessionKey - 面板会话标识：每次重新打开面板递增，用于重置两档延迟
 * @returns 该显示预览的条目 ID；null 表示此刻不显示预览内容
 */
export function useDeferredPreviewId(
  targetId: string | null,
  sessionKey: number
): string | null {
  const [displayedId, setDisplayedId] = useState<string | null>(null);
  // 本次面板会话是否已经出现过预览：决定下一次走 1500ms 还是 200ms 档
  const hasShownPreviewRef = useRef(false);

  // 新的面板会话：立刻清空预览并重置档位，让首个预览重新走 1500ms
  useEffect(() => {
    hasShownPreviewRef.current = false;
    setDisplayedId(null);
  }, [sessionKey]);

  useEffect(() => {
    // 已经在显示这一条（或两边都是「不显示」），无需重排定时器
    if (targetId === displayedId) return;
    if (targetId === null) {
      setDisplayedId(null);
      return;
    }
    const timer = setTimeout(() => {
      hasShownPreviewRef.current = true;
      setDisplayedId(targetId);
    }, previewDelayMs(!hasShownPreviewRef.current));
    return () => {
      clearTimeout(timer);
    };
  }, [targetId, displayedId]);

  return displayedId;
}
