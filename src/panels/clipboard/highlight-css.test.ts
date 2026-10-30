import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * 命中高亮 CSS 契约：基础规则必须自己接管底色。
 *
 * `<mark>` 带着 UA 的 `background-color: mark`（黄底），只有 `color` 档显式设了背景，
 * 其余三档若不覆盖底色就会残留黄底——这是真机验收时抓到的缺陷。
 * 两处渲染（主窗面板、弹出面板）各有一份 `.clip-hit`，任一处漏写都会复现，故一起断言。
 */

const CLIPBOARD_CSS = resolve(process.cwd(), "src/panels/clipboard/clipboard.css");
const POPOVER_CSS = resolve(process.cwd(), "src/clip-popover/popover.css");

interface CssTarget {
  /** 便于失败信息定位的文件名 */
  label: string;
  /** CSS 文件绝对路径 */
  path: string;
  /** 基础规则选择器（两份文件的写法不同） */
  baseSelector: string;
  /** 作用域根选择器：`color` 档的背景挂在这层上 */
  scopeSelector: string;
}

const TARGETS: CssTarget[] = [
  {
    label: "clipboard.css",
    path: CLIPBOARD_CSS,
    baseSelector: ".clip-hit",
    scopeSelector: ".clip-page",
  },
  {
    label: "popover.css",
    path: POPOVER_CSS,
    baseSelector: ".popover-row-text .clip-hit",
    scopeSelector: ".popover-shell",
  },
];

/** 取某个选择器第一条规则的声明块；找不到返回 null */
function declarationsFor(css: string, selector: string): string | null {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  // 选择器后必须紧跟 `{`（允许空白），避免误匹配 `.clip-hit-foo` 之类的同前缀选择器
  const match = css.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`));
  return match ? match[1] : null;
}

describe.each(TARGETS)("$label 命中高亮底色契约", (target) => {
  const css = readFileSync(target.path, "utf-8");

  it("基础 .clip-hit 规则显式清掉 UA 的 <mark> 黄底", () => {
    const declarations = declarationsFor(css, target.baseSelector);
    expect(declarations, `找不到 ${target.baseSelector} 基础规则`).not.toBeNull();

    // `background` 简写与 `background-color` 长写法都接受，但值必须是透明的
    const background = declarations?.match(/background(?:-color)?:\s*([^;]+);/)?.[1].trim();
    expect(background, "基础规则必须声明 background，否则 <mark> 黄底会残留").toBeDefined();
    expect(background).toBe("transparent");
  });

  it("颜色继承自父级：不继承 UA 的 marktext 黑字", () => {
    const declarations = declarationsFor(css, target.baseSelector);
    expect(declarations).toMatch(/color:\s*inherit;/);
  });

  it.each(["bold", "color", "italic", "underline"])("%s 档都有独立强调规则", (style) => {
    const selector = `${target.scopeSelector}[data-hit-style="${style}"] .clip-hit`;
    expect(declarationsFor(css, selector), `缺少 ${style} 档规则`).not.toBeNull();
  });

  it("color 档自带可见背景（不被基础透明规则吃掉）", () => {
    const selector = `${target.scopeSelector}[data-hit-style="color"] .clip-hit`;
    const declarations = declarationsFor(css, selector);
    const background = declarations?.match(/background:\s*([^;]+);/)?.[1].trim();

    expect(background, "color 档必须显式给背景").toBeDefined();
    expect(background).not.toBe("transparent");
  });
});
