import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";
import ClipboardPage from "./ClipboardPage";

// Mock Tauri event API：渲染测试环境无 Tauri 运行时
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock IPC：主窗列表只用读接口，外观设置是其中唯一被本文件断言的项
vi.mock("../../ipc/ipc-client", () => ({
  getClipThumbnail: vi.fn().mockResolvedValue(null),
  getAppearanceSettings: vi.fn(),
  listClipItems: vi.fn(),
  deleteClipItem: vi.fn(),
  toggleFavoriteClip: vi.fn(),
  pasteToFront: vi.fn(),
  copyClipToClipboard: vi.fn(),
  openAccessibilitySettings: vi.fn(),
  getHotkeys: vi.fn(),
  getLaunchOnLogin: vi.fn(),
  getOnboardingStatus: vi.fn(),
  setLaunchOnLogin: vi.fn(),
}));

import { listen } from "@tauri-apps/api/event";
import { CLIPBOARD_CHANGED_EVENT } from "../../ipc/events";
import {
  getAppearanceSettings,
  getHotkeys,
  getLaunchOnLogin,
  getOnboardingStatus,
  listClipItems,
} from "../../ipc/ipc-client";

/** 一条文本条目：只需要驱动列表挂载，不用真的渲染命中 */
const ITEMS = [
  {
    id: "id1",
    content: "hello world",
    kind: "text" as const,
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: 1000,
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.setItem("qq-onboard-finished", "true");
  vi.mocked(listClipItems).mockResolvedValue(ITEMS);
  vi.mocked(getHotkeys).mockResolvedValue({
    history: "CmdOrCtrl+Shift+H",
    translate: "CmdOrCtrl+Shift+T",
    main: "CmdOrCtrl+Shift+M",
  });
  vi.mocked(getLaunchOnLogin).mockResolvedValue(true);
  vi.mocked(getOnboardingStatus).mockResolvedValue({
    action: "proceed",
    capability: "full_paste",
  });
  vi.mocked(getAppearanceSettings).mockResolvedValue({
    sortOrder: "recent",
    imageHeightPx: 24,
    previewFirstDelayMs: 1500,
    highlightStyle: "default",
  });
});

/** 取主窗列表根容器（外观设置属性挂在它上面） */
function clipPage(container: HTMLElement): HTMLElement {
  const element = container.querySelector(".clip-page");
  if (!(element instanceof HTMLElement)) throw new Error("找不到 .clip-page");
  return element;
}

describe("ClipboardPage 消费命中高亮样式", () => {
  it("默认档与改动前一致：data-hit-style=default", async () => {
    // Arrange & Act
    const { container } = render(<ClipboardPage />);

    // Assert
    await waitFor(() => expect(clipPage(container)).toBeInTheDocument());
    expect(clipPage(container).getAttribute("data-hit-style")).toBe("default");
  });

  it("设置为下划线时把 data-hit-style 下发到列表根（CSS 据此换 <mark> 呈现）", async () => {
    // Arrange
    vi.mocked(getAppearanceSettings).mockResolvedValue({
      sortOrder: "recent",
      imageHeightPx: 24,
      previewFirstDelayMs: 1500,
      highlightStyle: "underline",
    });

    // Act
    const { container } = render(<ClipboardPage />);

    // Assert
    await waitFor(() => {
      expect(clipPage(container).getAttribute("data-hit-style")).toBe("underline");
    });
  });

  it("外观设置读取失败时回退默认档，列表照常渲染", async () => {
    // Arrange
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.mocked(getAppearanceSettings).mockRejectedValue(new Error("settings.json 读失败"));

    // Act
    const { container } = render(<ClipboardPage />);

    // Assert
    await waitFor(() => expect(clipPage(container)).toBeInTheDocument());
    expect(clipPage(container).getAttribute("data-hit-style")).toBe("default");
    warn.mockRestore();
  });

  it("收到 clipboard-changed 后重读外观设置：新样式立刻生效", async () => {
    // Arrange：捕获 listen 注册的回调，并让第二次读取返回下划线档
    let capturedCallback: (() => void) | undefined;
    vi.mocked(listen).mockImplementation((_eventName, handler) => {
      capturedCallback = handler as () => void;
      return Promise.resolve(() => {});
    });
    const { container } = render(<ClipboardPage />);
    await waitFor(() => expect(clipPage(container).getAttribute("data-hit-style")).toBe("default"));
    expect(vi.mocked(listen)).toHaveBeenCalledWith(CLIPBOARD_CHANGED_EVENT, expect.any(Function));

    vi.mocked(getAppearanceSettings).mockResolvedValue({
      sortOrder: "recent",
      imageHeightPx: 24,
      previewFirstDelayMs: 1500,
      highlightStyle: "underline",
    });

    // Act：模拟后端在「命中高亮样式落盘」后发出的列表刷新事件
    capturedCallback!();

    // Assert：外观设置被重新读取，属性随之切换
    await waitFor(() => {
      expect(clipPage(container).getAttribute("data-hit-style")).toBe("underline");
    });
    expect(vi.mocked(getAppearanceSettings)).toHaveBeenCalledTimes(2);
  });
});
