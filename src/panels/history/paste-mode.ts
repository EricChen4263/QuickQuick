// V1-F3-S06: 前端粘贴模式解析（A16）
//
// 设计对齐：设计文档§三关键机制（回车粘贴 / 修饰键仅复制）
// Enter 无修饰键 → paste（写回 + 发模拟粘贴）
// Enter 带任意修饰键 → copy_only（仅写回剪贴板，不发粘贴）

/** 粘贴模式：paste = 写回+粘贴；copy_only = 仅写回不粘贴 */
export type PasteMode = "paste" | "copy_only";

/**
 * 根据是否携带修饰键解析粘贴模式。
 *
 * @param hasModifier - 用户按键时是否同时按下修饰键（Cmd/Ctrl/Alt/Shift 等）
 * @returns "paste" 表示写回并模拟粘贴；"copy_only" 表示仅写回剪贴板
 */
export function resolvePasteMode(hasModifier: boolean): PasteMode {
  return hasModifier ? "copy_only" : "paste";
}
