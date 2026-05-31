import { describe, it, expect } from "vitest";
import { availableActions, resolveTranslateAction } from "./translate-actions";

// V2-F3-A15: translate_actions
// 译文操作集——一键复制/朗读/切目标语/换源重译/存翻译历史

describe("availableActions", () => {
  it("包含全部 5 个操作：copy/speak/switch_target/switch_source_retranslate/save_history", () => {
    // Arrange + Act
    const actions = availableActions();

    // Assert
    expect(actions).toContain("copy");
    expect(actions).toContain("speak");
    expect(actions).toContain("switch_target");
    expect(actions).toContain("switch_source_retranslate");
    expect(actions).toContain("save_history");
    expect(actions).toHaveLength(5);
  });
});

describe("resolveTranslateAction", () => {
  it("copy 命令映射为 copy 操作", () => {
    // Arrange
    const cmd = "copy";

    // Act
    const action = resolveTranslateAction(cmd);

    // Assert
    expect(action).toBe("copy");
  });

  it("speak 命令映射为 speak 操作", () => {
    // Arrange
    const cmd = "speak";

    // Act
    const action = resolveTranslateAction(cmd);

    // Assert
    expect(action).toBe("speak");
  });

  it("switch_target 命令映射为 switch_target 操作", () => {
    // Arrange
    const cmd = "switch_target";

    // Act
    const action = resolveTranslateAction(cmd);

    // Assert
    expect(action).toBe("switch_target");
  });

  it("switch_source_retranslate 命令映射为 switch_source_retranslate 操作", () => {
    // Arrange
    const cmd = "switch_source_retranslate";

    // Act
    const action = resolveTranslateAction(cmd);

    // Assert
    expect(action).toBe("switch_source_retranslate");
  });

  it("save_history 命令映射为 save_history 操作", () => {
    // Arrange
    const cmd = "save_history";

    // Act
    const action = resolveTranslateAction(cmd);

    // Assert
    expect(action).toBe("save_history");
  });

  it("非法命令返回 null（不抛异常）", () => {
    // Arrange
    const cmd = "unknown_command";

    // Act
    const action = resolveTranslateAction(cmd);

    // Assert：非法命令映射为 null，不 throw
    expect(action).toBeNull();
  });

  it("空字符串返回 null（非恒真：非法输入不映射到合法操作）", () => {
    // Arrange
    const cmd = "";

    // Act
    const action = resolveTranslateAction(cmd);

    // Assert
    expect(action).toBeNull();
  });
});
