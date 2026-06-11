import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import SettingsPage from "./SettingsPage";

// Mock Tauri app API：隔离运行时，使 getVersion 在测试中可控
vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn(),
}));

// Mock IPC client：隔离 Tauri 运行时
vi.mock("../../ipc/ipc-client", () => ({
  getHotkeys: vi.fn(),
  setHotkey: vi.fn(),
  getExcludeList: vi.fn(),
  setExcludeList: vi.fn(),
  getTranslateProviders: vi.fn(),
  getSelectedProvider: vi.fn(),
  setSelectedProvider: vi.fn(),
  getLaunchOnLogin: vi.fn(),
  setLaunchOnLogin: vi.fn(),
  getStayInTray: vi.fn(),
  setStayInTray: vi.fn(),
  getAutoUpdate: vi.fn(),
  setAutoUpdate: vi.fn(),
  getPauseCapture: vi.fn(),
  setPauseCapture: vi.fn(),
  getSkipSensitive: vi.fn(),
  setSkipSensitive: vi.fn(),
  getStorageStats: vi.fn(),
  cleanupHistory: vi.fn(),
  getImageThreshold: vi.fn(),
  setImageThreshold: vi.fn(),
}));

import { getVersion } from "@tauri-apps/api/app";
import {
  getHotkeys,
  setHotkey,
  getExcludeList,
  setExcludeList,
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
  getLaunchOnLogin,
  setLaunchOnLogin,
  getStayInTray,
  setStayInTray,
  getAutoUpdate,
  setAutoUpdate,
  getPauseCapture,
  setPauseCapture,
  getSkipSensitive,
  setSkipSensitive,
  getStorageStats,
  cleanupHistory,
  getImageThreshold,
  setImageThreshold,
} from "../../ipc/ipc-client";

const mockGetVersion = vi.mocked(getVersion);
const mockGetHotkeys = vi.mocked(getHotkeys);
const mockSetHotkey = vi.mocked(setHotkey);
const mockGetExcludeList = vi.mocked(getExcludeList);
const mockSetExcludeList = vi.mocked(setExcludeList);
const mockGetTranslateProviders = vi.mocked(getTranslateProviders);
const mockGetSelectedProvider = vi.mocked(getSelectedProvider);
const mockSetSelectedProvider = vi.mocked(setSelectedProvider);
const mockGetLaunchOnLogin = vi.mocked(getLaunchOnLogin);
const mockGetStayInTray = vi.mocked(getStayInTray);
const mockGetAutoUpdate = vi.mocked(getAutoUpdate);
const mockSetLaunchOnLogin = vi.mocked(setLaunchOnLogin);
const mockSetStayInTray = vi.mocked(setStayInTray);
const mockSetAutoUpdate = vi.mocked(setAutoUpdate);
const mockGetPauseCapture = vi.mocked(getPauseCapture);
const mockSetPauseCapture = vi.mocked(setPauseCapture);
const mockGetSkipSensitive = vi.mocked(getSkipSensitive);
const mockSetSkipSensitive = vi.mocked(setSkipSensitive);
const mockGetStorageStats = vi.mocked(getStorageStats);
const mockCleanupHistory = vi.mocked(cleanupHistory);
const mockGetImageThreshold = vi.mocked(getImageThreshold);
const mockSetImageThreshold = vi.mocked(setImageThreshold);

const MOCK_HOTKEYS = {
  history: "CmdOrCtrl+Shift+H",
  translate: "CmdOrCtrl+Shift+T",
  main: "CmdOrCtrl+Shift+M",
};
const MOCK_PROVIDERS = [
  { id: "google", name: "Google 翻译", needsKey: false, needsConfig: false, isUnofficial: true },
  { id: "deepl", name: "DeepL", needsKey: true, needsConfig: true, isUnofficial: false },
];
const MOCK_EXCLUDE_LIST = ["Xcode", "Terminal"];

describe("settings-page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetVersion.mockResolvedValue("0.0.1");
    mockGetHotkeys.mockResolvedValue(MOCK_HOTKEYS);
    mockSetHotkey.mockResolvedValue(undefined);
    mockGetExcludeList.mockResolvedValue(MOCK_EXCLUDE_LIST);
    mockSetExcludeList.mockResolvedValue(undefined);
    mockGetTranslateProviders.mockResolvedValue(MOCK_PROVIDERS);
    mockGetSelectedProvider.mockResolvedValue("google");
    mockSetSelectedProvider.mockResolvedValue(undefined);
    mockGetLaunchOnLogin.mockResolvedValue(true);
    mockGetStayInTray.mockResolvedValue(true);
    mockGetAutoUpdate.mockResolvedValue(true);
    mockSetLaunchOnLogin.mockResolvedValue(undefined);
    mockSetStayInTray.mockResolvedValue(undefined);
    mockSetAutoUpdate.mockResolvedValue(undefined);
    mockGetPauseCapture.mockResolvedValue(false);
    mockSetPauseCapture.mockResolvedValue(undefined);
    mockGetSkipSensitive.mockResolvedValue(true);
    mockSetSkipSensitive.mockResolvedValue(undefined);
    mockGetStorageStats.mockResolvedValue({ liveCount: 42, fileSizeBytes: 50 * 1024 * 1024 });
    mockCleanupHistory.mockResolvedValue({ softDeleted: 5, purged: 3 });
    mockGetImageThreshold.mockResolvedValue(20 * 1024 * 1024);
    mockSetImageThreshold.mockResolvedValue(undefined);
  });

  it("settings-page: 左侧纵向子项栏渲染六个子项（通用/热键/翻译源/隐私/存储/关于）", async () => {
    // Arrange & Act
    render(<SettingsPage />);

    // Assert：六个子项全部渲染
    const nav = screen.getByRole("navigation", { name: "设置子项" });
    expect(within(nav).getByRole("button", { name: "通用" })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "热键" })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "翻译源" })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "隐私" })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "存储" })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "关于" })).toBeInTheDocument();
  });

  it("settings-page: 默认选中通用，点击热键后右内容切换（DOM 变化）", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    // Assert：默认通用内容可见
    const nav = screen.getByRole("navigation", { name: "设置子项" });
    expect(within(nav).getByRole("button", { name: "通用" })).toHaveAttribute("aria-current", "page");

    // Act：点击热键
    await user.click(within(nav).getByRole("button", { name: "热键" }));

    // Assert：热键子项获得选中态
    expect(within(nav).getByRole("button", { name: "热键" })).toHaveAttribute("aria-current", "page");
    expect(within(nav).getByRole("button", { name: "通用" })).not.toHaveAttribute("aria-current");
  });

  it("settings-page: 热键面板——捕获与另一动作相同的键显示已被占用且不调用 setHotkey", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "热键" }));

    // 等待热键数据加载完成（三行「修改」按钮出现）
    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3);
    });

    // Act：点击第一行「修改」进入录制模式
    const editButtons = screen.getAllByRole("button", { name: "修改" });
    await user.click(editButtons[0]);

    // 录制模式下模拟按下 CmdOrCtrl+Shift+T（与 translate 键冲突）
    const captureArea = screen.getByRole("button", { name: "录制中…请按下快捷键" });
    fireEvent.keyDown(captureArea, { code: "KeyT", metaKey: true, shiftKey: true });

    // 点击保存
    await user.click(screen.getByRole("button", { name: "保存" }));

    // Assert：显示"已被占用"，setHotkey 不被调用
    await waitFor(() => {
      expect(screen.getByText("已被占用")).toBeInTheDocument();
    });
    expect(mockSetHotkey).not.toHaveBeenCalled();
  });

  it("settings-page: 热键面板——捕获不冲突键后调用 setHotkey(正确参数)", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "热键" }));

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3);
    });

    // Act：点击第一行「修改」进入录制模式
    const editButtons = screen.getAllByRole("button", { name: "修改" });
    await user.click(editButtons[0]);

    // 录制模式下模拟按下 CmdOrCtrl+Shift+Y（不冲突）
    const captureArea = screen.getByRole("button", { name: "录制中…请按下快捷键" });
    fireEvent.keyDown(captureArea, { code: "KeyY", metaKey: true, shiftKey: true });

    // 点击保存
    await user.click(screen.getByRole("button", { name: "保存" }));

    // Assert：调用 setHotkey 传正确参数
    await waitFor(() => {
      expect(mockSetHotkey).toHaveBeenCalledWith("history", "CmdOrCtrl+Shift+Y");
    });
  });

  it("settings-page: 热键面板——渲染应用主界面热键行并保存 main action", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "热键" }));

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3);
    });
    expect(screen.getByText("应用主界面")).toBeInTheDocument();
    expect(screen.getByText("打开并聚焦 QuickQuick 主窗口")).toBeInTheDocument();

    // Act：第三行 Main 热键改成不冲突键
    const editButtons = screen.getAllByRole("button", { name: "修改" });
    await user.click(editButtons[2]);
    const captureArea = screen.getByRole("button", { name: "录制中…请按下快捷键" });
    fireEvent.keyDown(captureArea, { code: "KeyQ", metaKey: true, shiftKey: true });
    await user.click(screen.getByRole("button", { name: "保存" }));

    // Assert
    await waitFor(() => {
      expect(mockSetHotkey).toHaveBeenCalledWith("main", "CmdOrCtrl+Shift+Q");
    });
  });

  it("settings-page: 热键面板——main 行捕获 history 键显示已被占用且不调用 setHotkey", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "热键" }));

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3);
    });

    // Act：第三行 Main 捕获与 history 相同的键
    const editButtons = screen.getAllByRole("button", { name: "修改" });
    await user.click(editButtons[2]);
    const captureArea = screen.getByRole("button", { name: "录制中…请按下快捷键" });
    fireEvent.keyDown(captureArea, { code: "KeyH", metaKey: true, shiftKey: true });
    await user.click(screen.getByRole("button", { name: "保存" }));

    // Assert
    await waitFor(() => {
      expect(screen.getByText("已被占用")).toBeInTheDocument();
    });
    expect(mockSetHotkey).not.toHaveBeenCalled();
  });

  it("settings-page: 隐私面板——添加一项后列表出现该项且调 setExcludeList(含该项)", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "隐私" }));

    // 等待排除名单加载
    await waitFor(() => {
      expect(screen.getByText("Xcode")).toBeInTheDocument();
    });

    // Act：输入新 App 名称并添加
    const addInput = screen.getByPlaceholderText(/应用名称/);
    await user.type(addInput, "Safari");
    await user.click(screen.getByRole("button", { name: "添加" }));

    // Assert：列表出现 Safari，setExcludeList 被调用且含 Safari
    await waitFor(() => {
      expect(screen.getByText("Safari")).toBeInTheDocument();
    });
    expect(mockSetExcludeList).toHaveBeenCalledWith(
      expect.arrayContaining(["Xcode", "Terminal", "Safari"])
    );
  });

  it("settings-page: 隐私面板——删除一项后调 setExcludeList(不含该项)", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "隐私" }));

    await waitFor(() => {
      expect(screen.getByText("Xcode")).toBeInTheDocument();
    });

    // Act：点击 Xcode 的删除按钮
    const deleteButtons = screen.getAllByRole("button", { name: /删除/ });
    await user.click(deleteButtons[0]);

    // Assert：setExcludeList 被调用且不含 Xcode
    await waitFor(() => {
      expect(mockSetExcludeList).toHaveBeenCalledWith(
        expect.not.arrayContaining(["Xcode"])
      );
    });
    expect(mockSetExcludeList).toHaveBeenCalledWith(["Terminal"]);
  });

  it("settings-page: 翻译源面板——渲染 providers 列表，选一个调 setSelectedProvider(id)", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "翻译源" }));

    // 等待 providers 加载
    await waitFor(() => {
      expect(screen.getByText("Google 翻译")).toBeInTheDocument();
      expect(screen.getByText("DeepL")).toBeInTheDocument();
    });

    // Act：选择 DeepL
    await user.click(screen.getByRole("radio", { name: "DeepL" }));

    // Assert：调用 setSelectedProvider("deepl")
    await waitFor(() => {
      expect(mockSetSelectedProvider).toHaveBeenCalledWith("deepl");
    });
  });

  it("settings-page: 热键面板加载失败时显示错误提示（role=alert）", async () => {
    // Arrange
    mockGetHotkeys.mockRejectedValue(new Error("IPC error"));
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "热键" }));

    // Assert：显示错误提示
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
  });

  it("settings-page: 翻译源面板——setSelectedProvider reject 时列表仍可见且显示 opError 提示", async () => {
    // Arrange
    mockSetSelectedProvider.mockRejectedValue(new Error("IPC error"));
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "翻译源" }));

    await waitFor(() => {
      expect(screen.getByText("Google 翻译")).toBeInTheDocument();
      expect(screen.getByText("DeepL")).toBeInTheDocument();
    });

    // Act：点击 DeepL，setSelectedProvider 将 reject
    await user.click(screen.getByRole("radio", { name: "DeepL" }));

    // Assert：provider 列表仍在 DOM 中（未被替换）
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(screen.getByText("Google 翻译")).toBeInTheDocument();
    expect(screen.getByText("DeepL")).toBeInTheDocument();
  });

  it("settings-page: 关于面板显示应用名 QuickQuick", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "关于" }));

    // Assert：关于页显示应用名（文本被 brand-accent span 拆分，按可访问名命中）
    expect(screen.getByRole("heading", { level: 2, name: "QuickQuick" })).toBeInTheDocument();
  });

  it("settings-page: 通用面板渲染三个 switch（开机自启动/托盘常驻/自动检查更新）默认均为 on", () => {
    render(<SettingsPage />);

    expect(screen.getByRole("switch", { name: "开机自启动" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("switch", { name: "托盘常驻" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("switch", { name: "自动检查更新" })).toHaveAttribute("aria-checked", "true");
  });

  it("settings-page: 通用面板点击「开机自启动」switch 后 aria-checked 切换为 false", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const sw = screen.getByRole("switch", { name: "开机自启动" });
    await user.click(sw);

    expect(sw).toHaveAttribute("aria-checked", "false");
    // 其余两项不受影响
    expect(screen.getByRole("switch", { name: "托盘常驻" })).toHaveAttribute("aria-checked", "true");
  });

  it("settings-page: 通用面板子项导航按钮文案含图标但 getByRole('button') 仍可命中（aria-hidden 图标不干扰名称）", () => {
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    // 六个 set-nav-item 按钮均可按文案命中
    expect(within(nav).getByRole("button", { name: "通用" })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "存储" })).toBeInTheDocument();
  });

  it("settings-page: 存储面板渲染 PanelHeader 标题「存储」及清理按钮", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "存储" }));

    // 标题和清理按钮均可见
    expect(screen.getByRole("heading", { name: "存储" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "清理…" })).toBeInTheDocument();
  });

  it("settings-page: 关于面板 logo 块渲染应用 icon 图片而非内联 SVG", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "关于" }));

    // logo 块渲染 <img alt="QuickQuick">，不再有内联 svg
    const logoImg = screen.getByRole("img", { name: "QuickQuick" });
    expect(logoImg.tagName).toBe("IMG");
    expect(logoImg.closest(".logo-mark")?.querySelector("svg")).toBeNull();
  });

  it("settings-page: 关于面板应用名套标题栏 brand-accent 样式（Quick 高亮）", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "关于" }));

    // h2 整体读出 QuickQuick，且前半「Quick」用 .brand-accent span 高亮
    const heading = screen.getByRole("heading", { level: 2, name: "QuickQuick" });
    const accent = heading.querySelector("span.brand-accent");
    expect(accent).not.toBeNull();
    expect(accent?.textContent).toBe("Quick");
  });

  it("settings-page: 翻译源面板——每个 provider 渲染 .src-card 结构（设计系统重塑）", async () => {
    // Arrange: 验证改造后 TranslateSourcePanel 使用 .src-card/.src-logo/.badge 结构
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "翻译源" }));

    await waitFor(() => {
      expect(screen.getByText("Google 翻译")).toBeInTheDocument();
    });

    // 初始选中 google → google 显示 "默认"，DeepL needsConfig=true → "待配置"
    expect(screen.getByText("默认")).toBeInTheDocument();
    expect(screen.getByText("待配置")).toBeInTheDocument();

    // 选择 DeepL 后：DeepL 变为选中显示"默认"，Google(needsConfig=false)显示"无需配置"
    await user.click(screen.getByRole("radio", { name: "DeepL" }));
    await waitFor(() => {
      expect(screen.getByText("无需配置")).toBeInTheDocument();
    });
    // DeepL 此时是选中态，显示"默认"（之前"待配置"消失）
    expect(screen.getByText("默认")).toBeInTheDocument();
    expect(screen.queryByText("待配置")).not.toBeInTheDocument();
  });

  it("settings-page: 热键面板——每行有 kbd 芯片展示当前键和「修改」按钮", async () => {
    // Arrange: 验证改造后 HotkeyPanel 每行有 kbd 展示当前热键、有「修改」按钮
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "热键" }));

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3);
    });

    // 三行都有「修改」按钮，当前无「保存」按钮（未进录制模式）
    expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3);
    expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
    // 无旧版 input（已移除手打改键）
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("settings-page: 热键面板——点「修改」进入录制模式，Esc 取消还原到「修改」按钮", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "热键" }));

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3);
    });

    // Act：点击第一行「修改」进入录制模式
    const editButtons = screen.getAllByRole("button", { name: "修改" });
    await user.click(editButtons[0]);

    // 录制模式中：「修改」消失、出现录制中按钮
    expect(screen.queryAllByRole("button", { name: "修改" })).toHaveLength(2);
    const captureArea = screen.getByRole("button", { name: "录制中…请按下快捷键" });
    expect(captureArea).toBeInTheDocument();

    // 按 Esc 取消
    fireEvent.keyDown(captureArea, { code: "Escape" });

    // 退出录制模式：「修改」按钮恢复为 3，录制中按钮消失
    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3);
    });
    expect(screen.queryByRole("button", { name: "录制中…请按下快捷键" })).not.toBeInTheDocument();
  });

  it("settings-page: 热键面板——未捕获到有效键时「保存」按钮禁用", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "热键" }));

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3);
    });

    // Act：进入录制模式但不按有效键
    const editButtons = screen.getAllByRole("button", { name: "修改" });
    await user.click(editButtons[0]);

    // 尝试按纯修饰键（无有效主键）
    const captureArea = screen.getByRole("button", { name: "录制中…请按下快捷键" });
    fireEvent.keyDown(captureArea, { code: "ShiftLeft", shiftKey: true });

    // Assert：「保存」按钮禁用
    const saveBtn = screen.getByRole("button", { name: "保存" });
    expect(saveBtn).toBeDisabled();
  });

  it("settings-page: 隐私面板——App 名单以 .chip 形式渲染（设计系统重塑）", async () => {
    // Arrange: 验证改造后 PrivacyPanel 用 chip-row/chip 结构渲染排除名单
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "隐私" }));

    await waitFor(() => {
      expect(screen.getByText("Xcode")).toBeInTheDocument();
      expect(screen.getByText("Terminal")).toBeInTheDocument();
    });

    // 添加 input 的 placeholder 可命中
    expect(screen.getByPlaceholderText(/应用名称/)).toBeInTheDocument();
    // 每个 app 有对应删除按钮（aria-label="删除 xxx"）
    expect(screen.getByRole("button", { name: "删除 Xcode" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "删除 Terminal" })).toBeInTheDocument();
  });

  it("settings-page: 隐私面板——mount 时调用 getPauseCapture 和 getSkipSensitive", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "隐私" }));

    await waitFor(() => {
      expect(mockGetPauseCapture).toHaveBeenCalledTimes(1);
      expect(mockGetSkipSensitive).toHaveBeenCalledTimes(1);
    });
  });

  it("settings-page: 隐私面板——toggle「暂停剪贴板监听」调用 setPauseCapture", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "隐私" }));

    await waitFor(() => {
      expect(mockGetPauseCapture).toHaveBeenCalled();
    });

    const pauseSwitch = screen.getByRole("switch", { name: "暂停剪贴板监听" });
    await user.click(pauseSwitch);

    await waitFor(() => {
      expect(mockSetPauseCapture).toHaveBeenCalledWith(true);
    });
  });

  it("settings-page: 隐私面板——toggle「跳过敏感内容」调用 setSkipSensitive", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "隐私" }));

    await waitFor(() => {
      expect(mockGetSkipSensitive).toHaveBeenCalled();
    });

    const skipSwitch = screen.getByRole("switch", { name: "跳过敏感内容" });
    await user.click(skipSwitch);

    await waitFor(() => {
      expect(mockSetSkipSensitive).toHaveBeenCalledWith(false);
    });
  });

  it("settings-page: 存储面板——mount 时调用 getStorageStats 并显示条目数", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "存储" }));

    await waitFor(() => {
      expect(mockGetStorageStats).toHaveBeenCalledTimes(1);
      expect(screen.getByText(/42 条/)).toBeInTheDocument();
    });
  });

  it("settings-page: 存储面板——条目显示已用 MB 和上限 500 MB", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "存储" }));

    await waitFor(() => {
      expect(screen.getByText(/50\.0 MB 已用/)).toBeInTheDocument();
      expect(screen.getByText(/上限 500 MB/)).toBeInTheDocument();
    });
  });

  it("settings-page: 存储面板——点击清理按钮调用 cleanupHistory 并刷新统计", async () => {
    mockCleanupHistory.mockResolvedValue({ softDeleted: 5, purged: 3 });
    mockGetStorageStats
      .mockResolvedValueOnce({ liveCount: 42, fileSizeBytes: 50 * 1024 * 1024 })
      .mockResolvedValueOnce({ liveCount: 37, fileSizeBytes: 45 * 1024 * 1024 });

    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "存储" }));

    await waitFor(() => {
      expect(screen.getByText(/42 条/)).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: "清理…" }));

    await waitFor(() => {
      expect(mockCleanupHistory).toHaveBeenCalledTimes(1);
      expect(mockGetStorageStats).toHaveBeenCalledTimes(2);
      expect(screen.getByText(/37 条/)).toBeInTheDocument();
    });
  });

  it("settings-page: 热键面板不渲染「回车粘贴」占位开关（已移除本地占位）", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "热键" }));

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3);
    });

    expect(screen.queryByText("回车粘贴")).not.toBeInTheDocument();
  });

  it("settings-page: 关于面板版本号从 getVersion 读取（非硬编码 v1.0.0），且不含 Tauri", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const nav = screen.getByRole("navigation", { name: "设置子项" });
    await user.click(within(nav).getByRole("button", { name: "关于" }));

    await waitFor(() => {
      expect(screen.getByText(/v0\.0\.1/)).toBeInTheDocument();
    });
    expect(screen.queryByText(/v1\.0\.0/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Tauri/)).not.toBeInTheDocument();
  });
});
