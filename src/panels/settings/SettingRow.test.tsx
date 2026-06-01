import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import SettingRow from "./SettingRow";

describe("SettingRow", () => {
  it("渲染 .set-row 容器", () => {
    const { container } = render(
      <SettingRow label="启动登录" children={<span>控件</span>} />
    );
    expect(container.querySelector(".set-row")).not.toBeNull();
  });

  it(".grow 内含 .label 文字", () => {
    const { container } = render(
      <SettingRow label="启动登录" children={<span>控件</span>} />
    );
    const label = container.querySelector(".set-row .grow .label");
    expect(label?.textContent).toBe("启动登录");
  });

  it("有 description 时渲染 .desc", () => {
    const { container } = render(
      <SettingRow label="启动登录" description="开机自动启动" children={<span>控件</span>} />
    );
    const desc = container.querySelector(".set-row .grow .desc");
    expect(desc?.textContent).toBe("开机自动启动");
  });

  it("无 description 时不渲染 .desc", () => {
    const { container } = render(
      <SettingRow label="启动登录" children={<span>控件</span>} />
    );
    expect(container.querySelector(".desc")).toBeNull();
  });

  it("将 children 渲染到 .set-row 的控件区", () => {
    render(
      <SettingRow label="启动登录" children={<span>右侧控件</span>} />
    );
    expect(screen.getByText("右侧控件")).toBeInTheDocument();
  });
});
