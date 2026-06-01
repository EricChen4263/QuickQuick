/**
 * OnboardCard 测试：首次运行辅助功能引导卡片。
 * 验证渲染结构、按钮回调和 aria 标签均符合行为契约。
 */

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { OnboardCard } from "./OnboardCard";

describe("OnboardCard", () => {
  it("渲染标题和说明文字", () => {
    render(
      <OnboardCard onDismiss={vi.fn()} onOpenSystemSettings={vi.fn()} />
    );
    expect(screen.getByText(/辅助功能|开启/)).toBeInTheDocument();
    expect(screen.getByText(/前往系统设置/)).toBeInTheDocument();
  });

  it("点击「前往系统设置」调用 onOpenSystemSettings", async () => {
    const onOpenSystemSettings = vi.fn();
    const user = userEvent.setup();
    render(
      <OnboardCard onDismiss={vi.fn()} onOpenSystemSettings={onOpenSystemSettings} />
    );

    await user.click(screen.getByText("前往系统设置"));
    expect(onOpenSystemSettings).toHaveBeenCalledTimes(1);
  });

  it("点击「稍后」调用 onDismiss", async () => {
    const onDismiss = vi.fn();
    const user = userEvent.setup();
    render(
      <OnboardCard onDismiss={onDismiss} onOpenSystemSettings={vi.fn()} />
    );

    await user.click(screen.getByText("稍后"));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it("点击关闭按钮（aria-label=关闭）调用 onDismiss", async () => {
    const onDismiss = vi.fn();
    const user = userEvent.setup();
    render(
      <OnboardCard onDismiss={onDismiss} onOpenSystemSettings={vi.fn()} />
    );

    await user.click(screen.getByRole("button", { name: "关闭" }));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it("渲染 .onboard 根容器", () => {
    const { container } = render(
      <OnboardCard onDismiss={vi.fn()} onOpenSystemSettings={vi.fn()} />
    );
    expect(container.querySelector(".onboard")).not.toBeNull();
  });
});
