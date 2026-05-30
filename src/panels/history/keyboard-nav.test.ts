import { describe, it, expect } from "vitest";
import { moveHighlight, quickSelectIndex, resolveEnter } from "./keyboard";
import type { HistoryItem } from "./search";

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
