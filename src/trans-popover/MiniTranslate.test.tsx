import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import MiniTranslate from "./MiniTranslate";
import type { TranslateResult } from "../ipc/ipc-client";

const RESULT: TranslateResult = {
  kind: "plain",
  translated: "你好，世界",
  sourceLang: "en",
  targetLang: "zh",
};

describe("MiniTranslate 渲染", () => {
  it("渲染翻译方向行 sourceLang → targetLang", () => {
    render(
      <MiniTranslate
        result={RESULT}
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
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={vi.fn()}
      />,
    );

    expect(screen.getByText("你好，世界")).toBeDefined();
  });

  it("点复制按钮调 onCopy 回调", async () => {
    const onCopy = vi.fn();

    render(
      <MiniTranslate
        result={RESULT}
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
        onCopy={vi.fn()}
        onSpeak={vi.fn()}
        onExpand={onExpand}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "展开" }));

    expect(onExpand).toHaveBeenCalledTimes(1);
  });
});
