import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import DirBar from "./DirBar";
import type { Provider } from "../../ipc/ipc-client";

const MOCK_PROVIDERS: Provider[] = [
  { id: "mymemory", name: "MyMemory · 默认", needsKey: false },
  { id: "baidu", name: "百度翻译", needsKey: true },
  { id: "deepl", name: "DeepL Free", needsKey: false },
];

const PROVIDERS_WITH_NEEDS_KEY: Provider[] = [
  { id: "mymemory", name: "MyMemory · 默认", needsKey: false },
  { id: "baidu", name: "百度翻译", needsKey: true },
  { id: "deepl-free", name: "DeepL Free", needsKey: true },
];

describe("DirBar", () => {
  it("renders language direction pill with sourceLang and targetLang", () => {
    const onProviderChange = vi.fn();
    render(
      <DirBar
        sourceLang="en"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
      />
    );

    expect(screen.getByText(/en/)).toBeInTheDocument();
    expect(screen.getByText(/zh/)).toBeInTheDocument();
  });

  it("renders provider select with all provider options", () => {
    const onProviderChange = vi.fn();
    render(
      <DirBar
        sourceLang="en"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ });
    expect(select).toBeInTheDocument();
    expect(screen.getByText("MyMemory · 默认")).toBeInTheDocument();
    expect(screen.getByText("百度翻译")).toBeInTheDocument();
    expect(screen.getByText("DeepL Free")).toBeInTheDocument();
  });

  it("provider select value reflects selectedProviderId", () => {
    const onProviderChange = vi.fn();
    render(
      <DirBar
        sourceLang="en"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="baidu"
        onProviderChange={onProviderChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ }) as HTMLSelectElement;
    expect(select.value).toBe("baidu");
  });

  it("calls onProviderChange with new id when select changes", async () => {
    const onProviderChange = vi.fn();
    const user = userEvent.setup();
    render(
      <DirBar
        sourceLang="en"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ });
    await user.selectOptions(select, "deepl");

    expect(onProviderChange).toHaveBeenCalledWith("deepl");
  });

  it("renders empty providers list without crashing", () => {
    const onProviderChange = vi.fn();
    render(
      <DirBar
        sourceLang="en"
        targetLang="zh"
        providers={[]}
        selectedProviderId=""
        onProviderChange={onProviderChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ });
    expect(select).toBeInTheDocument();
  });

  it("needsKey=true 的 option 带 disabled 属性，needsKey=false 的不带", () => {
    const onProviderChange = vi.fn();
    render(
      <DirBar
        sourceLang="en"
        targetLang="zh"
        providers={PROVIDERS_WITH_NEEDS_KEY}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
      />
    );

    const mymemoryOption = screen.getByRole("option", { name: "MyMemory · 默认" }) as HTMLOptionElement;
    const baiduOption = screen.getByRole("option", { name: "百度翻译" }) as HTMLOptionElement;
    const deeplOption = screen.getByRole("option", { name: "DeepL Free" }) as HTMLOptionElement;

    expect(mymemoryOption.disabled).toBe(false);
    expect(baiduOption.disabled).toBe(true);
    expect(deeplOption.disabled).toBe(true);
  });

  it("selectedProviderId 为 needsKey 源时 select 正确显示当前值且不崩", () => {
    const onProviderChange = vi.fn();
    render(
      <DirBar
        sourceLang="en"
        targetLang="zh"
        providers={PROVIDERS_WITH_NEEDS_KEY}
        selectedProviderId="baidu"
        onProviderChange={onProviderChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ }) as HTMLSelectElement;
    expect(select.value).toBe("baidu");
  });

  it("swap 按钮存在且有 aria-label 交换语言方向", () => {
    const onProviderChange = vi.fn();
    const onSwap = vi.fn();
    render(
      <DirBar
        sourceLang="en"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSwap={onSwap}
      />
    );

    const swapBtn = screen.getByRole("button", { name: "交换语言方向" });
    expect(swapBtn).toBeInTheDocument();
    expect(swapBtn).not.toBeDisabled();
  });

  it("swap 按钮点击时触发 onSwap 回调", async () => {
    const onProviderChange = vi.fn();
    const onSwap = vi.fn();
    const user = userEvent.setup();
    render(
      <DirBar
        sourceLang="en"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSwap={onSwap}
      />
    );

    const swapBtn = screen.getByRole("button", { name: "交换语言方向" });
    await user.click(swapBtn);

    expect(onSwap).toHaveBeenCalledTimes(1);
  });

  it("sourceLang 为 auto 时 swap 按钮禁用", () => {
    const onProviderChange = vi.fn();
    const onSwap = vi.fn();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSwap={onSwap}
      />
    );

    const swapBtn = screen.getByRole("button", { name: "交换语言方向" });
    expect(swapBtn).toBeDisabled();
  });

  it("sourceLang 为空字符串时 swap 按钮禁用", () => {
    const onProviderChange = vi.fn();
    const onSwap = vi.fn();
    render(
      <DirBar
        sourceLang=""
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSwap={onSwap}
      />
    );

    const swapBtn = screen.getByRole("button", { name: "交换语言方向" });
    expect(swapBtn).toBeDisabled();
  });

  it("未传 onSwap 时 swap 按钮禁用", () => {
    const onProviderChange = vi.fn();
    render(
      <DirBar
        sourceLang="en"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
      />
    );

    const swapBtn = screen.getByRole("button", { name: "交换语言方向" });
    expect(swapBtn).toBeDisabled();
  });
});
