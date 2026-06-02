import { describe, it, expect, vi, beforeEach } from "vitest";
import type { MockedFunction } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { checkForUpdates } from "./ipc-client";
import type { CheckUpdateResult } from "./ipc-client";

const mockInvoke = invoke as MockedFunction<typeof invoke>;

beforeEach(() => {
  mockInvoke.mockReset();
});

describe("checkForUpdates", () => {
  it("调用 invoke('check_for_updates') 并返回有更新的结果", async () => {
    const mockResult: CheckUpdateResult = {
      available: true,
      version: "1.1.0",
      currentVersion: "1.0.0",
    };
    mockInvoke.mockResolvedValueOnce(mockResult);

    const result = await checkForUpdates();

    expect(mockInvoke).toHaveBeenCalledWith("check_for_updates");
    expect(result.available).toBe(true);
    expect(result.version).toBe("1.1.0");
    expect(result.currentVersion).toBe("1.0.0");
  });

  it("返回无更新时 available=false、version 为空串", async () => {
    const mockResult: CheckUpdateResult = {
      available: false,
      version: "",
      currentVersion: "1.0.0",
    };
    mockInvoke.mockResolvedValueOnce(mockResult);

    const result = await checkForUpdates();

    expect(mockInvoke).toHaveBeenCalledWith("check_for_updates");
    expect(result.available).toBe(false);
    expect(result.version).toBe("");
  });

  it("invoke reject 时重抛为 Error 且含原始消息", async () => {
    mockInvoke.mockRejectedValueOnce("检查更新失败：网络错误");

    const err = await checkForUpdates().catch((e: unknown) => e);

    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("检查更新失败");
  });

  it("invoke reject Error 对象时透传消息", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("updater 初始化失败"));

    const err = await checkForUpdates().catch((e: unknown) => e);

    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("updater 初始化失败");
  });
});
