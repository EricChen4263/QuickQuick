import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import MiniTranslate, { fallbackHint, sourceLabel } from "./MiniTranslate";
import type { TranslateResult } from "../ipc/ipc-client";

const RESULT: TranslateResult = {
  kind: "plain",
  translated: "你好，世界",
  sourceLang: "en",
  targetLang: "zh",
};

/** 词典结果：translated 是纯文本摘要（回退 / 复制用），结构内容都在 entry 里。 */
const DICT_RESULT: TranslateResult = {
  kind: "dict",
  translated: "hello int. 哈罗，喂；你好 n. 招呼声",
  sourceLang: "en",
  targetLang: "zh",
  entry: {
    phonetic: "/həˈləʊ/",
    definitions: [
      { pos: "int.", meanings: ["哈罗，喂", "你好"] },
      { pos: "n.", meanings: ["招呼声"] },
    ],
    examples: ["Hello, how are you?"],
    audio: "https://example.com/hello.mp3",
    inflections: ["hellos"],
  },
};

describe("MiniTranslate 渲染", () => {
  it("渲染翻译方向行 sourceLang → targetLang", () => {
    render(
      <MiniTranslate
        result={RESULT}
        source="selection"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(screen.getByText("en → zh")).toBeDefined();
  });

  it("渲染译文内容", () => {
    render(
      <MiniTranslate
        result={RESULT}
        source="selection"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(screen.getByText("你好，世界")).toBeDefined();
  });

  it("选中来源 → 显示「来源：选中」标签（D4）", () => {
    render(
      <MiniTranslate
        result={RESULT}
        source="selection"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(screen.getByText("来源：选中")).toBeDefined();
  });

  it("剪贴板来源 → 显示「来源：剪贴板」标签（D4）", () => {
    render(
      <MiniTranslate
        result={RESULT}
        source="clipboard"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(screen.getByText("来源：剪贴板")).toBeDefined();
  });

  it("未授权降级 → 显示「未授权辅助功能，已改为译剪贴板内容」", () => {
    render(
      <MiniTranslate
        result={RESULT}
        source="clipboard"
        fallbackReason="untrusted"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(
      screen.getByText("未授权辅助功能，已改为译剪贴板内容"),
    ).toBeDefined();
  });

  it("平台不支持取词 → 显示「当前系统不支持选中取词」说明", () => {
    render(
      <MiniTranslate
        result={RESULT}
        source="clipboard"
        fallbackReason="unsupported"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(
      screen.getByText("当前系统不支持选中取词，已改为译剪贴板内容"),
    ).toBeDefined();
  });

  it("无选中（常态）→ 不渲染降级说明，避免噪音", () => {
    render(
      <MiniTranslate
        result={RESULT}
        source="clipboard"
        fallbackReason="noSelection"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(screen.queryByText(/已改为译剪贴板内容/)).toBeNull();
  });

  it("点复制按钮调 onCopy 回调", async () => {
    const onCopy = vi.fn();

    render(
      <MiniTranslate
        result={RESULT}
        source="selection"
        onCopy={onCopy}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "复制" }));

    expect(onCopy).toHaveBeenCalledTimes(1);
  });

  it("点朗读按钮调 onSpeak 回调", async () => {
    const onSpeak = vi.fn();

    render(
      <MiniTranslate
        result={RESULT}
        source="selection"
        onCopy={vi.fn()}
        onSpeak={onSpeak}
        onExpand={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "朗读" }));

    expect(onSpeak).toHaveBeenCalledTimes(1);
  });

  it("点展开按钮调 onExpand 回调", async () => {
    const onExpand = vi.fn();

    render(
      <MiniTranslate
        result={RESULT}
        source="selection"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={onExpand}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "展开" }));

    expect(onExpand).toHaveBeenCalledTimes(1);
  });
});

describe("MiniTranslate 词典结果", () => {
  it("dict 结果渲染音标与按词性分组的释义", () => {
    render(
      <MiniTranslate
        result={DICT_RESULT}
        source="selection"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(screen.getByTestId("dict-phonetic")).toHaveTextContent("/həˈləʊ/");
    expect(screen.getByText("int.")).toBeInTheDocument();
    expect(screen.getByText("哈罗，喂")).toBeInTheDocument();
    expect(screen.getByText("你好")).toBeInTheDocument();
    expect(screen.getByText("n.")).toBeInTheDocument();
    expect(screen.getByText("招呼声")).toBeInTheDocument();
  });

  it("dict 结果渲染例句、变形与发音，且不重复展示纯文本摘要", () => {
    render(
      <MiniTranslate
        result={DICT_RESULT}
        source="selection"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(screen.getByText("Hello, how are you?")).toBeInTheDocument();
    expect(screen.getByTestId("dict-inflections")).toHaveTextContent("变形");
    expect(screen.getByTestId("dict-inflections")).toHaveTextContent("hellos");
    expect(screen.getByTestId("dict-audio")).toHaveAttribute(
      "src",
      "https://example.com/hello.mp3",
    );
    // 摘要是复制 / 朗读用的回退文本，结构已表达同一信息，浮窗里不再重复占位
    expect(
      screen.queryByText("hello int. 哈罗，喂；你好 n. 招呼声"),
    ).toBeNull();
  });

  it("plain 结果仍只渲染 translated 纯文本，不渲染词典结构", () => {
    const { container } = render(
      <MiniTranslate
        result={RESULT}
        source="selection"
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(container.querySelector(".mini-body")).toHaveTextContent("你好，世界");
    expect(container.querySelector(".dict-entry")).toBeNull();
    expect(screen.queryByTestId("dict-phonetic")).toBeNull();
    expect(screen.queryByTestId("dict-audio")).toBeNull();
  });

  it("dict 结果下复制 / 朗读 / 展开按钮仍各自触发回调", async () => {
    const onCopy = vi.fn();
    const onSpeak = vi.fn();
    const onExpand = vi.fn();

    render(
      <MiniTranslate
        result={DICT_RESULT}
        source="selection"
        onCopy={onCopy}
        onSpeak={onSpeak}
        onExpand={onExpand}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "复制" }));
    await userEvent.click(screen.getByRole("button", { name: "朗读" }));
    await userEvent.click(screen.getByRole("button", { name: "展开" }));

    expect(onCopy).toHaveBeenCalledTimes(1);
    expect(onSpeak).toHaveBeenCalledTimes(1);
    expect(onExpand).toHaveBeenCalledTimes(1);
  });
});

describe("来源与降级文案映射", () => {
  it("sourceLabel 覆盖两个来源", () => {
    expect(sourceLabel("selection")).toBe("选中");
    expect(sourceLabel("clipboard")).toBe("剪贴板");
  });

  it("fallbackHint 只为取词通道不可用给文案，无选中与空值不给", () => {
    expect(fallbackHint("untrusted")).toBe("未授权辅助功能，已改为译剪贴板内容");
    expect(fallbackHint("unsupported")).toBe(
      "当前系统不支持选中取词，已改为译剪贴板内容",
    );
    expect(fallbackHint("noSelection")).toBeUndefined();
    expect(fallbackHint(null)).toBeUndefined();
  });
});
