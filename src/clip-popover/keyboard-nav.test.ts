import { describe, it, expect } from "vitest";
import { advanceSelection } from "./keyboard-nav";

describe("advanceSelection", () => {
  const ids = ["a", "b", "c", "d"];

  it("列表中段 ArrowDown 前进一步", () => {
    expect(advanceSelection("b", "ArrowDown", ids)).toBe("c");
  });

  it("列表中段 ArrowUp 后退一步", () => {
    expect(advanceSelection("c", "ArrowUp", ids)).toBe("b");
  });

  it("末尾 ArrowDown 保持末尾（边界 clamp）", () => {
    expect(advanceSelection("d", "ArrowDown", ids)).toBe("d");
  });

  it("首位 ArrowUp 保持首位（边界 clamp）", () => {
    expect(advanceSelection("a", "ArrowUp", ids)).toBe("a");
  });

  it("currentId 为 null 时 ArrowDown 选第一个", () => {
    expect(advanceSelection(null, "ArrowDown", ids)).toBe("a");
  });

  it("currentId 为 null 时 ArrowUp 选第一个", () => {
    expect(advanceSelection(null, "ArrowUp", ids)).toBe("a");
  });

  it("currentId 不在列表中时 ArrowDown 选第一个", () => {
    expect(advanceSelection("z", "ArrowDown", ids)).toBe("a");
  });

  it("currentId 不在列表中时 ArrowUp 选第一个", () => {
    expect(advanceSelection("z", "ArrowUp", ids)).toBe("a");
  });

  it("空列表返回 null", () => {
    expect(advanceSelection("a", "ArrowDown", [])).toBeNull();
  });

  it("空列表 null currentId 返回 null", () => {
    expect(advanceSelection(null, "ArrowDown", [])).toBeNull();
  });

  it("单元素列表 ArrowDown 保持同一 id", () => {
    expect(advanceSelection("x", "ArrowDown", ["x"])).toBe("x");
  });

  it("单元素列表 ArrowUp 保持同一 id", () => {
    expect(advanceSelection("x", "ArrowUp", ["x"])).toBe("x");
  });
});
