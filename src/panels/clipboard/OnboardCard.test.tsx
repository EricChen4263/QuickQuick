/**
 * OnboardCard 测试：首次引导向导的状态机与多步渲染。
 *
 * 两层断言：
 * - 纯状态机（createGuideState / advanceGuide / skipGuideStep / observeAccessibilityTrust /
 *   finishGuide / isGuideVisible）——步骤推进、跳过、授权后自动收起、终态不再出现；
 * - 组件渲染——每步的按钮回调、步骤指示、以及最关键的「授权从未授权变为已授权后，
 *   授权请求不再渲染」。
 */

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getOnboardingStatus, type OnboardingStatus } from "../../ipc/ipc-client";
import {
  GUIDE_STEPS,
  OnboardCard,
  advanceGuide,
  createGuideState,
  finishGuide,
  formatAccelerator,
  isGuideVisible,
  observeAccessibilityTrust,
  skipGuideStep,
  type GuideState,
} from "./OnboardCard";

// ipc-client 的 wrapper 统一经 invoke 下发；桩掉它才能锚定命令名与返回字段名。
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

/** 把状态推到指定步骤（未授权路径），供渲染测试复用。 */function stateAt(step: GuideState["step"], accessibilityTrusted = false): GuideState {
  let state: GuideState = createGuideState(accessibilityTrusted);
  while (state.step !== step && isGuideVisible(state)) {
    state = advanceGuide(state);
  }
  return state;
}

/** 向导的完整 props（含 vi.fn() 回调），渲染与重绘共用同一份。 */
type GuideProps = ComponentProps<typeof OnboardCard>;

/** 渲染向导，返回 props 与 rerender 以便模拟「授权状态变化后重绘」。 */
function renderGuide(overrides: Partial<GuideProps> = {}) {
  const props: GuideProps = {
    state: createGuideState(false),
    launchOnLogin: true,
    onAdvance: vi.fn(),
    onSkip: vi.fn(),
    onDismiss: vi.fn(),
    onOpenSystemSettings: vi.fn(),
    onLaunchOnLoginChange: vi.fn(),
    ...overrides,
  };
  const view = render(<OnboardCard {...props} />);
  return { props, ...view };
}

describe("OnboardCard 状态机", () => {
  it("首次进入停在欢迎页并记录授权观测值", () => {
    const untrusted = createGuideState(false);
    const trusted = createGuideState(true);

    expect(untrusted.step).toBe("welcome");
    expect(trusted.step).toBe("welcome");
    expect(untrusted.accessibilityTrusted).toBe(false);
    expect(trusted.accessibilityTrusted).toBe(true);
    expect(isGuideVisible(untrusted)).toBe(true);
  });

  it("未授权时四步顺序推进，最后进入终态且不再渲染", () => {
    let state = createGuideState(false);

    state = advanceGuide(state);
    expect(state.step).toBe("hotkey");
    state = advanceGuide(state);
    expect(state.step).toBe("accessibility");
    state = advanceGuide(state);
    expect(state.step).toBe("autostart");

    state = advanceGuide(state);
    expect(state.step).toBe("done");
    expect(isGuideVisible(state)).toBe(false);
  });

  it("已授权时前进跳过辅助功能授权步骤", () => {
    let state = createGuideState(true);

    state = advanceGuide(state);
    expect(state.step).toBe("hotkey");

    state = advanceGuide(state);
    expect(state.step).toBe("autostart");
    expect(state.accessibilityTrusted).toBe(true);
  });

  it("授权从未授权变为已授权后，授权步骤自动收起并前进到自启确认", () => {
    // Arrange：未授权，停在授权步骤
    const atAccessibility = stateAt("accessibility", false);
    expect(atAccessibility.step).toBe("accessibility");

    // Act：观测到授权变为真
    const collapsed = observeAccessibilityTrust(atAccessibility, true);

    // Assert：授权步骤收起，直接到自启确认；观测值同步更新
    expect(collapsed.step).toBe("autostart");
    expect(collapsed.accessibilityTrusted).toBe(true);
    expect(isGuideVisible(collapsed)).toBe(true);
  });

  it("仍是未授权时观测不前进（负向对照）", () => {
    const atAccessibility = stateAt("accessibility", false);

    const observed = observeAccessibilityTrust(atAccessibility, false);

    expect(observed.step).toBe("accessibility");
    expect(observed.accessibilityTrusted).toBe(false);
  });

  it("在更早步骤观测到授权，后续前进同样跳过授权步骤", () => {
    const trustedAtWelcome = observeAccessibilityTrust(createGuideState(false), true);
    expect(trustedAtWelcome.step).toBe("welcome");

    const atHotkey = advanceGuide(trustedAtWelcome);
    expect(atHotkey.step).toBe("hotkey");

    expect(advanceGuide(atHotkey).step).toBe("autostart");
  });

  it("跳过：欢迎页不可跳过，中间步骤跳到下一步", () => {
    const atWelcome = createGuideState(false);
    expect(skipGuideStep(atWelcome)).toEqual(atWelcome);

    const atHotkey = advanceGuide(atWelcome);
    expect(skipGuideStep(atHotkey).step).toBe("accessibility");
    expect(skipGuideStep(skipGuideStep(atHotkey)).step).toBe("autostart");
  });

  it("结束后进入终态，继续前进也不会复活", () => {
    const finished = finishGuide(stateAt("hotkey"));

    expect(finished.step).toBe("done");
    expect(isGuideVisible(finished)).toBe(false);
    expect(advanceGuide(finished)).toEqual(finished);
    expect(skipGuideStep(finished)).toEqual(finished);
  });

  it("加速键展示：Tauri 口径转成面板符号，空串退回默认热键", () => {
    expect(formatAccelerator("CmdOrCtrl+Shift+C")).toBe("⌘⇧C");
    expect(formatAccelerator("CmdOrCtrl+Alt+Space")).toBe("⌘⌥Space");
    expect(formatAccelerator("")).toBe("⌘⇧C");
  });

  it("步骤码是与 Rust OnboardingStep::code 对齐的跨语言契约", () => {
    expect([...GUIDE_STEPS]).toEqual(["welcome", "hotkey", "accessibility", "autostart"]);
    expect(finishGuide(createGuideState(false)).step).toBe("done");
  });
});

describe("OnboardCard 渲染", () => {
  it("欢迎页渲染标题与步骤指示", () => {
    renderGuide();

    expect(screen.getByText("欢迎使用 QuickQuick")).toBeInTheDocument();
    expect(screen.getByText("第 1 / 4 步")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "首次引导" })).toBeInTheDocument();
  });

  it("点击「下一步」调用 onAdvance", async () => {
    const user = userEvent.setup();
    const onAdvance = vi.fn();
    renderGuide({ onAdvance });

    await user.click(screen.getByText("下一步"));
    expect(onAdvance).toHaveBeenCalledTimes(1);
  });
  it("点击「前往系统设置」调用 onOpenSystemSettings", async () => {
    const user = userEvent.setup();
    const onOpenSystemSettings = vi.fn();
    renderGuide({ onOpenSystemSettings });

    await user.click(screen.getByText("前往系统设置"));
    expect(onOpenSystemSettings).toHaveBeenCalledTimes(1);
  });

  it("授权步骤渲染深链入口与「稍后」，稍后调用 onSkip", async () => {
    const user = userEvent.setup();
    const onSkip = vi.fn();
    renderGuide({ state: stateAt("accessibility"), onSkip });

    expect(screen.getByText(/开启「辅助功能」/)).toBeInTheDocument();
    expect(screen.getByText("第 3 / 4 步")).toBeInTheDocument();

    await user.click(screen.getByText("稍后"));
    expect(onSkip).toHaveBeenCalledTimes(1);
  });

  it("自启确认步骤反映 launchOnLogin，切换时上报新值", async () => {
    const user = userEvent.setup();
    const onLaunchOnLoginChange = vi.fn();
    renderGuide({ state: stateAt("autostart"), launchOnLogin: true, onLaunchOnLoginChange });

    const checkbox = screen.getByRole("checkbox", { name: /登录时自动启动/ });
    expect(checkbox).toBeChecked();

    await user.click(checkbox);
    expect(onLaunchOnLoginChange).toHaveBeenCalledWith(false);
  });

  it("热键步骤展示传入的热键文本", () => {
    renderGuide({ state: stateAt("hotkey"), hotkeyLabel: "⌘⇧H" });

    expect(screen.getByText("⌘⇧H")).toBeInTheDocument();
  });

  it("点击关闭按钮（aria-label=关闭）调用 onDismiss", async () => {
    const user = userEvent.setup();
    const onDismiss = vi.fn();
    renderGuide({ onDismiss });

    await user.click(screen.getByRole("button", { name: "关闭" }));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it("辅助功能从未授权变为已授权后，授权请求不再渲染", () => {
    // Arrange：停在授权步骤，授权请求可见
    const atAccessibility = stateAt("accessibility", false);
    const { props, rerender } = renderGuide({ state: atAccessibility });
    expect(screen.getByText("前往系统设置")).toBeInTheDocument();
    expect(screen.getByText(/开启「辅助功能」/)).toBeInTheDocument();

    // Act：模拟系统授权从未授权变为已授权，父组件用观测后的状态重绘
    const granted = observeAccessibilityTrust(atAccessibility, true);
    rerender(<OnboardCard {...props} state={granted} />);

    // Assert：授权请求与深链按钮都不再渲染，向导已进到自启确认
    expect(screen.queryByText(/开启「辅助功能」/)).toBeNull();
    expect(screen.queryByText("前往系统设置")).toBeNull();
    expect(screen.getByText("确认开机自启")).toBeInTheDocument();
  });

  it("已授权时重绘不会让授权请求重新出现", () => {
    const granted = observeAccessibilityTrust(stateAt("accessibility", false), true);
    const { props, rerender } = renderGuide({ state: granted });

    // 再走一步（自启确认 → 终态）后授权请求依然不存在
    rerender(<OnboardCard {...props} state={finishGuide(granted)} />);

    expect(screen.queryByText(/开启「辅助功能」/)).toBeNull();
  });

  it("终态（done）不渲染任何内容", () => {
    const { container } = renderGuide({ state: finishGuide(createGuideState(false)) });

    expect(container.querySelector(".onboard")).toBeNull();
    expect(screen.queryByRole("region", { name: "首次引导" })).toBeNull();
  });
});

describe("引导状态 IPC 契约", () => {
  it("getOnboardingStatus 调 get_onboarding_status 并原样透出 action / capability", async () => {
    const mockInvoke = vi.mocked(invoke);
    // Rust 侧 OnboardingStatusDto 的字段与取值（tests/onboarding.rs 里有对应断言）
    mockInvoke.mockResolvedValue({ action: "proceed", capability: "full_paste" });

    const status = await getOnboardingStatus();

    expect(mockInvoke).toHaveBeenCalledWith("get_onboarding_status");
    expect(status).toEqual({ action: "proceed", capability: "full_paste" });
  });

  it("授权判定只认 action === \"proceed\"：两个取值分别对应收起与保持授权步骤", () => {
    const proceed: OnboardingStatus = { action: "proceed", capability: "full_paste" };
    const untrusted: OnboardingStatus = {
      action: "show_accessibility_card",
      capability: "write_back_only",
    };

    const atAccessibility = stateAt("accessibility", false);

    expect(
      observeAccessibilityTrust(atAccessibility, proceed.action === "proceed").step,
    ).toBe("autostart");
    expect(
      observeAccessibilityTrust(atAccessibility, untrusted.action === "proceed").step,
    ).toBe("accessibility");
  });
});
