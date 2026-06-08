import { describe, it, expect } from "vitest";
import { resolveRoute, type HotkeyTrigger, type WindowView } from "./windowRoute";

// V0-F2-A03: 预热窗口按 V/T 路由历史/翻译不同视图（路由纯逻辑）
describe("window-route", () => {
  it("window_route_by_hotkey: CmdOrCtrl+Shift+C 路由到 history 视图", () => {
    // Arrange
    const trigger: HotkeyTrigger = "history";

    // Act
    const view: WindowView = resolveRoute(trigger);

    // Assert
    expect(view).toBe("history");
  });

  it("window_route_by_hotkey: CmdOrCtrl+Shift+T 路由到 translate 视图", () => {
    // Arrange
    const trigger: HotkeyTrigger = "translate";

    // Act
    const view: WindowView = resolveRoute(trigger);

    // Assert
    expect(view).toBe("translate");
  });

  it("window_route_by_hotkey: 相同热键连续触发保持同一视图不切换", () => {
    // Arrange & Act
    const first = resolveRoute("history");
    const second = resolveRoute("history");

    // Assert
    expect(first).toBe("history");
    expect(second).toBe("history");
  });

  it("window_route_by_hotkey: 从 history 切换到 translate 视图", () => {
    // Arrange
    const historyRoute = resolveRoute("history");

    // Act
    const translateRoute = resolveRoute("translate");

    // Assert
    expect(historyRoute).toBe("history");
    expect(translateRoute).toBe("translate");
  });
});
