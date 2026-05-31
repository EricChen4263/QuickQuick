// 设计语言 token 模块（§9.1）
// 峡湾青蓝品牌主色固定，不跟随系统；明暗双主题随系统切换。

/** 峡湾青蓝 Fjord Teal-Blue 品牌主色（§9.1：固定不跟随系统）*/
export const BRAND_FJORD_TEAL = "#3A7CA5";

/** 中圆角（§9.1：形态调性 ~10px）*/
export const RADIUS_MD = "10px";

/**
 * 系统原生字体栈（§9.1：mac SF Pro+苹方 / Win Segoe UI+微软雅黑）
 * 按平台优先级排列，最终回退到 sans-serif。
 */
export const FONT_STACK =
  "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', " +
  "'Segoe UI', '微软雅黑', sans-serif";

/**
 * 主题 token 接口（§9.1）
 * bg      — 主窗背景实色（浅色近白 / 深色近黑）
 * surface — 卡片/浮层背景（比 bg 略深/浅一级）
 * text    — 主文字色
 * textMuted — 次要/辅助文字色
 * border  — 细描边色（微色差分层）
 * accent  — 品牌强调色（基于 BRAND_FJORD_TEAL，各主题取适配明度版）
 */
export interface ThemeTokens {
  bg: string;
  surface: string;
  text: string;
  textMuted: string;
  border: string;
  accent: string;
}

/**
 * 浅色主题 token（§9.1：主窗实色，近白底）
 * accent 直接用品牌主色，浅色背景下对比度充足。
 */
export const lightTheme: ThemeTokens = {
  bg: "#F5F6F7",
  surface: "#FFFFFF",
  text: "#1A1A1A",
  textMuted: "#6B7280",
  border: "#E2E4E8",
  accent: BRAND_FJORD_TEAL,
};

/**
 * 深色主题 token（§9.1：主窗实色，近黑底）
 * accent 提亮至 #5B9FC4，保证在深色背景上的可读性与对比度。
 */
export const darkTheme: ThemeTokens = {
  bg: "#141517",
  surface: "#1E2023",
  text: "#F0F1F3",
  textMuted: "#9CA3AF",
  border: "#2D3038",
  accent: "#5B9FC4",
};

/**
 * 把 ThemeTokens 映射为 CSS 变量键值对（§9.1 CSS 变量契约）
 * 同时注入全局静态 token（--qq-radius-md、--qq-font）。
 * 返回值可直接赋给 DOM 元素的 style 属性。
 *
 * CSS 变量名约定（前缀 --qq- 统一）：
 *   --qq-bg、--qq-surface、--qq-text、--qq-text-muted、
 *   --qq-border、--qq-accent、--qq-radius-md、--qq-font
 */
export function themeToCssVars(theme: ThemeTokens): Record<string, string> {
  return {
    "--qq-bg": theme.bg,
    "--qq-surface": theme.surface,
    "--qq-text": theme.text,
    "--qq-text-muted": theme.textMuted,
    "--qq-border": theme.border,
    "--qq-accent": theme.accent,
    "--qq-radius-md": RADIUS_MD,
    "--qq-font": FONT_STACK,
  };
}
