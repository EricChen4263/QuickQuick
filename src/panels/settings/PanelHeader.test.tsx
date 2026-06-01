import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import PanelHeader from "./PanelHeader";

describe("PanelHeader", () => {
  it("渲染 .set-h 含 title", () => {
    const { container } = render(
      <PanelHeader title="隐私" subtitle="管理数据与权限" />
    );
    const h = container.querySelector(".set-h");
    expect(h?.textContent).toBe("隐私");
  });

  it("渲染 .set-sub 含 subtitle", () => {
    const { container } = render(
      <PanelHeader title="隐私" subtitle="管理数据与权限" />
    );
    const sub = container.querySelector(".set-sub");
    expect(sub?.textContent).toBe("管理数据与权限");
  });
});
