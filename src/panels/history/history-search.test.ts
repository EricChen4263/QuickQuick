import { describe, it, expect } from "vitest";
import { filterBySearch } from "./search";
import type { HistoryItem } from "./search";

// V1-F2-A09: 实时搜索过滤逻辑单测
describe("filterBySearch", () => {
  const items: HistoryItem[] = [
    { id: "1", text: "Hello World", kind: "text" },
    { id: "2", text: "hello typescript", kind: "text" },
    { id: "3", text: "Goodbye", kind: "richtext" },
    { id: "4", text: "Rich Content Hello", kind: "richtext" },
  ];

  it("空 query 返回全部条目", () => {
    // Arrange
    const query = "";
    // Act
    const result = filterBySearch(items, query);
    // Assert
    expect(result).toHaveLength(4);
    expect(result).toEqual(items);
    // 快捷路径须返回新数组，不得暴露原数组引用（I-01 审查要求）
    expect(result).not.toBe(items);
  });

  it("按子串匹配命中含该子串的条目", () => {
    // Arrange
    const query = "hello";
    // Act
    const result = filterBySearch(items, query);
    // Assert
    expect(result).toHaveLength(3);
    expect(result.map((i) => i.id)).toEqual(["1", "2", "4"]);
  });

  it("大小写不敏感匹配", () => {
    // Arrange
    const query = "HELLO";
    // Act
    const result = filterBySearch(items, query);
    // Assert
    expect(result).toHaveLength(3);
    expect(result.map((i) => i.id)).toEqual(["1", "2", "4"]);
  });

  it("无匹配时返回空数组", () => {
    // Arrange
    const query = "xyz_not_exist";
    // Act
    const result = filterBySearch(items, query);
    // Assert
    expect(result).toHaveLength(0);
  });

  it("不修改原数组（不可变）", () => {
    // Arrange
    const original = [...items];
    const query = "hello";
    // Act
    filterBySearch(items, query);
    // Assert：原数组长度和内容不变
    expect(items).toHaveLength(original.length);
    expect(items).toEqual(original);
  });

  it("query 只有空白字符时返回全部（视为空查询）", () => {
    // Arrange
    const query = "   ";
    // Act
    const result = filterBySearch(items, query);
    // Assert
    expect(result).toHaveLength(4);
  });
});
