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

/** 选中项当前由谁驱动：键盘导航期间归 keyboard，真实 pointer 位移之后归 pointer */
type InputOwner = "keyboard" | "pointer";

/** pointer 事件的屏幕坐标，只用于判断「这一次移动是不是真的动了」 */
interface PointerPosition {
  x: number;
  y: number;
}

/**
 * 悬停与键盘的输入主导权仲裁器。
 *
 * 要解决的问题：键盘翻页或列表滚动时，静止的鼠标会被浏览器「送到」另一行下方——
 * 滚动结束后浏览器会补发 mouseover / mouseenter / mousemove（坐标与滚动前完全相同），
 * 若直接按悬停改选中，键盘刚选中的行就被鼠标抢走。
 *
 * 因此把「谁主导选中项」显式建模：
 *   - 悬停改选中必须满足 {@link InputOwnership.isPointerOwner}；
 *   - 只有坐标真的变化过的 pointer 移动才把主导权交给鼠标（滚动补发的同坐标 mousemove 不算）；
 *   - 键盘导航调用 {@link InputOwnership.handToKeyboard} 立即收回主导权。
 */
export interface InputOwnership {
  /** 键盘导航：立即把主导权交还键盘，此后悬停不得改选中项 */
  handToKeyboard(): void;
  /** 当前是否由鼠标主导（悬停可否改选中项） */
  isPointerOwner(): boolean;
  /**
   * 记录一次 pointer 移动。
   *
   * @returns 坐标相对上一次记录是否变化，即本次移动是否构成「真实位移」；
   *          只有 true 时主导权才交给鼠标。
   */
  observePointerMove(x: number, y: number): boolean;
}

/**
 * 创建一个初始由键盘主导的仲裁器。
 *
 * 面板打开或键盘导航之后，选中项由键盘决定；此时鼠标哪怕恰好停在某一行上也不该抢走它。
 */
export function createInputOwnership(): InputOwnership {
  let owner: InputOwner = "keyboard";
  // 上一次记录到的 pointer 坐标：只比坐标，不依赖事件时序
  let lastPosition: PointerPosition | null = null;

  return {
    handToKeyboard(): void {
      owner = "keyboard";
      // 刻意保留 lastPosition：滚动会把同一个坐标重发一次 mousemove，
      // 若在这里清空，「滚动补发 → 坐标算变化 → 交还鼠标」的抢选中又回来了。
    },
    isPointerOwner(): boolean {
      return owner === "pointer";
    },
    observePointerMove(x: number, y: number): boolean {
      if (lastPosition !== null && lastPosition.x === x && lastPosition.y === y) {
        return false;
      }
      lastPosition = { x, y };
      owner = "pointer";
      return true;
    },
  };
}
