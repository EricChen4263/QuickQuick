import { describe, it, expect } from "vitest";
import { advanceSelection, createInputOwnership } from "./keyboard-nav";

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

describe("createInputOwnership（悬停与键盘的主导权仲裁）", () => {
  it("初始由键盘主导，悬停不得改选中项", () => {
    const ownership = createInputOwnership();

    expect(ownership.isPointerOwner()).toBe(false);
  });

  it("坐标变化过的移动算真实位移，主导权交给鼠标", () => {
    const ownership = createInputOwnership();

    const isRealMove = ownership.observePointerMove(12, 34);

    expect(isRealMove).toBe(true);
    expect(ownership.isPointerOwner()).toBe(true);
  });

  it("与上次同坐标的移动不算真实位移（滚动补发的 mousemove）", () => {
    const ownership = createInputOwnership();
    ownership.observePointerMove(12, 34);

    const isRealMove = ownership.observePointerMove(12, 34);

    expect(isRealMove).toBe(false);
  });

  it("交还键盘后，同坐标的移动仍然不算真实位移", () => {
    // Arrange：鼠标先真实移动到 (12,34)，再按方向键把主导权交还键盘
    const ownership = createInputOwnership();
    ownership.observePointerMove(12, 34);
    ownership.handToKeyboard();

    // Act：键盘导航让列表滚动，静止指针下补发同坐标移动
    const isRealMove = ownership.observePointerMove(12, 34);

    // Assert：坐标没变就不许抢，主导权仍在键盘
    expect(isRealMove).toBe(false);
    expect(ownership.isPointerOwner()).toBe(false);
  });

  it("交还键盘后，坐标真的变化则重新交给鼠标", () => {
    const ownership = createInputOwnership();
    ownership.observePointerMove(12, 34);
    ownership.handToKeyboard();

    const isRealMove = ownership.observePointerMove(12, 60);

    expect(isRealMove).toBe(true);
    expect(ownership.isPointerOwner()).toBe(true);
  });
});
