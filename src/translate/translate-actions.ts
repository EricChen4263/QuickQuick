// V2-F3-A15 译文操作集
//
// 职责：定义译文操作类型，并提供纯函数建模操作解析。
// UI 命令字符串 → TranslateAction 的映射关系集中在此，便于测试和复用。

/** 译文操作枚举：一键复制 / 朗读 / 存翻译历史 */
export type TranslateAction = "copy" | "speak" | "save_history";

/** 全部可用操作的有序列表（用于 UI 渲染和操作完整性断言）。 */
export function availableActions(): TranslateAction[] {
  return ["copy", "speak", "save_history"];
}

/**
 * 将 UI 命令字符串映射为对应的 TranslateAction。
 *
 * 合法命令（与 TranslateAction 字面量一一对应）返回对应操作；
 * 非法或未知命令返回 null，不抛异常。
 */
export function resolveTranslateAction(cmd: string): TranslateAction | null {
  const valid = new Set<string>(availableActions());
  if (valid.has(cmd)) {
    return cmd as TranslateAction;
  }
  return null;
}
