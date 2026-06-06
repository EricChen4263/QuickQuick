import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import { ClipPreview } from "./ClipPreview";
import type { ClipItem } from "../../ipc/ipc-client";

const NOOP = vi.fn();

/** 构造测试用条目，覆盖默认字段后允许逐项覆盖。 */
function makeItem(overrides: Partial<ClipItem>): ClipItem {
  return {
    id: "id-1",
    content: "fallback content",
    kind: "text",
    isFavorite: false,
    lastModifiedUtc: 1_700_000_000_000,
    ...overrides,
  };
}

function renderPreview(item: ClipItem) {
  return render(
    <ClipPreview
      item={item}
      onToggleFavorite={NOOP}
      onDelete={NOOP}
      onCopy={NOOP}
      onPasteToFront={NOOP}
      onTranslate={NOOP}
    />,
  );
}

describe("ClipPreview", () => {
  it("clip_preview_renders_sanitized_richtext", () => {
    const item = makeItem({
      kind: "richtext",
      htmlContent: "<b>hi</b><table><tr><td>x</td></tr></table>",
    });

    const { container } = renderPreview(item);

    const preview = container.querySelector(".preview-content");
    expect(preview).not.toBeNull();
    expect(preview!.querySelector("b")).not.toBeNull();
    expect(preview!.querySelector("table")).not.toBeNull();
    expect(preview!.textContent).toContain("hi");
  });

  it("clip_preview_plaintext_unchanged", () => {
    const item = makeItem({
      kind: "text",
      content: "纯文本内容 <b>not parsed</b>",
    });

    const { container } = renderPreview(item);

    const preview = container.querySelector(".preview-content");
    expect(preview).not.toBeNull();
    // 纯文本走文本渲染：尖括号作为字面文本出现，不被解析成元素
    expect(preview!.querySelector("b")).toBeNull();
    expect(preview!.textContent).toBe("纯文本内容 <b>not parsed</b>");
  });

  it("clip_preview_strips_malicious_html", () => {
    const item = makeItem({
      kind: "richtext",
      htmlContent:
        '<img src=x onerror="alert(1)">点<script>alert(2)</script>击<a href="javascript:alert(3)">x</a><iframe src="javascript:alert(4)"></iframe>',
    });

    const { container } = renderPreview(item);

    const preview = container.querySelector(".preview-content");
    expect(preview).not.toBeNull();

    // 危险内容被剥离
    expect(preview!.querySelector("script")).toBeNull();
    expect(preview!.querySelector("iframe")).toBeNull();
    const img = preview!.querySelector("img");
    if (img) {
      expect(img.hasAttribute("onerror")).toBe(false);
    }
    const anchor = preview!.querySelector("a");
    if (anchor) {
      // javascript: 协议被剥离：要么 href 整体被移除（null），要么不含 javascript:
      const href = anchor.getAttribute("href") ?? "";
      expect(href).not.toContain("javascript:");
    }
    expect(preview!.innerHTML.toLowerCase()).not.toContain("onerror");
    expect(preview!.innerHTML.toLowerCase()).not.toContain("javascript:");

    // 正常文本保留
    expect(preview!.textContent).toContain("点");
    expect(preview!.textContent).toContain("击");
  });
});
