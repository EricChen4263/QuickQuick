import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { EventCallback } from "@tauri-apps/api/event";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

vi.mock("../ipc/ipc-client", () => ({
  restartApp: vi.fn().mockResolvedValue(undefined),
}));

import { listen } from "@tauri-apps/api/event";
import { restartApp } from "../ipc/ipc-client";
import UpdateBanner from "./UpdateBanner";

interface ReadyPayload {
  version: string;
}

let capturedHandler: EventCallback<ReadyPayload> | undefined;

beforeEach(() => {
  vi.clearAllMocks();
  capturedHandler = undefined;
  vi.mocked(listen).mockImplementation((_event, cb) => {
    capturedHandler = cb as EventCallback<ReadyPayload>;
    return Promise.resolve(() => {});
  });
});

/** 模拟后端 emit `update://ready`，触发组件捕获的事件回调。 */
async function emitReady(version: string): Promise<void> {
  await waitFor(() => expect(capturedHandler).toBeDefined());
  await act(async () => {
    capturedHandler?.({
      event: "update://ready",
      id: 1,
      payload: { version },
    });
  });
}

describe("UpdateBanner", () => {
  it("update_banner_shows_on_ready_and_restart_invokes_command", async () => {
    const user = userEvent.setup();
    render(<UpdateBanner />);

    // 初始未就绪：不渲染提示条
    expect(screen.queryByText(/1\.2\.3/)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "重启更新" })
    ).not.toBeInTheDocument();

    // 收到就绪事件 → 出现含版本号的提示条
    await emitReady("1.2.3");
    await waitFor(() => {
      expect(screen.getByText(/1\.2\.3/)).toBeInTheDocument();
    });

    // 点「重启更新」→ 调用 restartApp
    await user.click(screen.getByRole("button", { name: "重启更新" }));
    expect(restartApp).toHaveBeenCalledTimes(1);
  });

  it("点「稍后」隐藏提示条", async () => {
    const user = userEvent.setup();
    render(<UpdateBanner />);

    await emitReady("2.0.0");
    expect(screen.getByText(/2\.0\.0/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "稍后" }));
    await waitFor(() => {
      expect(screen.queryByText(/2\.0\.0/)).not.toBeInTheDocument();
    });
    expect(restartApp).not.toHaveBeenCalled();
  });
});
