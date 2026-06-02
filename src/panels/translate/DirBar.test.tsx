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
  it("渲染源语和目标语两个 select", () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    expect(screen.getByRole("combobox", { name: "源语言" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "目标语言" })).toBeInTheDocument();
  });

  it("源语 select 含自动检测选项，目标语 select 不含自动检测", () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    const sourceSelect = screen.getByRole("combobox", { name: "源语言" });
    const targetSelect = screen.getByRole("combobox", { name: "目标语言" });

    // 源语下拉的 option 列表里应包含 "自动检测"
    const sourceOptions = Array.from(sourceSelect.querySelectorAll("option")).map((o) => o.textContent);
    expect(sourceOptions).toContain("自动检测");

    // 目标语下拉不含 "自动检测"
    const targetOptions = Array.from(targetSelect.querySelectorAll("option")).map((o) => o.textContent);
    expect(targetOptions).not.toContain("自动检测");
  });

  it("改变源语 select 触发 onSourceChange 带新 code", async () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    const user = userEvent.setup();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    const sourceSelect = screen.getByRole("combobox", { name: "源语言" });
    await user.selectOptions(sourceSelect, "en");

    expect(onSourceChange).toHaveBeenCalledWith("en");
  });

  it("改变目标语 select 触发 onTargetChange 带新 code", async () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    const user = userEvent.setup();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    const targetSelect = screen.getByRole("combobox", { name: "目标语言" });
    await user.selectOptions(targetSelect, "en");

    expect(onTargetChange).toHaveBeenCalledWith("en");
  });

  it("provider select 仍在且功能正常", () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ });
    expect(select).toBeInTheDocument();
    expect(screen.getByText("MyMemory · 默认")).toBeInTheDocument();
    expect(screen.getByText("百度翻译")).toBeInTheDocument();
  });

  it("renders provider select with all provider options", () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ });
    expect(select).toBeInTheDocument();
    expect(screen.getByText("DeepL Free")).toBeInTheDocument();
  });

  it("provider select value reflects selectedProviderId", () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="baidu"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ }) as HTMLSelectElement;
    expect(select.value).toBe("baidu");
  });

  it("calls onProviderChange with new id when select changes", async () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    const user = userEvent.setup();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ });
    await user.selectOptions(select, "deepl");

    expect(onProviderChange).toHaveBeenCalledWith("deepl");
  });

  it("renders empty providers list without crashing", () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={[]}
        selectedProviderId=""
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ });
    expect(select).toBeInTheDocument();
  });

  it("needsKey=true 的 option 带 disabled 属性，needsKey=false 的不带", () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={PROVIDERS_WITH_NEEDS_KEY}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
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
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={PROVIDERS_WITH_NEEDS_KEY}
        selectedProviderId="baidu"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    const select = screen.getByRole("combobox", { name: /翻译源/ }) as HTMLSelectElement;
    expect(select.value).toBe("baidu");
  });

  it("不再渲染交换语言方向按钮", () => {
    const onProviderChange = vi.fn();
    const onSourceChange = vi.fn();
    const onTargetChange = vi.fn();
    render(
      <DirBar
        sourceLang="auto"
        targetLang="zh"
        providers={MOCK_PROVIDERS}
        selectedProviderId="mymemory"
        onProviderChange={onProviderChange}
        onSourceChange={onSourceChange}
        onTargetChange={onTargetChange}
      />
    );

    expect(screen.queryByRole("button", { name: "交换语言方向" })).not.toBeInTheDocument();
  });
});
