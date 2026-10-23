import { describe, it, expect } from "vitest";
import {
  moveHighlight,
  quickSelectIndex,
  resolveEnter,
  isImeComposing,
  directPickIndex,
  isDirectPickAllowed,
  applyDeleteWord,
  applyDeleteChar,
  ctrlMoveDirection,
  firstIndex,
  lastIndex,
} from "./keyboard";
import type { HistoryItem } from "./search";

// P2-2 键盘模型：vim 风格移动键映射
describe("ctrlMoveDirection", () => {
  it("⌃N / ⌃J 映射为下移", () => {
    expect(ctrlMoveDirection("n")).toBe("ArrowDown");
    expect(ctrlMoveDirection("j")).toBe("ArrowDown");
  });

  it("⌃P / ⌃K 映射为上移", () => {
    expect(ctrlMoveDirection("p")).toBe("ArrowUp");
    expect(ctrlMoveDirection("k")).toBe("ArrowUp");
  });

  it("非移动键返回 null", () => {
    expect(ctrlMoveDirection("w")).toBeNull();
    expect(ctrlMoveDirection("ArrowDown")).toBeNull();
    expect(ctrlMoveDirection("")).toBeNull();
  });
});

describe("firstIndex / lastIndex", () => {
  it("非空列表首条为 0、末条为 count-1", () => {
    expect(firstIndex(4)).toBe(0);
    expect(lastIndex(4)).toBe(3);
  });

  it("单条列表首末都是 0", () => {
    expect(firstIndex(1)).toBe(0);
    expect(lastIndex(1)).toBe(0);
  });

  it("空列表返回 -1（无有效条目）", () => {
    expect(firstIndex(0)).toBe(-1);
    expect(lastIndex(0)).toBe(-1);
  });
});

// P2-2 ⌃W：按空白/标点切词，删除光标前一个词
describe("applyDeleteWord", () => {
  it("光标在末尾时删掉最后一个英文单词", () => {
    // Arrange & Act
    const edit = applyDeleteWord("hello world", 11);
    // Assert
    expect(edit).toEqual({ value: "hello ", caret: 6 });
  });

  it("光标在中间时只删光标前的词，光标后内容保留", () => {
    // Arrange & Act：光标落在 "hello" 之后
    const edit = applyDeleteWord("hello world", 5);
    // Assert
    expect(edit).toEqual({ value: " world", caret: 0 });
  });

  it("光标在开头时原样返回", () => {
    // Arrange & Act
    const edit = applyDeleteWord("hello world", 0);
    // Assert
    expect(edit).toEqual({ value: "hello world", caret: 0 });
  });

  it("末尾连续空白一并吃掉（readline 语义）", () => {
    // Arrange & Act
    const edit = applyDeleteWord("hello world   ", 14);
    // Assert
    expect(edit).toEqual({ value: "hello ", caret: 6 });
  });

  it("中文按标点切词：只删「世界」，保留前面的逗号", () => {
    // Arrange & Act
    const edit = applyDeleteWord("你好，世界", 5);
    // Assert
    expect(edit).toEqual({ value: "你好，", caret: 3 });
  });

  it("中英混排：删除光标前的中文段", () => {
    // Arrange & Act
    const edit = applyDeleteWord("abc 中文", 6);
    // Assert
    expect(edit).toEqual({ value: "abc ", caret: 4 });
  });

  it("整段都是分隔符时全部删光，光标落到 0", () => {
    // Arrange & Act
    const edit = applyDeleteWord("   ", 3);
    // Assert
    expect(edit).toEqual({ value: "", caret: 0 });
  });

  it("光标越界时按内容长度收敛", () => {
    // Arrange & Act：caret 远大于长度
    const edit = applyDeleteWord("hello", 99);
    // Assert
    expect(edit).toEqual({ value: "", caret: 0 });
  });
});

// P2-2 ⌃H：删除光标前一个字符（等价 Backspace）
describe("applyDeleteChar", () => {
  it("光标在中间时删掉光标前那个字符", () => {
    // Arrange & Act
    const edit = applyDeleteChar("abcd", 2);
    // Assert：删掉 "b" → "acd"，光标停在 1
    expect(edit).toEqual({ value: "acd", caret: 1 });
  });

  it("光标在末尾时删掉最后一个字符", () => {
    const edit = applyDeleteChar("abcd", 4);
    expect(edit).toEqual({ value: "abc", caret: 3 });
  });

  it("光标在开头时原样返回", () => {
    const edit = applyDeleteChar("abcd", 0);
    expect(edit).toEqual({ value: "abcd", caret: 0 });
  });

  it("代理对（emoji）整对删除，不留半个码元", () => {
    // Arrange："a😀" 共 3 个 UTF-16 码元
    const edit = applyDeleteChar("a😀", 3);
    // Assert
    expect(edit).toEqual({ value: "a", caret: 1 });
  });

  it("光标紧跟在单个普通字符后：不误删前面的代理对", () => {
    // Arrange：caret=1 表示删除 "a"
    const edit = applyDeleteChar("a😀", 1);
    // Assert
    expect(edit).toEqual({ value: "😀", caret: 0 });
  });

  it("光标越界时按内容长度收敛", () => {
    const edit = applyDeleteChar("abcd", 99);
    expect(edit).toEqual({ value: "abc", caret: 3 });
  });
});

// P2-3 数字直达：1–9 → 第 1–9 条，0 → 第 10 条，序号越界返回 null
describe("directPickIndex", () => {
  it("'0' 映射到第 10 条（索引 9）", () => {
    expect(directPickIndex("0", 10)).toBe(9);
  });

  it("'1' 映射到索引 0，'9' 映射到索引 8", () => {
    expect(directPickIndex("1", 10)).toBe(0);
    expect(directPickIndex("9", 10)).toBe(8);
  });

  it("条目数不足该序号时返回 null（不降级取最后一条）", () => {
    // 只有 3 条时，第 5 条与第 10 条都不存在
    expect(directPickIndex("5", 3)).toBeNull();
    expect(directPickIndex("0", 3)).toBeNull();
    // 边界：恰好够
    expect(directPickIndex("3", 3)).toBe(2);
  });

  it("空列表一律返回 null", () => {
    expect(directPickIndex("1", 0)).toBeNull();
    expect(directPickIndex("0", 0)).toBeNull();
  });

  it("非单个数字字符返回 null", () => {
    expect(directPickIndex("a", 10)).toBeNull();
    expect(directPickIndex("12", 10)).toBeNull();
    expect(directPickIndex("", 10)).toBeNull();
  });
});

// P2-3 前置条件：搜索框为空 + 无任何修饰键
describe("isDirectPickAllowed", () => {
  it("搜索框为空且无修饰键时允许数字直达", () => {
    expect(isDirectPickAllowed("", {})).toBe(true);
  });

  it("搜索框非空时不允许（数字要能正常输入用于搜索）", () => {
    expect(isDirectPickAllowed("2", {})).toBe(false);
  });

  it("带修饰键时不允许（⌘1-9 等组合有自己的语义）", () => {
    expect(isDirectPickAllowed("", { metaKey: true })).toBe(false);
    expect(isDirectPickAllowed("", { ctrlKey: true })).toBe(false);
    expect(isDirectPickAllowed("", { altKey: true })).toBe(false);
    expect(isDirectPickAllowed("", { shiftKey: true })).toBe(false);
  });
});

// P2-6 输入法防御：组词期间两种平台表现都要识别
describe("isImeComposing", () => {
  it("isComposing=true 判定为组词中", () => {
    // Arrange & Act
    const result = isImeComposing({ isComposing: true });
    // Assert
    expect(result).toBe(true);
  });

  it("keyCode=229（Safari/WKWebView 组词中的经典表现）判定为组词中", () => {
    // Arrange & Act
    const result = isImeComposing({ isComposing: false, keyCode: 229 });
    // Assert
    expect(result).toBe(true);
  });

  it("普通按键（无组词、keyCode 非 229）判定为不在组词中", () => {
    // Arrange & Act
    const result = isImeComposing({ isComposing: false, keyCode: 40 });
    // Assert
    expect(result).toBe(false);
  });
});

// V1-F2-A12: 键盘流纯逻辑单测
describe("moveHighlight", () => {
  it("ArrowDown 使高亮索引 +1", () => {
    // Arrange & Act
    const result = moveHighlight(0, "ArrowDown", 5);
    // Assert
    expect(result).toBe(1);
  });

  it("ArrowUp 使高亮索引 -1", () => {
    // Arrange & Act
    const result = moveHighlight(2, "ArrowUp", 5);
    // Assert
    expect(result).toBe(1);
  });

  it("ArrowDown 在末尾时 clamp 不越出（边界不越界）", () => {
    // Arrange & Act
    const result = moveHighlight(4, "ArrowDown", 5);
    // Assert：最大索引为 count-1=4，不越出
    expect(result).toBe(4);
  });

  it("ArrowUp 在首位时 clamp 不越出（边界不越界）", () => {
    // Arrange & Act
    const result = moveHighlight(0, "ArrowUp", 5);
    // Assert：最小索引为 0，不越出
    expect(result).toBe(0);
  });

  it("count=0 时返回 -1（列表为空，无有效高亮）", () => {
    // Arrange & Act
    const result = moveHighlight(0, "ArrowDown", 0);
    // Assert
    expect(result).toBe(-1);
  });

  it("count=1 时上下移动都保持在索引 0", () => {
    // Arrange & Act
    const down = moveHighlight(0, "ArrowDown", 1);
    const up = moveHighlight(0, "ArrowUp", 1);
    // Assert
    expect(down).toBe(0);
    expect(up).toBe(0);
  });
});

describe("quickSelectIndex", () => {
  it("数字键 '1' 映射到索引 0", () => {
    expect(quickSelectIndex("1")).toBe(0);
  });

  it("数字键 '9' 映射到索引 8", () => {
    expect(quickSelectIndex("9")).toBe(8);
  });

  it("数字键 '5' 映射到索引 4", () => {
    expect(quickSelectIndex("5")).toBe(4);
  });

  it("非数字键返回 null", () => {
    expect(quickSelectIndex("a")).toBeNull();
    expect(quickSelectIndex("0")).toBeNull();
    expect(quickSelectIndex("10")).toBeNull();
    expect(quickSelectIndex("")).toBeNull();
  });
});

describe("resolveEnter", () => {
  const items: HistoryItem[] = [
    { id: "1", text: "第一条", kind: "text" },
    { id: "2", text: "第二条", kind: "richtext" },
    { id: "3", text: "第三条", kind: "text" },
  ];

  it("高亮在有效索引时返回对应条目", () => {
    // Arrange & Act
    const result = resolveEnter(1, items);
    // Assert
    expect(result).toEqual({ id: "2", text: "第二条", kind: "richtext" });
  });

  it("高亮在首条时返回第一条", () => {
    // Arrange & Act
    const result = resolveEnter(0, items);
    // Assert
    expect(result).toEqual({ id: "1", text: "第一条", kind: "text" });
  });

  it("高亮越界（>= length）时返回 null", () => {
    // Arrange & Act
    const result = resolveEnter(5, items);
    // Assert
    expect(result).toBeNull();
  });

  it("高亮为 -1（列表为空场景）时返回 null", () => {
    // Arrange & Act
    const result = resolveEnter(-1, items);
    // Assert
    expect(result).toBeNull();
  });

  it("列表为空时返回 null", () => {
    // Arrange & Act
    const result = resolveEnter(0, []);
    // Assert
    expect(result).toBeNull();
  });
});
