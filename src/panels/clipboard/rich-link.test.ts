/**
 * 富文本链接点击解析纯函数测试（RT1-F2-S03）。
 * 防 webview 劫持：仅 http/https/mailto 协议链接返回 url，其余（file:// 等）返回 null。
 */

import { describe, it, expect } from "vitest";
import { resolveRichLinkClick } from "./rich-link";

/** 构造带链接的容器并返回指定子元素（用于模拟点击目标） */
function buildAnchor(href: string, innerHtml = "link"): HTMLElement {
  const container = document.createElement("div");
  container.innerHTML = `<a href="${href}">${innerHtml}</a>`;
  return container.querySelector("a") as HTMLElement;
}

describe("resolveRichLinkClick", () => {
  it("resolve_rich_link_filters_non_http_schemes", () => {
    // file:// 协议：防本地文件被打开，返回 null
    const fileAnchor = buildAnchor("file:///etc/passwd");
    expect(resolveRichLinkClick(fileAnchor)).toBeNull();

    // javascript: / data: 协议：防脚本注入与内联 HTML 渲染，返回 null
    expect(resolveRichLinkClick(buildAnchor("javascript:alert(1)"))).toBeNull();
    expect(resolveRichLinkClick(buildAnchor("data:text/html,<b>x</b>"))).toBeNull();

    // 无 href（裸 <a>）：返回 null
    const bareContainer = document.createElement("div");
    bareContainer.innerHTML = `<a>no href</a>`;
    expect(resolveRichLinkClick(bareContainer.querySelector("a") as HTMLElement)).toBeNull();

    // http / https / mailto：返回完整 url
    expect(resolveRichLinkClick(buildAnchor("https://example.com/p"))).toBe("https://example.com/p");
    expect(resolveRichLinkClick(buildAnchor("http://example.com"))).toBe("http://example.com/");
    expect(resolveRichLinkClick(buildAnchor("mailto:a@b.com"))).toBe("mailto:a@b.com");
  });

  it("点击链接内的子元素（如 <b>）向上找到最近的 <a>", () => {
    const container = document.createElement("div");
    container.innerHTML = `<a href="https://example.com/x"><b>bold link</b></a>`;
    const bold = container.querySelector("b") as HTMLElement;
    expect(resolveRichLinkClick(bold)).toBe("https://example.com/x");
  });

  it("点击非链接元素返回 null", () => {
    const container = document.createElement("div");
    container.innerHTML = `<b>not a link</b>`;
    const bold = container.querySelector("b") as HTMLElement;
    expect(resolveRichLinkClick(bold)).toBeNull();
  });
});
