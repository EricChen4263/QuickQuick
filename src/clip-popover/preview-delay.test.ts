import { describe, it, expect } from "vitest";
import { previewDelayMs, PREVIEW_FIRST_DELAY_MS, PREVIEW_SWITCH_DELAY_MS } from "./preview-delay";

describe("previewDelayMs 分级延迟取值", () => {
  it("面板本次打开的首个预览返回 1500ms", () => {
    expect(previewDelayMs(true)).toBe(1500);
  });

  it("首个预览出现后的切换返回 200ms", () => {
    expect(previewDelayMs(false)).toBe(200);
  });

  it("两档常量本身仍是 1500 / 200（改档位必须是有意为之）", () => {
    expect(PREVIEW_FIRST_DELAY_MS).toBe(1500);
    expect(PREVIEW_SWITCH_DELAY_MS).toBe(200);
  });

  it("首个预览档可被设置覆盖（传 300 就等 300ms）", () => {
    expect(previewDelayMs(true, 300)).toBe(300);
    expect(previewDelayMs(true, 300)).not.toBe(PREVIEW_FIRST_DELAY_MS);
  });

  it("后续切换档不受设置影响：传任何首个延迟都仍是 200ms", () => {
    for (const firstDelay of [200, 300, 1500, 5000]) {
      expect(previewDelayMs(false, firstDelay)).toBe(PREVIEW_SWITCH_DELAY_MS);
    }
  });
});
