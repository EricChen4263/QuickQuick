import { describe, it, expect } from "vitest";
import { resolveSelectAction } from "./select-trigger";

// V2-F3-A12: select_translate_icon_trigger
// 选中即译触发纪律——选中后只冒图标，点击或快捷键才译（绝非选中自动弹）
describe("resolveSelectAction", () => {
  it("text_selected 返回 show_icon，而非 translate（不自动翻译）", () => {
    // Arrange
    const event = "text_selected" as const;

    // Act
    const action = resolveSelectAction(event);

    // Assert: 选中文字只冒图标，绝不自动触发翻译
    expect(action).toBe("show_icon");
    expect(action).not.toBe("translate");
  });

  it("icon_clicked 返回 translate（点图标才触发翻译）", () => {
    // Arrange
    const event = "icon_clicked" as const;

    // Act
    const action = resolveSelectAction(event);

    // Assert
    expect(action).toBe("translate");
  });

  it("hotkey_translate 返回 translate（Cmd+Shift+T 快捷键才触发翻译）", () => {
    // Arrange
    const event = "hotkey_translate" as const;

    // Act
    const action = resolveSelectAction(event);

    // Assert
    expect(action).toBe("translate");
  });

  it("click_elsewhere 返回 dismiss（点别处则图标消失）", () => {
    // Arrange
    const event = "click_elsewhere" as const;

    // Act
    const action = resolveSelectAction(event);

    // Assert
    expect(action).toBe("dismiss");
  });
});
