import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import SettingGroup from "./SettingGroup";

describe("SettingGroup", () => {
  it("渲染 .set-group 容器", () => {
    const { container } = render(
      <SettingGroup>
        <span>子项</span>
      </SettingGroup>
    );
    expect(container.querySelector(".set-group")).not.toBeNull();
  });

  it("将 children 渲染到 .set-group 内", () => {
    render(
      <SettingGroup>
        <span>子项内容</span>
      </SettingGroup>
    );
    expect(screen.getByText("子项内容")).toBeInTheDocument();
  });
});
