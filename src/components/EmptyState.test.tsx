import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import EmptyState from "./EmptyState";

describe("EmptyState", () => {
  const icon = <svg data-testid="test-icon" />;

  it("渲染 .empty 容器", () => {
    const { container } = render(
      <EmptyState icon={icon} title="没有内容" description="暂无记录" />
    );
    expect(container.querySelector(".empty")).not.toBeNull();
  });

  it("渲染 .mark 包裹 icon", () => {
    const { container } = render(
      <EmptyState icon={icon} title="没有内容" description="暂无记录" />
    );
    const mark = container.querySelector(".empty .mark");
    expect(mark).not.toBeNull();
    expect(mark?.querySelector("[data-testid='test-icon']")).not.toBeNull();
  });

  it("在 h3 里渲染 title", () => {
    render(<EmptyState icon={icon} title="没有内容" description="暂无记录" />);
    expect(screen.getByRole("heading", { level: 3, name: "没有内容" })).toBeInTheDocument();
  });

  it("在 p 里渲染 description", () => {
    const { container } = render(
      <EmptyState icon={icon} title="没有内容" description="暂无记录" />
    );
    const p = container.querySelector(".empty p");
    expect(p?.textContent).toBe("暂无记录");
  });
});
