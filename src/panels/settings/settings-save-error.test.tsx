import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

/**
 * 面板侧 mock：隐私 / 翻译源 / 热键三个面板的 IPC 入口。
 * 这里只覆盖「保存失败要出现在页面级错误条」这条链路需要的函数。
 */
vi.mock("../../ipc/ipc-client", () => ({
  // 隐私
  getExcludeList: vi.fn().mockResolvedValue([]),
  setExcludeList: vi.fn().mockResolvedValue(undefined),
  getExcludeMode: vi.fn().mockResolvedValue("blacklist"),
  setExcludeMode: vi.fn().mockResolvedValue(undefined),
  getIgnoreRegexes: vi.fn().mockResolvedValue([]),
  setIgnoreRegexes: vi.fn().mockResolvedValue(undefined),
  getPauseCapture: vi.fn().mockResolvedValue(false),
  setPauseCapture: vi.fn().mockResolvedValue(undefined),
  getSkipSensitive: vi.fn().mockResolvedValue(true),
  setSkipSensitive: vi.fn().mockResolvedValue(undefined),
  // 翻译源
  getTranslateProviders: vi.fn().mockResolvedValue([
    { id: "google_free", name: "Google（免费）" },
    { id: "deepl", name: "DeepL" },
  ]),
  getSelectedProvider: vi.fn().mockResolvedValue("google_free"),
  setSelectedProvider: vi.fn().mockResolvedValue(undefined),
  getProviderCredentialSchema: vi.fn().mockResolvedValue([]),
  getProviderCredentials: vi.fn().mockResolvedValue([]),
  // 热键
  getHotkeys: vi.fn().mockResolvedValue({ history: "CmdOrCtrl+Shift+C", translate: "CmdOrCtrl+Shift+T", main: "CmdOrCtrl+Shift+M" }),
  setHotkey: vi.fn().mockResolvedValue(undefined),
  // 通用（SettingsPage 默认停在「通用」）
  getLaunchOnLogin: vi.fn().mockResolvedValue(true),
  setLaunchOnLogin: vi.fn().mockResolvedValue(undefined),
  getStayInTray: vi.fn().mockResolvedValue(true),
  setStayInTray: vi.fn().mockResolvedValue(undefined),
  getAutoUpdate: vi.fn().mockResolvedValue(true),
  setAutoUpdate: vi.fn().mockResolvedValue(undefined),
  checkForUpdates: vi.fn().mockResolvedValue({ available: false, version: "", currentVersion: "1.0.0" }),
  downloadAndInstallUpdate: vi.fn().mockResolvedValue(undefined),
  // 外观（切到「外观」子项时会挂载 AppearancePanel）
  getAppearanceSettings: vi.fn().mockResolvedValue({
    sortOrder: "recent",
    imageHeightPx: 40,
    previewFirstDelayMs: 1500,
    highlightStyle: "bold",
  }),
  setSortOrder: vi.fn().mockResolvedValue(undefined),
  setImageHeight: vi.fn().mockResolvedValue(undefined),
  setPreviewFirstDelay: vi.fn().mockResolvedValue(undefined),
  setHighlightStyle: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("0.4.0"),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import {
  getExcludeList,
  getHotkeys,
  setExcludeList,
  setLaunchOnLogin,
  setSkipSensitive,
  setSelectedProvider,
  setStayInTray,
  setHotkey,
} from "../../ipc/ipc-client";
import {
  SettingsSaveErrorBanner,
  SettingsSaveErrorProvider,
  saveErrorMessage,
  useSettingsSaveError,
} from "./settings-save-error";
import PrivacyPanel from "./PrivacyPanel";
import TranslateSourcePanel from "./TranslateSourcePanel";
import HotkeyPanel from "./HotkeyPanel";
import SettingsPage from "./SettingsPage";
import GeneralPanel from "./GeneralPanel";

const BACKEND_MESSAGE =
  "settings.json 无法解析（JSON 语法错误），已拒绝写入以免覆盖你的设置；请先修复该文件。备份副本：settings.json.1.bak";

function renderWithBanner(ui: React.ReactElement) {
  return render(
    <SettingsSaveErrorProvider>
      {ui}
      <SettingsSaveErrorBanner />
    </SettingsSaveErrorProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("saveErrorMessage", () => {
  it("Error 取 message、字符串原样、其它值转字符串", () => {
    expect(saveErrorMessage(new Error(BACKEND_MESSAGE))).toBe(BACKEND_MESSAGE);
    expect(saveErrorMessage(BACKEND_MESSAGE)).toBe(BACKEND_MESSAGE);
    expect(saveErrorMessage(42)).toBe("42");
  });
});

describe("SettingsSaveErrorChannel（按来源隔离）", () => {
  /** 直接使用通道的探针组件：暴露 report/clear/runSave 给测试调用 */
  function Probe({ onReady }: { onReady: (channel: ReturnType<typeof useSettingsSaveError>) => void }) {
    const channel = useSettingsSaveError();
    onReady(channel);
    return null;
  }

  it("reportSaveError 显示后端原文，clearSaveError 清掉该来源", async () => {
    let channel!: ReturnType<typeof useSettingsSaveError>;
    renderWithBanner(<Probe onReady={(c) => { channel = c; }} />);

    act(() => channel.reportSaveError("appearance.sortOrder", new Error(BACKEND_MESSAGE)));
    expect(await screen.findByRole("alert")).toHaveTextContent("已拒绝写入");

    act(() => channel.clearSaveError("appearance.sortOrder"));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("两个来源同时失败时都能看到；清一个不影响另一个", async () => {
    let channel!: ReturnType<typeof useSettingsSaveError>;
    renderWithBanner(<Probe onReady={(c) => { channel = c; }} />);

    act(() => {
      channel.reportSaveError("general.launchOnLogin", new Error("开机自启动失败"));
      channel.reportSaveError("appearance.imageHeight", new Error("图片高度失败"));
    });

    const banner = await screen.findByRole("alert");
    expect(banner).toHaveTextContent("开机自启动失败");
    expect(banner).toHaveTextContent("图片高度失败");

    act(() => channel.clearSaveError("appearance.imageHeight"));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent("开机自启动失败");
    });
    expect(screen.getByRole("alert")).not.toHaveTextContent("图片高度失败");
  });

  it("runSave 失败时上报该来源原文，成功后只清该来源", async () => {
    let channel!: ReturnType<typeof useSettingsSaveError>;
    renderWithBanner(<Probe onReady={(c) => { channel = c; }} />);

    // 先让另一个来源失败，验证 runSave 成功不会顺手清掉它
    act(() => channel.reportSaveError("general.stayInTray", new Error("别的来源失败")));

    let result!: Awaited<ReturnType<typeof channel.runSave>>;
    await act(async () => {
      result = await channel.runSave("appearance.imageHeight", () =>
        Promise.reject(new Error(BACKEND_MESSAGE)),
      );
    });
    expect(result).toEqual({ ok: false, error: BACKEND_MESSAGE });
    expect(await screen.findByRole("alert")).toHaveTextContent("settings.json.1.bak");

    await act(async () => {
      result = await channel.runSave("appearance.imageHeight", () => Promise.resolve("done"));
    });
    expect(result).toEqual({ ok: true, value: "done" });

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent("别的来源失败");
    });
    expect(screen.getByRole("alert")).not.toHaveTextContent("已拒绝写入");
  });

  it("错误条上的「知道了」清空所有来源", async () => {
    let channel!: ReturnType<typeof useSettingsSaveError>;
    renderWithBanner(<Probe onReady={(c) => { channel = c; }} />);

    act(() => {
      channel.reportSaveError("general.launchOnLogin", new Error("A 失败"));
      channel.reportSaveError("storage.historyLimit", new Error("B 失败"));
    });
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "知道了" }));

    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("无 Provider 时兜底不留白：reportSaveError 走 console.error，不静默", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    let channel!: ReturnType<typeof useSettingsSaveError>;
    render(<Probe onReady={(c) => { channel = c; }} />);

    act(() => channel.reportSaveError("general.autoUpdate", new Error("兜底来源失败")));

    expect(errorSpy).toHaveBeenCalledWith(
      expect.stringContaining("无页面级错误条"),
      "兜底来源失败",
    );
    errorSpy.mockRestore();
  });

  it("无 Provider 时 runSave 仍返回 ok=false 并 console.error", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    let channel!: ReturnType<typeof useSettingsSaveError>;
    render(<Probe onReady={(c) => { channel = c; }} />);

    let result!: Awaited<ReturnType<typeof channel.runSave>>;
    await act(async () => {
      result = await channel.runSave("storage.cleanup", () => Promise.reject(new Error("审计失败")));
    });

    expect(result).toEqual({ ok: false, error: "审计失败" });
    expect(errorSpy).toHaveBeenCalledWith(
      expect.stringContaining("无页面级错误条"),
      "审计失败",
    );
    errorSpy.mockRestore();
  });
});

describe("各面板的保存失败都路由到页面级错误条", () => {
  it("隐私：跳过敏感内容保存失败 → 错误条显示后端原文，开关不假装生效", async () => {
    vi.mocked(setSkipSensitive).mockRejectedValueOnce(new Error(BACKEND_MESSAGE));
    const user = userEvent.setup();
    renderWithBanner(<PrivacyPanel />);

    const toggle = await screen.findByRole("switch", { name: "跳过敏感内容" });
    expect(toggle).toHaveAttribute("aria-checked", "true");

    await user.click(toggle);

    expect(await screen.findByRole("alert")).toHaveTextContent("已拒绝写入");
    expect(screen.getByRole("alert")).toHaveTextContent("settings.json.1.bak");
    expect(screen.getByRole("switch", { name: "跳过敏感内容" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("翻译源：切换翻译源失败 → 错误条显示后端原文", async () => {
    vi.mocked(setSelectedProvider).mockRejectedValueOnce(new Error(BACKEND_MESSAGE));
    const user = userEvent.setup();
    renderWithBanner(<TranslateSourcePanel />);

    const deeplOption = await screen.findByText("DeepL");
    await user.click(deeplOption);

    expect(await screen.findByRole("alert")).toHaveTextContent("已拒绝写入");
  });

  it("热键：保存热键失败 → 错误条显示后端原文，本行不显示原始错误", async () => {
    vi.mocked(setHotkey).mockRejectedValueOnce(new Error(BACKEND_MESSAGE));
    renderWithBanner(<HotkeyPanel />);

    await waitFor(() => expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3));
    const user = userEvent.setup();
    await user.click(screen.getAllByRole("button", { name: "修改" })[0]);

    // 录制中：在捕获区按下 ⌘⇧K（与另外两行默认键位不冲突），再点保存
    const capture = screen.getByRole("button", { name: "录制中…请按下快捷键" });
    fireEvent.keyDown(capture, {
      key: "k",
      code: "KeyK",
      metaKey: true,
      shiftKey: true,
    });
    await user.click(await screen.findByRole("button", { name: "保存" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("已拒绝写入");
  });

  it("设置页：面板里的保存失败出现在页面顶部错误条上（整页接线）", async () => {
    vi.mocked(setLaunchOnLogin).mockRejectedValueOnce(new Error(BACKEND_MESSAGE));
    const user = userEvent.setup();
    render(<SettingsPage />);

    const toggle = await screen.findByRole("switch", { name: "开机自启动" });
    await user.click(toggle);

    expect(await screen.findByRole("alert")).toHaveTextContent("已拒绝写入");
    expect(screen.getByRole("alert")).toHaveTextContent("settings.json.1.bak");
    expect(screen.getByRole("switch", { name: "开机自启动" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("热键：配置损坏时展示修复指引，键位行仍渲染（不整块失效）", async () => {
    const backupPath =
      "/Users/tester/Library/Application Support/QuickQuick/hotkey.json.1792899443151.bak";
    vi.mocked(getHotkeys).mockResolvedValueOnce({
      history: "CmdOrCtrl+Shift+C",
      translate: "CmdOrCtrl+Shift+T",
      main: "CmdOrCtrl+Shift+M",
      corrupt: {
        file: "hotkey.json",
        backupPath,
        hint: `热键配置文件已损坏（JSON 语法错误：expected value at line 1 column 1），已备份到 ${backupPath}。请修复或删除该文件后重启应用；在此之前使用默认键位。`,
      },
    });
    renderWithBanner(<HotkeyPanel />);

    // 修复指引可见，且指向真实备份路径
    expect(await screen.findByText("热键配置文件已损坏")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("已备份到");
    expect(screen.getByRole("alert")).toHaveTextContent("hotkey.json.1792899443151.bak");
    expect(screen.getByRole("alert")).not.toHaveTextContent("稍后重试");

    // 键位行照常渲染（三行都能改键），不是「加载失败」整块替换
    await waitFor(() => expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3));
  });

  it("热键：正常配置不显示损坏提示", async () => {
    vi.mocked(getHotkeys).mockResolvedValueOnce({
      history: "CmdOrCtrl+Shift+C",
      translate: "CmdOrCtrl+Shift+T",
      main: "CmdOrCtrl+Shift+M",
      corrupt: null,
    });
    renderWithBanner(<HotkeyPanel />);

    await waitFor(() => expect(screen.getAllByRole("button", { name: "修改" })).toHaveLength(3));
    expect(screen.queryByText("热键配置文件已损坏")).not.toBeInTheDocument();
  });

  it("通用：A 开关失败后 B 开关成功，A 的错误不被抹掉（C 按来源隔离）", async () => {
    vi.mocked(setLaunchOnLogin).mockRejectedValueOnce(new Error("开机自启动：已拒绝写入"));
    vi.mocked(setStayInTray).mockResolvedValueOnce(undefined);
    const user = userEvent.setup();
    render(<SettingsPage />);

    await user.click(await screen.findByRole("switch", { name: "开机自启动" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("开机自启动：已拒绝写入");

    // B 开关成功：只清 general.stayInTray（它本来就没失败），A 的错误必须还在
    await user.click(screen.getByRole("switch", { name: "托盘常驻" }));
    await waitFor(() => expect(setStayInTray).toHaveBeenCalled());

    expect(screen.getByRole("alert")).toHaveTextContent("开机自启动：已拒绝写入");
    expect(screen.getByRole("switch", { name: "托盘常驻" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });

  it("通用面板挂载时不清空别处留下的错误（D 挂载即清空）", async () => {
    let channel!: ReturnType<typeof useSettingsSaveError>;
    function Probe({
      onReady,
    }: {
      onReady: (c: ReturnType<typeof useSettingsSaveError>) => void;
    }) {
      const current = useSettingsSaveError();
      onReady(current);
      return null;
    }

    const { rerender } = render(
      <SettingsSaveErrorProvider>
        <Probe onReady={(c) => { channel = c; }} />
        <SettingsSaveErrorBanner />
      </SettingsSaveErrorProvider>,
    );

    // 别的面板留下了错误
    act(() => channel.reportSaveError("appearance.imageHeight", new Error("图片高度被拒写")));
    expect(await screen.findByRole("alert")).toHaveTextContent("图片高度被拒写");

    // 挂载通用面板：它自己没有失败，不得清除上面的错误
    rerender(
      <SettingsSaveErrorProvider>
        <GeneralPanel />
        <SettingsSaveErrorBanner />
      </SettingsSaveErrorProvider>,
    );
    await waitFor(() => expect(screen.getByRole("switch", { name: "开机自启动" })).toBeInTheDocument());

    expect(screen.getByRole("alert")).toHaveTextContent("图片高度被拒写");
  });

  it("通用：卸载重挂后同开关保存成功，旧错误不残留（成功路径显式清除）", async () => {
    vi.mocked(setLaunchOnLogin).mockRejectedValueOnce(new Error("开机自启动：已拒绝写入"));
    const user = userEvent.setup();
    render(<SettingsPage />);

    await user.click(await screen.findByRole("switch", { name: "开机自启动" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("开机自启动：已拒绝写入");

    // 切到「外观」再切回「通用」：GeneralPanel 被卸载后重挂
    await user.click(screen.getByRole("button", { name: "外观" }));
    await waitFor(() =>
      expect(screen.queryByRole("switch", { name: "开机自启动" })).not.toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "通用" }));

    // 同开关这次成功：错误条应当消失（清除发生在成功路径上，不依赖挂载时机）
    await user.click(await screen.findByRole("switch", { name: "开机自启动" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("隐私：添加名单失败后删除成功，添加的错误不被抹掉（跨来源不误清）", async () => {
    vi.mocked(setExcludeList).mockRejectedValueOnce(new Error("添加失败：已拒绝写入"));
    vi.mocked(setExcludeList).mockResolvedValueOnce(undefined);
    vi.mocked(getExcludeList).mockResolvedValue(["钉钉"]);
    const user = userEvent.setup();
    renderWithBanner(<PrivacyPanel />);

    // 初始名单里有一条「钉钉」，删除按钮可用
    const removeButton = await screen.findByRole("button", { name: /删除 钉钉/ });
    const input = screen.getByPlaceholderText("应用名称（如 1Password）");
    await user.type(input, "飞书");
    await user.click(screen.getByRole("button", { name: "添加应用" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("添加失败：已拒绝写入");

    // 删除成功：只清 privacy.removeApp，添加的错误必须还在
    await user.click(removeButton);

    await waitFor(() => expect(setExcludeList).toHaveBeenCalledTimes(2));
    expect(screen.getByRole("alert")).toHaveTextContent("添加失败：已拒绝写入");
  });
});
