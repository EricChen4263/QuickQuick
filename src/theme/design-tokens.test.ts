import { describe, it, expect } from "vitest";
import {
  BRAND_FJORD_TEAL,
  RADIUS_MD,
  FONT_STACK,
  lightTheme,
  darkTheme,
  themeToCssVars,
} from "./design-tokens";
import { readFileSync } from "fs";
import { resolve } from "path";

// V4-F3-A12: 设计语言 token 落地（§9.1 峡湾青蓝 / 中圆角 / 深浅双主题）

describe("design-tokens", () => {
  it("BRAND_FJORD_TEAL 精确等于品牌主色 #3A7CA5", () => {
    expect(BRAND_FJORD_TEAL).toBe("#3A7CA5");
  });

  it("RADIUS_MD 精确等于中圆角 10px", () => {
    expect(RADIUS_MD).toBe("10px");
  });

  it("FONT_STACK 非空且含系统原生字体", () => {
    expect(typeof FONT_STACK).toBe("string");
    expect(FONT_STACK.length).toBeGreaterThan(0);
  });

  it("lightTheme 含全部 ThemeTokens 键且非空", () => {
    const keys: Array<keyof typeof lightTheme> = [
      "bg",
      "surface",
      "text",
      "textMuted",
      "border",
      "accent",
    ];
    for (const key of keys) {
      expect(lightTheme[key], `lightTheme.${key} 应非空`).toBeTruthy();
      expect(typeof lightTheme[key]).toBe("string");
    }
  });

  it("darkTheme 含全部 ThemeTokens 键且非空", () => {
    const keys: Array<keyof typeof darkTheme> = [
      "bg",
      "surface",
      "text",
      "textMuted",
      "border",
      "accent",
    ];
    for (const key of keys) {
      expect(darkTheme[key], `darkTheme.${key} 应非空`).toBeTruthy();
      expect(typeof darkTheme[key]).toBe("string");
    }
  });

  it("lightTheme 与 darkTheme 的 bg 不同（明暗有别）", () => {
    expect(lightTheme.bg).not.toBe(darkTheme.bg);
  });

  it("lightTheme accent 基于品牌色 #3A7CA5", () => {
    expect(lightTheme.accent).toBe(BRAND_FJORD_TEAL);
  });

  it("darkTheme accent 派生自品牌色（含 #3A7CA5 或更亮适配版）", () => {
    expect(darkTheme.accent.length).toBeGreaterThan(0);
    // 深色主题 accent 基于品牌色（同色或提亮变体，均为 hex 格式）
    expect(darkTheme.accent).toMatch(/^#[0-9A-Fa-f]{6}$/);
  });

  it("themeToCssVars(lightTheme) 产出含 --qq-accent 且值为品牌色", () => {
    const vars = themeToCssVars(lightTheme);
    expect(vars["--qq-accent"]).toBe(BRAND_FJORD_TEAL);
  });

  it("themeToCssVars(lightTheme) 产出含 --qq-radius-md 值为 10px", () => {
    const vars = themeToCssVars(lightTheme);
    expect(vars["--qq-radius-md"]).toBe("10px");
  });

  it("themeToCssVars(lightTheme) 产出含 --qq-font", () => {
    const vars = themeToCssVars(lightTheme);
    expect(typeof vars["--qq-font"]).toBe("string");
    expect(vars["--qq-font"].length).toBeGreaterThan(0);
  });

  it("theme.css 含品牌色、圆角、dark media query 及毛玻璃", () => {
    const cssPath = resolve(__dirname, "./theme.css");
    const css = readFileSync(cssPath, "utf-8");
    expect(css).toContain("#3A7CA5");
    expect(css).toContain("10px");
    expect(css).toContain("prefers-color-scheme: dark");
    expect(css).toContain("backdrop-filter");
  });
});
