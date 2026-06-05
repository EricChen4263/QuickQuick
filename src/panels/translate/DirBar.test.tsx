import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import DirBar from "./DirBar";
import type { Provider } from "../../ipc/ipc-client";

// 本组件级测试只验证「列出/禁用/选中」行为，与非官方标注无关，
// 故 isUnofficial 统一置 false 避免标注后缀污染 option 的精确 name 匹配；
// 标注渲染由 label-degrade.test.tsx 用独立夹具覆盖。
const MOCK_PROVIDERS: Provider[] = [
  { id: "mymemory", name: "MyMemory · 默认", needsKey: false, isUnofficial: false },
  { id: "baidu", name: "百度翻译", needsKey: true, isUnofficial: false },
  { id: "deepl", name: "DeepL Free", needsKey: false, isUnofficial: false },
];

const PROVIDERS_WITH_NEEDS_KEY: Provider[] = [
  { id: "mymemory", name: "MyMemory · 默认", needsKey: false, isUnofficial: false },
  { id: "baidu", name: "百度翻译", needsKey: true, isUnofficial: false },
  { id: "deepl-free", name: "DeepL Free", needsKey: true, isUnofficial: false },
];

/** 默认 props 工厂：减少各用例重复装配 onXxx mock */
function renderDirBar(overrides: Partial<React.ComponentProps<typeof DirBar>> = {}) {
  const props = {
    sourceLang: "auto",
    targetLang: "zh",
    providers: MOCK_PROVIDERS,
    selectedProviderId: "mymemory",
    onProviderChange: vi.fn(),
    onSourceChange: vi.fn(),
    onTargetChange: vi.fn(),
    ...overrides,
  };
  render(<DirBar {...props} />);
  return props;
}

describe("DirBar", () => {
  it("渲染源语和目标语两个下拉 trigger", () => {
    renderDirBar();

    expect(screen.getByRole("button", { name: "源语言" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "目标语言" })).toBeInTheDocument();
  });

  it("源语与目标语之间渲染方向箭头：装饰性、aria-hidden、不可交互", () => {
    const { container } = render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={vi.fn()}
        onSourceChange={vi.fn()}
        onTargetChange={vi.fn()}
      />
    );

    // 箭头是 .lang-selects 内的装饰 svg，标 aria-hidden、非 button（不进无障碍树、不可点）
    const arrow = container.querySelector(".lang-selects .lang-dir-arrow");
    expect(arrow).not.toBeNull();
    expect(arrow).toHaveAttribute("aria-hidden", "true");
    expect(screen.queryByRole("button", { name: "交换语言方向" })).not.toBeInTheDocument();
  });

  it("源语下拉含自动检测选项，目标语下拉不含自动检测", async () => {
    const user = userEvent.setup();
    renderDirBar();

    // 展开源语下拉，应含"自动检测"
    await user.click(screen.getByRole("button", { name: "源语言" }));
    expect(screen.getByRole("option", { name: "自动检测" })).toBeInTheDocument();
    await user.keyboard("{Escape}");

    // 展开目标语下拉，不应含"自动检测"
    await user.click(screen.getByRole("button", { name: "目标语言" }));
    expect(screen.queryByRole("option", { name: "自动检测" })).not.toBeInTheDocument();
  });

  it("选源语选项触发 onSourceChange 带新 code", async () => {
    const user = userEvent.setup();
    const { onSourceChange } = renderDirBar();

    await user.click(screen.getByRole("button", { name: "源语言" }));
    await user.click(screen.getByRole("option", { name: "英文" }));

    expect(onSourceChange).toHaveBeenCalledWith("en");
  });

  it("选目标语选项触发 onTargetChange 带新 code", async () => {
    const user = userEvent.setup();
    const { onTargetChange } = renderDirBar();

    await user.click(screen.getByRole("button", { name: "目标语言" }));
    await user.click(screen.getByRole("option", { name: "英文" }));

    expect(onTargetChange).toHaveBeenCalledWith("en");
  });

  it("翻译源下拉仍在且列出全部 provider", async () => {
    const user = userEvent.setup();
    renderDirBar();

    const trigger = screen.getByRole("button", { name: "翻译源" });
    expect(trigger).toBeInTheDocument();
    // trigger 显示当前选中 provider 名
    expect(trigger).toHaveTextContent("MyMemory · 默认");

    await user.click(trigger);
    expect(screen.getByRole("option", { name: "百度翻译" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "DeepL Free" })).toBeInTheDocument();
  });

  it("翻译源 trigger 文案反映 selectedProviderId", () => {
    renderDirBar({ selectedProviderId: "baidu" });

    expect(screen.getByRole("button", { name: "翻译源" })).toHaveTextContent("百度翻译");
  });

  it("选翻译源选项触发 onProviderChange 带新 id", async () => {
    const user = userEvent.setup();
    const { onProviderChange } = renderDirBar();

    await user.click(screen.getByRole("button", { name: "翻译源" }));
    await user.click(screen.getByRole("option", { name: "DeepL Free" }));

    expect(onProviderChange).toHaveBeenCalledWith("deepl");
  });

  it("providers 为空时翻译源 trigger 仍渲染不崩", () => {
    renderDirBar({ providers: [], selectedProviderId: "" });

    expect(screen.getByRole("button", { name: "翻译源" })).toBeInTheDocument();
  });

  it("needsKey 源未配置时 option 标记 aria-disabled，needsKey=false 的不标", async () => {
    const user = userEvent.setup();
    renderDirBar({ providers: PROVIDERS_WITH_NEEDS_KEY });

    await user.click(screen.getByRole("button", { name: "翻译源" }));
    expect(screen.getByRole("option", { name: "MyMemory · 默认" })).toHaveAttribute("aria-disabled", "false");
    expect(screen.getByRole("option", { name: "百度翻译" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("option", { name: "DeepL Free" })).toHaveAttribute("aria-disabled", "true");
  });

  it("selectedProviderId 为 needsKey 源时 trigger 正确显示当前值且不崩", () => {
    renderDirBar({ providers: PROVIDERS_WITH_NEEDS_KEY, selectedProviderId: "baidu" });

    expect(screen.getByRole("button", { name: "翻译源" })).toHaveTextContent("百度翻译");
  });

  it("不再渲染交换语言方向按钮", () => {
    renderDirBar();

    expect(screen.queryByRole("button", { name: "交换语言方向" })).not.toBeInTheDocument();
  });

  it("①: needsKey=true 且在 configuredIds 中 → option 不 disabled", async () => {
    const user = userEvent.setup();
    renderDirBar({ providers: PROVIDERS_WITH_NEEDS_KEY, configuredIds: new Set(["baidu"]) });

    await user.click(screen.getByRole("button", { name: "翻译源" }));
    expect(screen.getByRole("option", { name: "百度翻译" })).toHaveAttribute("aria-disabled", "false");
  });

  it("①: needsKey=true 且不在 configuredIds 中 → option disabled", async () => {
    const user = userEvent.setup();
    renderDirBar({ providers: PROVIDERS_WITH_NEEDS_KEY, configuredIds: new Set<string>() });

    await user.click(screen.getByRole("button", { name: "翻译源" }));
    expect(screen.getByRole("option", { name: "百度翻译" })).toHaveAttribute("aria-disabled", "true");
  });

  it("①: needsKey=false 的 option 无论 configuredIds → 不 disabled", async () => {
    const user = userEvent.setup();
    renderDirBar({ providers: PROVIDERS_WITH_NEEDS_KEY, configuredIds: new Set<string>() });

    await user.click(screen.getByRole("button", { name: "翻译源" }));
    expect(screen.getByRole("option", { name: "MyMemory · 默认" })).toHaveAttribute("aria-disabled", "false");
  });

  it("点击 disabled 翻译源 option 不触发 onProviderChange", async () => {
    const user = userEvent.setup();
    const { onProviderChange } = renderDirBar({
      providers: PROVIDERS_WITH_NEEDS_KEY,
      configuredIds: new Set<string>(),
    });

    await user.click(screen.getByRole("button", { name: "翻译源" }));
    await user.click(screen.getByRole("option", { name: "百度翻译" }));

    expect(onProviderChange).not.toHaveBeenCalled();
  });
});
