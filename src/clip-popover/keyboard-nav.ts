import { moveHighlight } from "../panels/history/keyboard";

/**
 * 根据方向键计算 clip-popover 中下一个选中项的 ID。
 *
 * 复用 moveHighlight 的索引计算逻辑（边界 clamp）。
 * currentId 不在列表或为 null 时，两个方向都选第一项（列表非空时）。
 * 空列表返回 null。
 *
 * @param currentId - 当前选中项 ID（null 表示无选中）
 * @param key       - 方向键
 * @param flatIds   - 当前可见扁平 ID 列表（顺序：收藏 → 今天 → 更早）
 * @returns 下一个选中项 ID，列表为空时返回 null
 */
export function advanceSelection(
  currentId: string | null,
  key: "ArrowUp" | "ArrowDown",
  flatIds: string[],
): string | null {
  if (flatIds.length === 0) {
    return null;
  }

  const currentIndex = currentId !== null ? flatIds.indexOf(currentId) : -1;

  if (currentIndex === -1) {
    return flatIds[0];
  }

  const nextIndex = moveHighlight(currentIndex, key, flatIds.length);
  return flatIds[nextIndex];
}
