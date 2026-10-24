import type { ClipItem } from "../ipc/ipc-client";

/**
 * 预览区当前该呈现的状态（三态互斥）。
 * - `item`：有条目可看；
 * - `waiting`：已选中条目，但分级延迟还没到点；
 * - `empty`：真的没有选中项（列表为空）。
 */
export type PreviewState =
  | { kind: "item"; item: ClipItem }
  | { kind: "waiting" }
  | { kind: "empty" };

/**
 * 分级延迟窗口内的中性占位文案。
 * 不写「无选中项」——左侧列表里明明有高亮行，那句话与事实相反；
 * 也不写「加载中」——本地数据本来就没有加载过程。
 */
export const PREVIEW_WAITING_TEXT = "预览稍后显示";

/**
 * 由「延迟后该显示的条目」与「当前是否真有选中项」决定预览区三态。
 *
 * 重点是区分两种「都还没有内容可渲染」的情形：
 * - 有选中项、只是延迟未到点 → `waiting`（渲染中性占位）；
 * - 压根没有选中项（列表为空） → `empty`（渲染「无选中项」）。
 * 两者不能共用文案，否则会出现「列表里有高亮行、预览却说无选中项」的自相矛盾。
 *
 * @param displayedItem - 分级延迟到点后该显示的条目；null 表示此刻还不到点
 * @param hasSelection  - 当前是否存在选中项
 * @returns 预览区三态之一
 */
export function resolvePreviewState(
  displayedItem: ClipItem | null,
  hasSelection: boolean
): PreviewState {
  if (displayedItem !== null) {
    return { kind: "item", item: displayedItem };
  }
  return hasSelection ? { kind: "waiting" } : { kind: "empty" };
}
