import { describe, it, expect, vi, beforeEach } from "vitest";
import type { MockedFunction } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { restartApp } from "./ipc-client";

const mockInvoke = invoke as MockedFunction<typeof invoke>;

beforeEach(() => {
  mockInvoke.mockReset();
});

describe("restartApp", () => {
  it("调用 invoke('restart_app')", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await restartApp();

    expect(mockInvoke).toHaveBeenCalledWith("restart_app");
  });

  it("invoke reject 时重抛为 Error 且含原始消息", async () => {
    mockInvoke.mockRejectedValueOnce("重启失败");

    const err = await restartApp().catch((e: unknown) => e);

    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("重启失败");
  });
});
