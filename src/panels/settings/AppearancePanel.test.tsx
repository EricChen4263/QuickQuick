import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Mock IPC client：外观面板只依赖这五个入口
vi.mock("../../ipc/ipc-client", () => ({
  getAppearanceSettings: vi.fn(),
  setSortOrder: vi.fn(),
  setImageHeight: vi.fn(),
  setPreviewFirstDelay: vi.fn(),
  setHighlightStyle: vi.fn(),
}));

import AppearancePanel from "./AppearancePanel";
import {
  getAppearanceSettings,
  setHighlightStyle,
  setImageHeight,
  setPreviewFirstDelay,
  setSortOrder,
} from "../../ipc/ipc-client";

const mockGetAppearanceSettings = vi.mocked(getAppearanceSettings);
const mockSetSortOrder = vi.mocked(setSortOrder);
const mockSetImageHeight = vi.mocked(setImageHeight);
const mockSetPreviewFirstDelay = vi.mocked(setPreviewFirstDelay);
const mockSetHighlightStyle = vi.mocked(setHighlightStyle);

/** 取「弹出面板图片高度」输入框（按 aria-label 定位，避免与预览延迟输入框混淆） */
function imageHeightInput(): HTMLInputElement {
  const input = screen.getByLabelText("弹出面板图片高度");
  if (!(input instanceof HTMLInputElement)) throw new Error("图片高度不是 input");
  return input;
}

/** 取「首次预览延迟」输入框 */
function previewDelayInput(): HTMLInputElement {
  const input = screen.getByLabelText("首次预览延迟");
  if (!(input instanceof HTMLInputElement)) throw new Error("预览延迟不是 input");
  return input;
}

describe("AppearancePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetAppearanceSettings.mockResolvedValue({
      sortOrder: "recent",
      imageHeightPx: 24,
      previewFirstDelayMs: 1500,
      highlightStyle: "default",
    });
    mockSetSortOrder.mockResolvedValue(undefined);
    mockSetImageHeight.mockResolvedValue(undefined);
    mockSetPreviewFirstDelay.mockResolvedValue(undefined);
    mockSetHighlightStyle.mockResolvedValue(undefined);
  });

  it("挂载后把后端已存值回填到四个控件", async () => {
    // Arrange
    mockGetAppearanceSettings.mockResolvedValue({
      sortOrder: "first_copied",
      imageHeightPx: 40,
      previewFirstDelayMs: 800,
      highlightStyle: "underline",
    });

    // Act
    render(<AppearancePanel />);

    // Assert
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "排序方式" })).toHaveTextContent("首次复制");
      expect(screen.getByRole("button", { name: "命中高亮样式" })).toHaveTextContent("下划线");
      expect(imageHeightInput().value).toBe("40");
      expect(previewDelayInput().value).toBe("800");
    });
  });

  it("四项 tooltip 都写明默认值", async () => {
    // Arrange
    render(<AppearancePanel />);
    // 等首帧的读取落地，避免在 act 之外断言状态更新后的 DOM
    await waitFor(() => expect(imageHeightInput().value).toBe("24"));

    // Assert：默认值必须出现在 title 文案里（抄 Maccy 的文案约定）
    expect(screen.getByTitle(/默认：最近复制/)).toBeInTheDocument();
    expect(screen.getByTitle(/默认：24px/)).toBeInTheDocument();
    expect(screen.getByTitle(/默认：1500ms/)).toBeInTheDocument();
    expect(screen.getByTitle(/默认：背景高亮/)).toBeInTheDocument();
  });

  it("切换排序方式调用 setSortOrder(first_copied) 并回填新值", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<AppearancePanel />);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "排序方式" })).toHaveTextContent("最近复制");
    });

    // Act
    await user.click(screen.getByRole("button", { name: "排序方式" }));
    await user.click(screen.getByRole("option", { name: "首次复制" }));

    // Assert
    await waitFor(() => {
      expect(mockSetSortOrder).toHaveBeenCalledWith("first_copied");
      expect(screen.getByRole("button", { name: "排序方式" })).toHaveTextContent("首次复制");
    });
  });

  it("切换命中高亮样式调用 setHighlightStyle(underline)", async () => {
    // Arrange
    const user = userEvent.setup();
    render(<AppearancePanel />);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "命中高亮样式" })).toHaveTextContent("背景高亮");
    });

    // Act
    await user.click(screen.getByRole("button", { name: "命中高亮样式" }));
    await user.click(screen.getByRole("option", { name: "下划线" }));

    // Assert
    await waitFor(() => {
      expect(mockSetHighlightStyle).toHaveBeenCalledWith("underline");
      expect(screen.getByRole("button", { name: "命中高亮样式" })).toHaveTextContent("下划线");
    });
  });

  it("图片高度越界时拒收：不调后端、提示范围、输入框回滚到已生效值", async () => {
    // Arrange
    render(<AppearancePanel />);
    await waitFor(() => expect(imageHeightInput().value).toBe("24"));

    // Act
    fireEvent.change(imageHeightInput(), { target: { value: "999" } });
    fireEvent.blur(imageHeightInput());

    // Assert
    expect(await screen.findByRole("status")).toHaveTextContent("16–48");
    expect(mockSetImageHeight).not.toHaveBeenCalled();
    expect(imageHeightInput().value).toBe("24");
  });

  it("图片高度非整数输入同样拒收（不做「悄悄取整」）", async () => {
    // Arrange
    render(<AppearancePanel />);
    await waitFor(() => expect(imageHeightInput().value).toBe("24"));

    // Act
    fireEvent.change(imageHeightInput(), { target: { value: "24.5" } });
    fireEvent.blur(imageHeightInput());

    // Assert
    expect(mockSetImageHeight).not.toHaveBeenCalled();
    expect(imageHeightInput().value).toBe("24");
  });

  it("图片高度合法值提交后调用 setImageHeight(40)", async () => {
    // Arrange
    render(<AppearancePanel />);
    await waitFor(() => expect(imageHeightInput().value).toBe("24"));

    // Act
    fireEvent.change(imageHeightInput(), { target: { value: "40" } });
    fireEvent.blur(imageHeightInput());

    // Assert
    await waitFor(() => expect(mockSetImageHeight).toHaveBeenCalledWith(40));
    expect(imageHeightInput().value).toBe("40");
  });

  it("首次预览延迟越界时拒收：不调后端并提示 200–5000", async () => {
    // Arrange
    render(<AppearancePanel />);
    await waitFor(() => expect(previewDelayInput().value).toBe("1500"));

    // Act
    fireEvent.change(previewDelayInput(), { target: { value: "10" } });
    fireEvent.blur(previewDelayInput());

    // Assert
    expect(await screen.findByRole("status")).toHaveTextContent("200–5000");
    expect(mockSetPreviewFirstDelay).not.toHaveBeenCalled();
    expect(previewDelayInput().value).toBe("1500");
  });

  it("首次预览延迟合法值提交后调用 setPreviewFirstDelay(800)", async () => {
    // Arrange
    render(<AppearancePanel />);
    await waitFor(() => expect(previewDelayInput().value).toBe("1500"));

    // Act
    fireEvent.change(previewDelayInput(), { target: { value: "800" } });
    fireEvent.blur(previewDelayInput());

    // Assert
    await waitFor(() => expect(mockSetPreviewFirstDelay).toHaveBeenCalledWith(800));
  });

  it("后端保存失败时给出提示，且不假装已生效", async () => {
    // Arrange
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    mockSetImageHeight.mockRejectedValue(new Error("图片高度超出范围：40px"));
    render(<AppearancePanel />);
    await waitFor(() => expect(imageHeightInput().value).toBe("24"));

    // Act
    fireEvent.change(imageHeightInput(), { target: { value: "40" } });
    fireEvent.blur(imageHeightInput());

    // Assert
    expect(await screen.findByRole("status")).toHaveTextContent("保存失败");
    expect(imageHeightInput().value).toBe("24");
    error.mockRestore();
  });
});
