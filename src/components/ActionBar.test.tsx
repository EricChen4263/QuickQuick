import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ActionBar, ActionBarHint } from "./ActionBar";

describe("ActionBar", () => {
  it("不传 as 默认渲染 div 根元素且含 glass 变体类", () => {
    const { container } = render(<ActionBar variant="glass">x</ActionBar>);
    const root = container.querySelector(".qq-action-bar");
    expect(root).not.toBeNull();
    expect(root?.tagName).toBe("DIV");
    expect(root?.classList.contains("qq-action-bar--glass")).toBe(true);
  });

  it("as=footer 渲染 footer 根元素", () => {
    const { container } = render(
      <ActionBar variant="glass" as="footer">
        x
      </ActionBar>,
    );
    expect(container.querySelector(".qq-action-bar")?.tagName).toBe("FOOTER");
  });

  it("variant=surface 含 surface 变体类", () => {
    const { container } = render(<ActionBar variant="surface">x</ActionBar>);
    expect(
      container.querySelector(".qq-action-bar")?.classList.contains("qq-action-bar--surface"),
    ).toBe(true);
  });

  it("align=between 含 align-between 类", () => {
    const { container } = render(
      <ActionBar variant="glass" align="between">
        x
      </ActionBar>,
    );
    expect(
      container
        .querySelector(".qq-action-bar")
        ?.classList.contains("qq-action-bar--align-between"),
    ).toBe(true);
  });

  it("不传 align 默认含 align-start 类", () => {
    const { container } = render(<ActionBar variant="glass">x</ActionBar>);
    expect(
      container
        .querySelector(".qq-action-bar")
        ?.classList.contains("qq-action-bar--align-start"),
    ).toBe(true);
  });

  it("children 透传可被查到", () => {
    render(
      <ActionBar variant="glass">
        <button type="button">确定</button>
      </ActionBar>,
    );
    expect(screen.getByText("确定")).toBeInTheDocument();
  });

  it("className 追加到根元素且不覆盖内置类", () => {
    const { container } = render(
      <ActionBar variant="glass" className="extra-class">
        x
      </ActionBar>,
    );
    const root = container.querySelector(".qq-action-bar");
    expect(root?.classList.contains("extra-class")).toBe(true);
    expect(root?.classList.contains("qq-action-bar--glass")).toBe(true);
  });
});

describe("ActionBarHint", () => {
  it("渲染含指定 kbd 文本的 kbd 元素与 label 文本", () => {
    const { container } = render(<ActionBarHint kbd="↵" label="粘贴" />);
    const kbd = container.querySelector("kbd.qq-kbd");
    expect(kbd?.textContent).toBe("↵");
    expect(screen.getByText("粘贴", { exact: false })).toBeInTheDocument();
  });
});
