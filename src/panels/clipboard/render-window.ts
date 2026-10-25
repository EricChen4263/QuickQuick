/**
 * 列表增量渲染的窗口计算（纯逻辑，无 DOM 副作用）。
 *
 * 为什么是「增量渲染」而不是固定行高的窗口化：`.clip-text` 是两行截断，
 * 行高随内容在「一行摘要」与「两行摘要」之间变化，固定行高虚拟滚动必须假定
 * 等行高，只能靠强制固定高度对账，会改动既有视觉。这里只控制「渲染多少条」，
 * 行高始终由内容决定，离屏行的开销靠「根本不进 DOM」来省，而不是靠 CSS 跳过。
 *
 * 搜索与类型筛选依旧在内存里对全量做匹配，窗口只影响 DOM 节点数，
 * 不影响键盘导航的可达范围（选择是 id/索引态，遍历的始终是完整过滤结果）。
 */

/**
 * 首屏渲染条数，同时也是每次按块追加的块大小。
 * 100 条足以填满主窗视口并留出滚动余量，同时把 DOM 行数压到两位数百分比。
 */
export const RENDER_BLOCK_SIZE = 100;

/**
 * 滚动位置距底部的距离阈值（px）。
 * 进入该范围即认定「接近底部」，提前追加一块，避免用户滚到底还要再滚一次才有内容。
 */
export const NEAR_BOTTOM_THRESHOLD_PX = 240;

/**
 * 把窗口大小收敛到 [0, total]；**total 为 0 时保留窗口，不清零**。
 *
 * 窗口是「渲染上限」，不是「当前条数」：数据还没取回（或过滤后暂时为空）时把它压成 0，
 * 会让「数据到达的第一个提交」渲染 0 行，下一提交才由 effect 补齐——多一次 0→N 的中间态，
 * 任何「等列表出现某条文案」的判断都可能落在那一帧（P4-2 登记的 flake 根因）。
 * 只保证非负：负值仍收敛到 0，避免向上游传播。
 */
function clampToTotal(count: number, total: number): number {
  const nonNegative = Math.max(count, 0);
  if (total <= 0) return nonNegative;
  return Math.min(nonNegative, total);
}

/**
 * 初始渲染窗口：取一块，总数不足一块时全渲染。
 *
 * 空数据（total = 0）时仍返回一块，而不是 0——窗口是上限，数据为空不代表上限该是 0；
 * 这样过滤到空再清空时不需要额外一次「窗口从 0 涨回来」的提交。
 *
 * @param total - 当前过滤结果总数
 * @returns 首屏应渲染的条数
 */
export function initialRenderCount(total: number): number {
  return clampToTotal(RENDER_BLOCK_SIZE, total);
}

/**
 * 滚动接近底部时的窗口增长：追加一块，不超过总数。
 *
 * @param current - 当前窗口大小
 * @param total   - 当前过滤结果总数
 * @returns 追加一块后的窗口大小
 */
export function growRenderCount(current: number, total: number): number {
  // 空数据没有「可追加的下一块」：保持不变（也不要清零）。
  if (total <= 0) return Math.max(current, 0);
  return clampToTotal(current + RENDER_BLOCK_SIZE, total);
}

/**
 * 选中项落在窗口之外时，把窗口扩到能包含它。
 *
 * 键盘导航作用在完整过滤结果上，选中项可能一步跳到未渲染的尾部
 * （⌘↓ 跳末条、连按 ↓、搜索后回落等）。窗口不跟随扩张的话，高亮行不在 DOM 里，
 * 表现为「高亮消失且滚不过去」。
 *
 * 扩张按块对齐：窗口永远停在块边界上，避免每次移动一条就触发一次重算；
 * 落在同一块内的后续移动不再引起窗口变化（调用方可依赖同一返回值跳过 setState）。
 *
 * @param current       - 当前窗口大小
 * @param selectedIndex - 选中项在完整过滤结果中的索引（-1 表示无选中项）
 * @param total         - 当前过滤结果总数
 * @returns 能包含选中项的窗口大小，且不小于当前窗口；total = 0 时保持当前窗口（不清零）
 */
export function nextRenderCount(current: number, selectedIndex: number, total: number): number {
  const clampedCurrent = clampToTotal(current, total);
  if (selectedIndex < 0) return clampedCurrent;
  const needed = selectedIndex + 1;
  if (needed <= clampedCurrent) return clampedCurrent;
  const blockAligned = Math.ceil(needed / RENDER_BLOCK_SIZE) * RENDER_BLOCK_SIZE;
  return clampToTotal(Math.max(blockAligned, clampedCurrent), total);
}

/**
 * 过滤条件变化时是否需要把窗口重置回首屏。
 *
 * 判定键由所有影响结果集的过滤条件组成（搜索词、类型筛选）；键变了结果集必然不同，
 * 旧窗口没有意义——否则「搜完再清空」会保留一个巨大的窗口，等于没治理。
 * 键不变（例如输入后被回退成同样的值）时不重置，避免无谓的窗口收缩。
 *
 * @param prevKey - 上一次的过滤条件键
 * @param nextKey - 本次的过滤条件键
 * @returns 需要重置返回 true
 */
export function shouldResetRenderCount(prevKey: string, nextKey: string): boolean {
  return prevKey !== nextKey;
}

/**
 * 滚动位置是否已进入「接近底部」的追加区。
 *
 * @param scrollTop    - 当前滚动偏移
 * @param scrollHeight - 滚动内容总高度
 * @param clientHeight - 可视区高度
 * @returns 距离底部不超过阈值返回 true
 */
export function isNearBottom(
  scrollTop: number,
  scrollHeight: number,
  clientHeight: number,
): boolean {
  return scrollHeight - scrollTop - clientHeight <= NEAR_BOTTOM_THRESHOLD_PX;
}
