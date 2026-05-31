type RebindOk = { ok: true; accelerator: string };
type RebindFail = { ok: false; error: "已被占用" };

export type RebindResult = RebindOk | RebindFail;

/**
 * 实时校验新快捷键是否已被占用。
 *
 * 使用严格字符串相等（区分大小写），调用方负责统一大小写格式。
 * 返回判定结果：通过则携带 accelerator，拒绝则携带错误原因。
 */
export function validateRebind(
  newAccel: string,
  occupied: string[],
): RebindResult {
  if (occupied.includes(newAccel)) {
    return { ok: false, error: "已被占用" };
  }
  return { ok: true, accelerator: newAccel };
}
