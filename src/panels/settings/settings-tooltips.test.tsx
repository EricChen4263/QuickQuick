import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

// 渲染测试环境无 Tauri 运行时：事件订阅与 IPC 全部打桩
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../../ipc/ipc-client", () => ({
  // 通用
  getLaunchOnLogin: vi.fn(),
  setLaunchOnLogin: vi.fn(),
  getStayInTray: vi.fn(),
  setStayInTray: vi.fn(),
  getAutoUpdate: vi.fn(),
  setAutoUpdate: vi.fn(),
  // 外观
  getAppearanceSettings: vi.fn(),
  setSortOrder: vi.fn(),
  setImageHeight: vi.fn(),
  setPreviewFirstDelay: vi.fn(),
  setHighlightStyle: vi.fn(),
  // 热键
  getHotkeys: vi.fn(),
  setHotkey: vi.fn(),
  // 翻译源
  getTranslateProviders: vi.fn(),
  getSelectedProvider: vi.fn(),
  setSelectedProvider: vi.fn(),
  getProviderCredentialSchema: vi.fn(),
  getProviderCredentials: vi.fn(),
  // 隐私
  getExcludeList: vi.fn(),
  setExcludeList: vi.fn(),
  getExcludeMode: vi.fn(),
  setExcludeMode: vi.fn(),
  getIgnoreRegexes: vi.fn(),
  setIgnoreRegexes: vi.fn(),
  getPauseCapture: vi.fn(),
  setPauseCapture: vi.fn(),
  getSkipSensitive: vi.fn(),
  setSkipSensitive: vi.fn(),
  // 存储
  getStorageStats: vi.fn(),
  cleanupHistory: vi.fn(),
  getImageThreshold: vi.fn(),
  setImageThreshold: vi.fn(),
  getMaxHistoryItems: vi.fn(),
  setMaxHistoryItems: vi.fn(),
  exportBackup: vi.fn(),
  importBackup: vi.fn(),
  revealBackupInFinder: vi.fn(),
  // StoragePanel 直接引用这两个常量做输入校验
  MIN_HISTORY_ITEMS: 1,
  MAX_HISTORY_ITEMS: 999,
}));

import GeneralPanel from "./GeneralPanel";
import AppearancePanel from "./AppearancePanel";
import HotkeyPanel from "./HotkeyPanel";
import TranslateSourcePanel from "./TranslateSourcePanel";
import PrivacyPanel from "./PrivacyPanel";
import StoragePanel from "./StoragePanel";
import {
  getAppearanceSettings,
  getAutoUpdate,
  getExcludeList,
  getExcludeMode,
  getHotkeys,
  getIgnoreRegexes,
  getImageThreshold,
  getLaunchOnLogin,
  getMaxHistoryItems,
  getPauseCapture,
  getProviderCredentialSchema,
  getProviderCredentials,
  getSelectedProvider,
  getSkipSensitive,
  getStayInTray,
  getStorageStats,
  getTranslateProviders,
} from "../../ipc/ipc-client";

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getLaunchOnLogin).mockResolvedValue(true);
  vi.mocked(getStayInTray).mockResolvedValue(true);
  vi.mocked(getAutoUpdate).mockResolvedValue(true);
  vi.mocked(getAppearanceSettings).mockResolvedValue({
    sortOrder: "recent",
    imageHeightPx: 40,
    previewFirstDelayMs: 1500,
    highlightStyle: "bold",
  });
  vi.mocked(getHotkeys).mockResolvedValue({
    history: "CmdOrCtrl+Shift+C",
    translate: "CmdOrCtrl+Shift+T",
    main: "CmdOrCtrl+Shift+M",
  });
  vi.mocked(getTranslateProviders).mockResolvedValue([
    { id: "google_free", name: "Google（免费）", needsKey: false, needsConfig: false, isUnofficial: true },
    { id: "deepl", name: "DeepL", needsKey: true, needsConfig: true, isUnofficial: false },
  ]);
  vi.mocked(getSelectedProvider).mockResolvedValue("google_free");
  vi.mocked(getProviderCredentialSchema).mockResolvedValue([]);
  vi.mocked(getProviderCredentials).mockResolvedValue([]);
  vi.mocked(getPauseCapture).mockResolvedValue(false);
  vi.mocked(getSkipSensitive).mockResolvedValue(true);
  vi.mocked(getExcludeList).mockResolvedValue([]);
  vi.mocked(getExcludeMode).mockResolvedValue("blacklist");
  vi.mocked(getIgnoreRegexes).mockResolvedValue([]);
  vi.mocked(getStorageStats).mockResolvedValue({
    liveCount: 3,
    imageBytes: 0,
    fileSizeBytes: 4096,
  });
  vi.mocked(getImageThreshold).mockResolvedValue(20 * 1024 * 1024);
  vi.mocked(getMaxHistoryItems).mockResolvedValue(500);
});

/**
 * 取某一段文字所在设置行的 tooltip 文案。
 *
 * 用法约束：`text` 必须是该行内唯一的文本（标签或控件 aria-label），
 * 否则会命中多行、断言失去意义。
 */
function tooltipOfRowContaining(text: string): string | null {
  const element = screen.getByText(text);
  const row = element.closest(".set-row");
  if (row === null) throw new Error(`「${text}」不在任何 .set-row 里`);
  return row.getAttribute("title");
}

describe("全部设置项都有 tooltip（对齐 Maccy 的 .help 约定）", () => {
  it("通用：三个开关 + 立即检查更新都写明默认值/无默认值", async () => {
    render(<GeneralPanel />);
    await waitFor(() => expect(getLaunchOnLogin).toHaveBeenCalled());

    for (const label of ["开机自启动", "托盘常驻", "自动检查更新"]) {
      expect(tooltipOfRowContaining(label)).toMatch(/^默认：/);
    }
    expect(tooltipOfRowContaining("立即检查更新")).toContain("无默认值");
  });

  it("外观：四项 tooltip 都写明默认值", async () => {
    render(<AppearancePanel />);
    await waitFor(() => expect(getAppearanceSettings).toHaveBeenCalled());

    for (const label of ["排序方式", "命中高亮样式"]) {
      expect(tooltipOfRowContaining(label)).toMatch(/^默认：/);
    }
    for (const label of ["弹出面板图片高度", "首次预览延迟"]) {
      const input = screen.getByLabelText(label);
      const row = input.closest(".set-row");
      expect(row?.getAttribute("title")).toMatch(/^默认：/);
    }
  });

  it("热键：三行都写明默认键位", async () => {
    render(<HotkeyPanel />);
    await waitFor(() => expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3));

    const expected: Array<[string, string]> = [
      ["剪贴板历史", "⌘+Shift+C"],
      ["翻译选中", "⌘+Shift+T"],
      ["应用主界面", "⌘+Shift+M"],
    ];
    for (const [label, defaultKey] of expected) {
      const tooltip = tooltipOfRowContaining(label);
      expect(tooltip).toContain("默认：");
      expect(tooltip).toContain(defaultKey);
    }
  });

  it("翻译源：整组 tooltip 写明默认源", async () => {
    const { container } = render(<TranslateSourcePanel />);
    await waitFor(() => expect(getSelectedProvider).toHaveBeenCalled());

    const group = container.querySelector(".set-group");
    expect(group?.getAttribute("title")).toContain("默认：Google（免费）");
  });

  it("隐私：五个设置项都有 tooltip（开关写明默认开/关，名单与正则写明默认空）", async () => {
    render(<PrivacyPanel />);
    await waitFor(() => expect(getExcludeMode).toHaveBeenCalled());

    expect(tooltipOfRowContaining("暂停剪贴板监听")).toContain("默认：关");
    expect(tooltipOfRowContaining("跳过敏感内容")).toContain("默认：开");
    expect(tooltipOfRowContaining("App 名单模式")).toContain("默认：黑名单");
    expect(tooltipOfRowContaining("App 名单")).toContain("默认：空名单");
    expect(tooltipOfRowContaining("内容忽略正则")).toContain("默认：空");
  });

  it("存储：可配置项写明默认值，固定项与一次性操作写明口径", async () => {
    render(<StoragePanel />);
    await waitFor(() => expect(getMaxHistoryItems).toHaveBeenCalled());

    expect(tooltipOfRowContaining("历史条目上限")).toContain("默认：500 条");
    expect(tooltipOfRowContaining("单张图片阈值")).toContain("默认：20 MB");
    expect(tooltipOfRowContaining("图片占用上限")).toContain("500 MB");
    expect(tooltipOfRowContaining("立即清理非收藏历史")).toContain("无默认值");
  });
});
