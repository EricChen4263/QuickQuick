import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import SelectIconApp from "./SelectIconApp";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("SelectIconApp", () => {
  it("点击图标恰好请求一次翻译", () => {
    // Arrange
    const onTranslate = vi.fn().mockResolvedValue(undefined);
    render(<SelectIconApp onTranslate={onTranslate} />);

    // Act
    fireEvent.click(screen.getByRole("button", { name: "翻译选中文字" }));

    // Assert
    expect(onTranslate).toHaveBeenCalledTimes(1);
  });

  it("翻译请求失败时记录告警而不是抛出未处理异常", async () => {
    // Arrange
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const failure = new Error("IPC 不可用");
    const onTranslate = vi.fn().mockRejectedValue(failure);
    render(<SelectIconApp onTranslate={onTranslate} />);

    // Act
    fireEvent.click(screen.getByRole("button", { name: "翻译选中文字" }));

    // Assert
    await waitFor(() => {
      expect(warn).toHaveBeenCalledWith("[select-icon] 翻译请求失败:", failure);
    });
  });
});
