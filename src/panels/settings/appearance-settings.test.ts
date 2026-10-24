import { describe, it, expect } from "vitest";
import {
  DEFAULT_APPEARANCE_SETTINGS,
  DEFAULT_HIGHLIGHT_STYLE,
  DEFAULT_IMAGE_HEIGHT_PX,
  DEFAULT_PREVIEW_FIRST_DELAY_MS,
  DEFAULT_SORT_ORDER,
  HIGHLIGHT_STYLE_OPTIONS,
  MAX_IMAGE_HEIGHT_PX,
  MAX_PREVIEW_FIRST_DELAY_MS,
  MIN_IMAGE_HEIGHT_PX,
  MIN_PREVIEW_FIRST_DELAY_MS,
  SORT_ORDER_OPTIONS,
  isHighlightStyle,
  isSortOrder,
  normalizeAppearanceSettings,
  parseIntInRange,
} from "./appearance-settings";
import { PREVIEW_FIRST_DELAY_MS } from "../../clip-popover/preview-delay";

describe("外观设置默认值", () => {
  it("默认值与改动前行为一致：最近复制 / 24px / 1500ms / 背景高亮", () => {
    expect(DEFAULT_SORT_ORDER).toBe("recent");
    expect(DEFAULT_IMAGE_HEIGHT_PX).toBe(24);
    expect(DEFAULT_PREVIEW_FIRST_DELAY_MS).toBe(1500);
    expect(DEFAULT_HIGHLIGHT_STYLE).toBe("default");
  });

  it("首次预览默认值与弹窗硬编码默认值同源（不允许两处各写 1500）", () => {
    expect(DEFAULT_PREVIEW_FIRST_DELAY_MS).toBe(PREVIEW_FIRST_DELAY_MS);
  });

  it("DEFAULT_APPEARANCE_SETTINGS 与四个默认常量逐项一致（避免两处各写一份）", () => {
    expect(DEFAULT_APPEARANCE_SETTINGS).toEqual({
      sortOrder: DEFAULT_SORT_ORDER,
      imageHeightPx: DEFAULT_IMAGE_HEIGHT_PX,
      previewFirstDelayMs: DEFAULT_PREVIEW_FIRST_DELAY_MS,
      highlightStyle: DEFAULT_HIGHLIGHT_STYLE,
    });
  });

  it("区间常量与 Rust 侧对齐：图片高度 16–48、首次预览 200–5000", () => {
    expect([MIN_IMAGE_HEIGHT_PX, MAX_IMAGE_HEIGHT_PX]).toEqual([16, 48]);
    expect([MIN_PREVIEW_FIRST_DELAY_MS, MAX_PREVIEW_FIRST_DELAY_MS]).toEqual([200, 5000]);
  });
});

describe("parseIntInRange", () => {
  it("区间内整数原样返回（含两端）", () => {
    expect(parseIntInRange("16", 16, 48)).toBe(16);
    expect(parseIntInRange("40", 16, 48)).toBe(40);
    expect(parseIntInRange("48", 16, 48)).toBe(48);
  });

  it("前后空白被忽略", () => {
    expect(parseIntInRange("  24 ", 16, 48)).toBe(24);
  });

  it("越界返回 null（拒收而不是悄悄收敛）", () => {
    expect(parseIntInRange("15", 16, 48)).toBeNull();
    expect(parseIntInRange("49", 16, 48)).toBeNull();
    expect(parseIntInRange("0", 16, 48)).toBeNull();
  });

  it("非纯整数字面量一律 null（含小数、科学计数、空串、全角数字）", () => {
    for (const raw of ["24.5", "2.4e1", "", "abc", "２４", "-24", "+24"]) {
      expect(parseIntInRange(raw, 16, 48)).toBeNull();
    }
  });
});

describe("类型守卫", () => {
  it("isSortOrder 只认 recent / first_copied", () => {
    expect(isSortOrder("recent")).toBe(true);
    expect(isSortOrder("first_copied")).toBe(true);
    expect(isSortOrder("by_magic")).toBe(false);
    expect(isSortOrder(undefined)).toBe(false);
  });

  it("isHighlightStyle 只认 default / underline / bold", () => {
    for (const value of ["default", "underline", "bold"]) {
      expect(isHighlightStyle(value)).toBe(true);
    }
    expect(isHighlightStyle("rainbow")).toBe(false);
    expect(isHighlightStyle(1)).toBe(false);
  });
});

describe("normalizeAppearanceSettings", () => {
  it("合法输入原样透传", () => {
    const input = {
      sortOrder: "first_copied",
      imageHeightPx: 40,
      previewFirstDelayMs: 800,
      highlightStyle: "bold",
    };

    expect(normalizeAppearanceSettings(input)).toEqual(input);
  });

  it("undefined / null / 非对象一律回退默认值", () => {
    expect(normalizeAppearanceSettings(undefined)).toEqual(DEFAULT_APPEARANCE_SETTINGS);
    expect(normalizeAppearanceSettings(null)).toEqual(DEFAULT_APPEARANCE_SETTINGS);
    expect(normalizeAppearanceSettings("nope")).toEqual(DEFAULT_APPEARANCE_SETTINGS);
  });

  it("缺字段只回退该字段，不动其它已给的值", () => {
    const normalized = normalizeAppearanceSettings({ imageHeightPx: 32 });

    expect(normalized).toEqual({
      sortOrder: DEFAULT_SORT_ORDER,
      imageHeightPx: 32,
      previewFirstDelayMs: DEFAULT_PREVIEW_FIRST_DELAY_MS,
      highlightStyle: DEFAULT_HIGHLIGHT_STYLE,
    });
  });

  it("数值越界被收敛到区间（数字可以夹，枚举不能猜）", () => {
    const normalized = normalizeAppearanceSettings({
      imageHeightPx: 999,
      previewFirstDelayMs: 0,
      sortOrder: "by_magic",
      highlightStyle: "rainbow",
    });

    expect(normalized.imageHeightPx).toBe(MAX_IMAGE_HEIGHT_PX);
    expect(normalized.previewFirstDelayMs).toBe(MIN_PREVIEW_FIRST_DELAY_MS);
    expect(normalized.sortOrder).toBe(DEFAULT_SORT_ORDER);
    expect(normalized.highlightStyle).toBe(DEFAULT_HIGHLIGHT_STYLE);
  });

  it("非数字 / NaN / Infinity 回退默认值（不会把 NaN 透给 CSS）", () => {
    for (const bad of ["40", Number.NaN, Number.POSITIVE_INFINITY]) {
      const normalized = normalizeAppearanceSettings({ imageHeightPx: bad });
      expect(normalized.imageHeightPx).toBe(DEFAULT_IMAGE_HEIGHT_PX);
    }
  });
});

describe("下拉选项", () => {
  it("排序方式两档、第一项为默认档", () => {
    expect(SORT_ORDER_OPTIONS.map((option) => option.value)).toEqual(["recent", "first_copied"]);
    expect(SORT_ORDER_OPTIONS[0].value).toBe(DEFAULT_SORT_ORDER);
  });

  it("命中高亮三档、第一项为默认档", () => {
    expect(HIGHLIGHT_STYLE_OPTIONS.map((option) => option.value)).toEqual([
      "default",
      "underline",
      "bold",
    ]);
    expect(HIGHLIGHT_STYLE_OPTIONS[0].value).toBe(DEFAULT_HIGHLIGHT_STYLE);
  });
});
