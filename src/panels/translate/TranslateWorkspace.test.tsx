import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import TranslateWorkspace from "./TranslateWorkspace";
import type {
  TranslatePlainResult,
  TranslateDictResult,
  Provider,
} from "../../ipc/ipc-client";

const PROVIDERS: Provider[] = [
  { id: "google", name: "Google", needsKey: false, needsConfig: false, isUnofficial: false },
];

const PLAIN_RESULT: TranslatePlainResult = {
  kind: "plain",
  translated: "你好，世界",
  sourceLang: "en",
  targetLang: "zh",
};

const DICT_RESULT: TranslateDictResult = {
  kind: "dict",
  translated: "hello 摘要文本",
  sourceLang: "en",
  targetLang: "zh",
  entry: {
    phonetic: "/həˈləʊ/",
    definitions: [{ pos: "int.", meanings: ["你好"] }],
    examples: ["Hello there"],
    audio: null,
    inflections: [],
  },
};

function renderWorkspace(result: TranslatePlainResult | TranslateDictResult) {
  return render(
    <TranslateWorkspace
      inputText="hello"
      result={result}
      isLoading={false}
      error={null}
      sourceLang="en"
      targetLang="zh"
      providers={PROVIDERS}
      selectedProviderId="google"
      configuredIds={new Set<string>()}
      onInputChange={vi.fn()}
      onTranslate={vi.fn()}
      onSourceChange={vi.fn()}
      onTargetChange={vi.fn()}
      onAction={vi.fn()}
      onProviderChange={vi.fn()}
    />
  );
}

describe("TranslateWorkspace 结果分流", () => {
  it("plain_result_renders_translated_text", () => {
    renderWorkspace(PLAIN_RESULT);

    // Plain 走原译文渲染，不回归
    expect(screen.getByText("你好，世界")).toBeInTheDocument();
    // 不渲染词典组件
    expect(screen.queryByTestId("dict-phonetic")).not.toBeInTheDocument();
  });

  it("dict 结果渲染词典展示组件而非纯译文", () => {
    renderWorkspace(DICT_RESULT);

    // Dict 走 DictEntryView：音标 + 释义
    expect(screen.getByTestId("dict-phonetic")).toHaveTextContent("/həˈləʊ/");
    expect(screen.getByText("你好")).toBeInTheDocument();
    expect(screen.getByText("Hello there")).toBeInTheDocument();
  });
});
