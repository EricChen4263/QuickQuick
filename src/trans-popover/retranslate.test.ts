import { describe, it, expect } from "vitest";
import { shouldRetranslate } from "./retranslate";

describe("shouldRetranslate", () => {
  it("相同文本 → false（不重译）", () => {
    expect(shouldRetranslate("hello", "hello")).toBe(false);
  });

  it("不同文本 → true（触发重译）", () => {
    expect(shouldRetranslate("world", "hello")).toBe(true);
  });

  it("newText 为 null → false（无可译内容）", () => {
    expect(shouldRetranslate(null, "hello")).toBe(false);
  });

  it("lastText 为 null 且 newText 有值 → true（首次翻译）", () => {
    expect(shouldRetranslate("hello", null)).toBe(true);
  });
});
