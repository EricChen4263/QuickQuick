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
});
