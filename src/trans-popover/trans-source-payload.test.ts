import { describe, it, expect } from "vitest";
import { asTransSourcePayload } from "./trans-source-payload";

describe("asTransSourcePayload", () => {
  it("合法 payload 原样透传文本、来源与降级原因", () => {
    const payload = asTransSourcePayload({
      text: "Hello world",
      source: "selection",
      fallbackReason: null,
    });

    expect(payload).toEqual({
      text: "Hello world",
      source: "selection",
      fallbackReason: null,
    });
  });

  it("旧格式（裸字符串）payload → 无文本，不把字符串当待译文本", () => {
    const payload = asTransSourcePayload("Hello world");

    expect(payload.text).toBeNull();
    expect(payload.source).toBe("clipboard");
  });

  it("非对象 payload（null / undefined）→ 无文本", () => {
    expect(asTransSourcePayload(null).text).toBeNull();
    expect(asTransSourcePayload(undefined).text).toBeNull();
  });

  it("text 类型不符 → 无文本；未知来源 → 保守按剪贴板；未知降级原因 → 丢弃", () => {
    const payload = asTransSourcePayload({
      text: 42,
      source: "bogus",
      fallbackReason: "made-up",
    });

    expect(payload.text).toBeNull();
    expect(payload.source).toBe("clipboard");
    expect(payload.fallbackReason).toBeNull();
  });
});
