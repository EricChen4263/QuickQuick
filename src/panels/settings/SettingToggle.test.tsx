import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import SettingToggle from "./SettingToggle";

describe("SettingToggle", () => {
  it("渲染在 .set-row 里（复用 SettingRow）", () => {
    const { container } = render(
      <SettingToggle label="开机启动" checked={false} onChange={() => {}} />
    );
    expect(container.querySelector(".set-row")).not.toBeNull();
  });

  it("渲染 button.switch[role=switch]", () => {
    const { container } = render(
      <SettingToggle label="开机启动" checked={false} onChange={() => {}} />
    );
    const btn = container.querySelector("button.switch");
    expect(btn).not.toBeNull();
    expect(btn?.getAttribute("role")).toBe("switch");
  });

  it("checked=false 时 aria-checked 为 false", () => {
    const { container } = render(
      <SettingToggle label="开机启动" checked={false} onChange={() => {}} />
    );
    expect(container.querySelector("button.switch")?.getAttribute("aria-checked")).toBe("false");
  });

  it("checked=true 时 aria-checked 为 true", () => {
    const { container } = render(
      <SettingToggle label="开机启动" checked={true} onChange={() => {}} />
    );
    expect(container.querySelector("button.switch")?.getAttribute("aria-checked")).toBe("true");
  });

  it("点击时以取反值调用 onChange", async () => {
    const onChange = vi.fn();
    const { container } = render(
      <SettingToggle label="开机启动" checked={false} onChange={onChange} />
    );
    await userEvent.click(container.querySelector("button.switch")!);
    expect(onChange).toHaveBeenCalledOnce();
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("disabled=true 时按钮被禁用且点击不触发 onChange", async () => {
    const onChange = vi.fn();
    const { container } = render(
      <SettingToggle label="开机启动" checked={false} onChange={onChange} disabled />
    );
    const btn = container.querySelector("button.switch")!;
    expect(btn).toBeDisabled();
    await userEvent.click(btn);
    expect(onChange).not.toHaveBeenCalled();
  });

  it("aria-label 设为 label 文字", () => {
    const { container } = render(
      <SettingToggle label="开机启动" checked={false} onChange={() => {}} />
    );
    expect(container.querySelector("button.switch")?.getAttribute("aria-label")).toBe("开机启动");
  });

  it("有 description 时渲染 .desc", () => {
    const { container } = render(
      <SettingToggle label="开机启动" description="系统启动时自动运行" checked={false} onChange={() => {}} />
    );
    expect(container.querySelector(".desc")?.textContent).toBe("系统启动时自动运行");
  });

  it("不渲染 input 元素（button 版 switch，非旧 input+label 版）", () => {
    const { container } = render(
      <SettingToggle label="开机启动" checked={false} onChange={() => {}} />
    );
    expect(container.querySelector("input")).toBeNull();
  });

  it("button.switch 的 type 为 button（防止表单提交）", () => {
    const { container } = render(
      <SettingToggle label="开机启动" checked={false} onChange={() => {}} />
    );
    expect(container.querySelector("button.switch")?.getAttribute("type")).toBe("button");
  });
});
