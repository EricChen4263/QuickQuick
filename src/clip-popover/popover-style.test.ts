import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { DEFAULT_IMAGE_HEIGHT_PX } from "../panels/settings/appearance-settings";

const popoverCssPath = resolve(process.cwd(), "src/clip-popover/popover.css");

function readPopoverCss(): string {
  return readFileSync(popoverCssPath, "utf-8");
}

describe("popover.css 缩略图高度契约", () => {
  it("CSS fallback 与设置默认值一致（40px，Maccy 的 imageMaxHeight 默认）", () => {
    const css = readPopoverCss();

    const match = css.match(/\.popover-row-thumb\s*\{[^}]*height:\s*var\(--popover-thumb-height,\s*(\d+)px\)/);
    expect(match, "缩略图高度必须走 --popover-thumb-height 变量并带 px fallback").not.toBeNull();

    const fallback = Number(match?.[1]);
    expect(fallback).toBe(40);
    expect(fallback).toBe(DEFAULT_IMAGE_HEIGHT_PX);
  });
});

describe("popover.css 大图预览契约", () => {
  it("预览图高度必须有上限，避免撑破 480pt 的 popover 窗口", () => {
    const css = readPopoverCss();

    const match = css.match(/\.popover-preview-img\s*\{[^}]*max-height:\s*([\d.]+)(vh|px)/);
    expect(match, "预览图必须声明 max-height").not.toBeNull();
    expect(match?.[2]).toBe("vh");
    expect(Number(match?.[1])).toBe(70);
  });
});

