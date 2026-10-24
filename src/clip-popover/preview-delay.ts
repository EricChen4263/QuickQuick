/**
 * 行内预览的分级延迟策略。
 *
 * 预览数据已经在内存里（PopoverPreview 直接渲染 ClipItem，不发请求、不读磁盘），
 * 所以延迟的目的不是「少取数据」，而是「快速上下翻页时预览区不乱跳」：
 * 按住 ↓ 连按时，中间条目的预览不该逐条闪过去。
 *
 * 两档延迟：
 * - 首个预览默认 1500ms（可由设置项「首次预览延迟」调整）：面板刚打开时用户通常
 *   还在浏览列表、没决定看哪一条，先让列表说话；
 * - 后续切换 200ms（固定，不开放配置）：已经进入看预览的节奏，只需要滤掉翻页途中的中间项。
 */

/** 面板本次打开后首个预览的等待时长默认值（毫秒），与设置项默认值同一来源 */
export const PREVIEW_FIRST_DELAY_MS = 1500;

/** 首个预览出现之后，切换条目的防抖等待时长（毫秒）；固定不与设置联动 */
export const PREVIEW_SWITCH_DELAY_MS = 200;

/**
 * 取得这一次预览该等待的毫秒数。
 *
 * @param isFirstPreview - 面板本次打开后是否尚未显示过任何预览
 * @param firstDelayMs - 首个预览的等待时长（毫秒）；缺省用默认 1500ms。
 *   设置项只作用于这一档，后续切换恒为 200ms
 * @returns 首个预览 `firstDelayMs`，此后 200ms
 */
export function previewDelayMs(
  isFirstPreview: boolean,
  firstDelayMs: number = PREVIEW_FIRST_DELAY_MS,
): number {
  return isFirstPreview ? firstDelayMs : PREVIEW_SWITCH_DELAY_MS;
}
