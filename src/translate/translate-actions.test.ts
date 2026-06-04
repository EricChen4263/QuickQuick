import { describe, it, expect } from "vitest";
import { availableActions, resolveTranslateAction } from "./translate-actions";

// V2-F3-A15: translate_actions
// 译文操作集——一键复制 / 朗读 / 存翻译历史
// （切目标语 switch_target、换源重译 switch_source_retranslate 已移除：
//  UI 上译文区只留复制/朗读，方向切换由顶部 DirBar 直接重译承担）

describe("availableActions", () => {
  it("包含 3 个操作：copy/speak/save_history，不再含 switch_target/switch_source_retranslate", () => {
    // Arrange + Act
    const actions = availableActions();

    // Assert
    expect(actions).toEqual(["copy", "speak", "save_history"]);
    expect(actions).not.toContain("switch_target");
    expect(actions).not.toContain("switch_source_retranslate");
  });
});

describe("resolveTranslateAction", () => {
  it("copy 命令映射为 copy 操作", () => {
    expect(resolveTranslateAction("copy")).toBe("copy");
  });

  it("speak 命令映射为 speak 操作", () => {
    expect(resolveTranslateAction("speak")).toBe("speak");
  });

  it("save_history 命令映射为 save_history 操作", () => {
    expect(resolveTranslateAction("save_history")).toBe("save_history");
  });

  it("已移除的 switch_target 命令返回 null", () => {
    expect(resolveTranslateAction("switch_target")).toBeNull();
  });

  it("已移除的 switch_source_retranslate 命令返回 null", () => {
    expect(resolveTranslateAction("switch_source_retranslate")).toBeNull();
  });

  it("非法命令返回 null（不抛异常）", () => {
    expect(resolveTranslateAction("unknown_command")).toBeNull();
  });

  it("空字符串返回 null（非恒真：非法输入不映射到合法操作）", () => {
    expect(resolveTranslateAction("")).toBeNull();
  });
});
