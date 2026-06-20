import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

// 锁住真正的根因：主窗口拖动依赖 Tauri startDragging JS API，
// 该 API 受 capabilities ACL 管控，缺 allow-start-dragging 会被静默拒绝、
// 导致 data-tauri-drag-region 整条标题栏拖不动（与 popover ACL 坑同源）。
describe("capabilities/default.json ACL 授权", () => {
  it("permissions 数组包含 core:window:allow-start-dragging（否则标题栏拖动被 ACL 静默拒绝）", () => {
    const configPath = resolve(process.cwd(), "src-tauri/capabilities/default.json");
    const config = JSON.parse(readFileSync(configPath, "utf-8")) as {
      permissions: string[];
    };

    expect(config.permissions).toContain("core:window:allow-start-dragging");
  });

  it("permissions 数组包含最小化/最大化/关闭窗口权限（Windows 自绘标题栏按钮依赖这些窗口 JS API，缺权限会被 ACL 静默拒绝）", () => {
    const configPath = resolve(process.cwd(), "src-tauri/capabilities/default.json");
    const config = JSON.parse(readFileSync(configPath, "utf-8")) as {
      permissions: string[];
    };

    expect(config.permissions).toContain("core:window:allow-minimize");
    expect(config.permissions).toContain("core:window:allow-toggle-maximize");
    expect(config.permissions).toContain("core:window:allow-close");
  });
});
