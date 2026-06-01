import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

let ThemeSwitchModule: { ThemeSwitch: React.ComponentType };
let storeModule: typeof import("../theme/themeStore");

// React 需要在模块重置前 import，否则每次 renderHook 会拿不同实例
import React from "react";

beforeEach(async () => {
  vi.resetModules();
  storeModule = await import("../theme/themeStore");
  storeModule._reset();
  ThemeSwitchModule = await import("./ThemeSwitch");
});

describe("ThemeSwitch 组件", () => {
  it("渲染三个按钮（auto/light/dark）", () => {
    const { ThemeSwitch } = ThemeSwitchModule;
    render(React.createElement(ThemeSwitch));
    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(3);
  });

  it("默认 pref=auto 时，跟随系统按钮 aria-pressed 为 true", () => {
    const { ThemeSwitch } = ThemeSwitchModule;
    render(React.createElement(ThemeSwitch));
    const autoBtn = screen.getByTitle("跟随系统");
    expect(autoBtn).toHaveAttribute("aria-pressed", "true");
  });

  it("默认 pref=auto 时，其余两按钮 aria-pressed 为 false", () => {
    const { ThemeSwitch } = ThemeSwitchModule;
    render(React.createElement(ThemeSwitch));
    expect(screen.getByTitle("浅色")).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByTitle("深色")).toHaveAttribute("aria-pressed", "false");
  });

  it("点击浅色按钮后，浅色 aria-pressed 变为 true，其余变 false", async () => {
    const { ThemeSwitch } = ThemeSwitchModule;
    const user = userEvent.setup();
    render(React.createElement(ThemeSwitch));
    await user.click(screen.getByTitle("浅色"));
    expect(screen.getByTitle("浅色")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTitle("跟随系统")).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByTitle("深色")).toHaveAttribute("aria-pressed", "false");
  });

  it("点击深色按钮后，深色 aria-pressed 变为 true", async () => {
    const { ThemeSwitch } = ThemeSwitchModule;
    const user = userEvent.setup();
    render(React.createElement(ThemeSwitch));
    await user.click(screen.getByTitle("深色"));
    expect(screen.getByTitle("深色")).toHaveAttribute("aria-pressed", "true");
  });

  it("点击按钮会调用 store.setPref", async () => {
    const { ThemeSwitch } = ThemeSwitchModule;
    const user = userEvent.setup();
    render(React.createElement(ThemeSwitch));
    await user.click(screen.getByTitle("深色"));
    expect(storeModule.getPref()).toBe("dark");
  });
});
