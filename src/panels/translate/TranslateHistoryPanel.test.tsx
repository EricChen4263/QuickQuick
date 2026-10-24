import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import type { TranslateHistoryItem } from "../../ipc/ipc-client";
import TranslateHistoryPanel, { TRANSLATE_HISTORY_PAGE_LIMIT } from "./TranslateHistoryPanel";

/** 造 count 条历史，id 递增（顺序无关，只用于断言提示与条数）。 */
function makeItems(count: number): TranslateHistoryItem[] {
  return Array.from({ length: count }, (_, i) => ({
    id: `h-${i}`,
    sourceText: `src-${i}`,
    translatedText: `dst-${i}`,
    sourceLang: "en",
    targetLang: "zh",
    providerId: "mock",
    createdUtc: 1_000_000 - i,
  }));
}

describe("translate-history-panel", () => {
  it("translate-history-panel: 条数达到单页上限时提示「仅显示最近 N 条」", () => {
    // Arrange：后端返回正好一页（上限条数）
    const items = makeItems(TRANSLATE_HISTORY_PAGE_LIMIT);

    // Act
    render(<TranslateHistoryPanel items={items} onSelectItem={vi.fn()} />);

    // Assert：提示文案带上限值，且条数徽标等于实际拿到的条数
    expect(screen.getByTestId("history-limit-hint")).toHaveTextContent(
      `仅显示最近 ${TRANSLATE_HISTORY_PAGE_LIMIT} 条`
    );
    expect(screen.getByText(String(TRANSLATE_HISTORY_PAGE_LIMIT))).toBeInTheDocument();
  });

  it("translate-history-panel: 条数未达上限时不出现分页提示（少一条也不提示）", () => {
    // Arrange：比上限少一条，说明历史确实被完整返回
    const items = makeItems(TRANSLATE_HISTORY_PAGE_LIMIT - 1);

    // Act
    render(<TranslateHistoryPanel items={items} onSelectItem={vi.fn()} />);

    // Assert
    expect(screen.queryByTestId("history-limit-hint")).not.toBeInTheDocument();
  });

  it("translate-history-panel: 空历史时既不出现分页提示也不渲染条目", () => {
    // Act
    render(<TranslateHistoryPanel items={[]} onSelectItem={vi.fn()} />);

    // Assert：空态与分页提示互斥
    expect(screen.getByText("暂无翻译历史")).toBeInTheDocument();
    expect(screen.queryByTestId("history-limit-hint")).not.toBeInTheDocument();
  });
});
