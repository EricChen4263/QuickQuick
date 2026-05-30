import { describe, it, expect } from "vitest";
import { resolvePasteMode } from "./paste-mode";

// V1-F3-A16: 回车粘贴 / 修饰键仅复制——前端粘贴模式纯逻辑单测
describe("resolvePasteMode", () => {
  it("无修饰键（纯 Enter）时返回 paste 模式（写回 + 粘贴）", () => {
    // Arrange
    const hasModifier = false;

    // Act
    const mode = resolvePasteMode(hasModifier);

    // Assert
    expect(mode).toBe("paste");
  });

  it("有修饰键时返回 copy_only 模式（仅写回不粘贴）", () => {
    // Arrange
    const hasModifier = true;

    // Act
    const mode = resolvePasteMode(hasModifier);

    // Assert
    expect(mode).toBe("copy_only");
  });
});
