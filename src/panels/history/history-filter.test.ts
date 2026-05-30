import { describe, it, expect } from "vitest";
import { filterByType } from "./filter";
import type { HistoryFilter } from "./filter";
import type { HistoryItem } from "./search";

// V1-F2-A10: 类型筛选逻辑单测
describe("filterByType", () => {
  const items: HistoryItem[] = [
    { id: "1", text: "纯文本 A", kind: "text" },
    { id: "2", text: "纯文本 B", kind: "text" },
    { id: "3", text: "富文本 A", kind: "richtext" },
    { id: "4", text: "富文本 B", kind: "richtext" },
  ];

  it("filter=all 返回全部条目", () => {
    // Arrange
    const filter: HistoryFilter = "all";
    // Act
    const result = filterByType(items, filter);
    // Assert
    expect(result).toHaveLength(4);
    expect(result).toEqual(items);
    // 快捷路径须返回新数组，不得暴露原数组引用（I-01 审查要求）
    expect(result).not.toBe(items);
  });

  it("filter=text 只返回 kind=text 的条目", () => {
    // Arrange
    const filter: HistoryFilter = "text";
    // Act
    const result = filterByType(items, filter);
    // Assert
    expect(result).toHaveLength(2);
    expect(result.every((i) => i.kind === "text")).toBe(true);
    expect(result.map((i) => i.id)).toEqual(["1", "2"]);
  });

  it("filter=richtext 只返回 kind=richtext 的条目", () => {
    // Arrange
    const filter: HistoryFilter = "richtext";
    // Act
    const result = filterByType(items, filter);
    // Assert
    expect(result).toHaveLength(2);
    expect(result.every((i) => i.kind === "richtext")).toBe(true);
    expect(result.map((i) => i.id)).toEqual(["3", "4"]);
  });

  it("混合列表中 filter=text 只命中文本条目", () => {
    // Arrange
    const mixed: HistoryItem[] = [
      { id: "a", text: "text item", kind: "text" },
      { id: "b", text: "rich item", kind: "richtext" },
      { id: "c", text: "another text", kind: "text" },
    ];
    const filter: HistoryFilter = "text";
    // Act
    const result = filterByType(mixed, filter);
    // Assert
    expect(result).toHaveLength(2);
    expect(result.map((i) => i.id)).toEqual(["a", "c"]);
  });

  it("列表为空时返回空数组", () => {
    // Arrange
    const emptyItems: HistoryItem[] = [];
    // Act
    const result = filterByType(emptyItems, "text");
    // Assert
    expect(result).toHaveLength(0);
  });
});
