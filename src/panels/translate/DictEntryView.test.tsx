import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import DictEntryView from "./DictEntryView";
import type { DictEntry } from "../../ipc/ipc-client";

const FULL_ENTRY: DictEntry = {
  phonetic: "/həˈləʊ/",
  definitions: [
    { pos: "int.", meanings: ["哈罗，喂", "你好"] },
    { pos: "n.", meanings: ["招呼声"] },
  ],
  examples: ["Hello, how are you?", "She said hello to me."],
  audio: "https://example.com/hello.mp3",
  inflections: ["hellos", "helloed"],
};

describe("DictEntryView", () => {
  it("dict_result_renders_phonetic_and_definitions", () => {
    render(<DictEntryView entry={FULL_ENTRY} />);

    // 音标
    expect(screen.getByText("/həˈləʊ/")).toBeInTheDocument();
    // 按词性分组释义：词性标签 + 释义文本
    expect(screen.getByText("int.")).toBeInTheDocument();
    expect(screen.getByText("哈罗，喂")).toBeInTheDocument();
    expect(screen.getByText("你好")).toBeInTheDocument();
    expect(screen.getByText("n.")).toBeInTheDocument();
    expect(screen.getByText("招呼声")).toBeInTheDocument();
  });

  it("dict_component_renders_examples_and_audio", () => {
    render(<DictEntryView entry={FULL_ENTRY} />);

    // 例句
    expect(screen.getByText("Hello, how are you?")).toBeInTheDocument();
    expect(screen.getByText("She said hello to me.")).toBeInTheDocument();
    // 发音入口：可播放音频元素，src 指向音频地址
    const audio = screen.getByTestId("dict-audio") as HTMLAudioElement;
    expect(audio).toBeInTheDocument();
    expect(audio).toHaveAttribute("src", "https://example.com/hello.mp3");
  });

  it("无音频时不渲染发音入口", () => {
    const entry: DictEntry = { ...FULL_ENTRY, audio: null };
    render(<DictEntryView entry={entry} />);

    expect(screen.queryByTestId("dict-audio")).not.toBeInTheDocument();
  });

  it("无音标/无例句/无变形时不渲染对应区块", () => {
    const entry: DictEntry = {
      phonetic: null,
      definitions: [{ pos: null, meanings: ["仅释义"] }],
      examples: [],
      audio: null,
      inflections: [],
    };
    render(<DictEntryView entry={entry} />);

    expect(screen.queryByTestId("dict-phonetic")).not.toBeInTheDocument();
    expect(screen.queryByTestId("dict-examples")).not.toBeInTheDocument();
    expect(screen.queryByTestId("dict-inflections")).not.toBeInTheDocument();
    // 释义仍渲染（无词性时不渲染 pos 标签）
    expect(screen.getByText("仅释义")).toBeInTheDocument();
    expect(screen.queryByTestId("dict-pos")).not.toBeInTheDocument();
  });
});
