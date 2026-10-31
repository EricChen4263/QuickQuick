import type {
  TransFallbackReason,
  TransSourceKind,
  TransSourcePayload,
} from "../ipc/ipc-client";

/** 事件 payload 落到「无文本」时的收敛结果：宁可提示用户去选字，也不误译一段 undefined。 */
const EMPTY_PAYLOAD: TransSourcePayload = {
  text: null,
  source: "clipboard",
  fallbackReason: null,
};

/**
 * 把 trans-source 事件的 payload 收敛成 TransSourcePayload。
 *
 * 为什么需要运行时收敛：payload 由 Rust 侧序列化，结构变了（或前端 bundle 与后端版本
 * 错配）时直接当对象取字段，会把 undefined 当成待译文本喂给 translateText，界面上表现为
 * 一条莫名其妙的「翻译失败」。这里统一退化成「无文本」，让浮窗给出可行动的提示。
 */
export function asTransSourcePayload(raw: unknown): TransSourcePayload {
  if (typeof raw !== "object" || raw === null) {
    return EMPTY_PAYLOAD;
  }

  const payload = raw as Partial<TransSourcePayload>;
  return {
    text: typeof payload.text === "string" ? payload.text : null,
    source: toSourceKind(payload.source),
    fallbackReason: toFallbackReason(payload.fallbackReason),
  };
}

/** 只认后端序列化出的两个合法值，其余一律按剪贴板处理（来源标签宁可保守，不可虚报选中）。 */
function toSourceKind(value: unknown): TransSourceKind {
  return value === "selection" ? "selection" : "clipboard";
}

/** 未知的降级原因丢弃：提示文案只对已确认的两种原因负责。 */
function toFallbackReason(value: unknown): TransFallbackReason | null {
  return value === "noSelection" || value === "unsupported" || value === "untrusted"
    ? value
    : null;
}
