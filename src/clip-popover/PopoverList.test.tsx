import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { PopoverList } from "./PopoverList";
import type { ClipGroups } from "./grouping";
import type { ClipItem } from "../ipc/ipc-client";

vi.mock("../components/KindIcon", () => ({
  KindIcon: ({ kind }: { kind: string }) => <span data-testid="kind-icon">{kind}</span>,
}));

function makeItem(id: string, overrides: Partial<ClipItem> = {}): ClipItem {
  return {
    id,
    content: `content-${id}`,
    kind: "text",
    isFavorite: false,
    lastModifiedUtc: 1000,
    ...overrides,
  };
}

const noop = () => undefined;

describe("PopoverList 分组渲染", () => {
  it("三组均有条目时渲染收藏/今天/更早三个标题", () => {
    const groups: ClipGroups = {
      favorites: [makeItem("fav1", { isFavorite: true })],
      today: [makeItem("t1")],
      earlier: [makeItem("old1")],
    };

    render(<PopoverList groups={groups} selectedId={null} onSelect={noop} />);

    expect(screen.getByText("收藏")).toBeDefined();
    expect(screen.getByText("今天")).toBeDefined();
    expect(screen.getByText("更早")).toBeDefined();
  });

  it("只有收藏组时不渲染今天/更早标题", () => {
    const groups: ClipGroups = {
      favorites: [makeItem("fav1", { isFavorite: true })],
      today: [],
      earlier: [],
    };

    render(<PopoverList groups={groups} selectedId={null} onSelect={noop} />);

    expect(screen.getByText("收藏")).toBeDefined();
    expect(screen.queryByText("今天")).toBeNull();
    expect(screen.queryByText("更早")).toBeNull();
  });

  it("只有今天组时不渲染收藏/更早标题", () => {
    const groups: ClipGroups = {
      favorites: [],
      today: [makeItem("t1")],
      earlier: [],
    };

    render(<PopoverList groups={groups} selectedId={null} onSelect={noop} />);

    expect(screen.queryByText("收藏")).toBeNull();
    expect(screen.getByText("今天")).toBeDefined();
    expect(screen.queryByText("更早")).toBeNull();
  });

  it("全空组时渲染占位文案「剪贴板暂无内容」，不渲染任何分组标题", () => {
    const groups: ClipGroups = { favorites: [], today: [], earlier: [] };

    render(<PopoverList groups={groups} selectedId={null} onSelect={noop} />);

    expect(screen.getByText("剪贴板暂无内容")).toBeDefined();
    expect(screen.queryByText("收藏")).toBeNull();
    expect(screen.queryByText("今天")).toBeNull();
    expect(screen.queryByText("更早")).toBeNull();
  });

  it("selectedId 匹配条目时该条目 aria-selected=true，其余为 false", () => {
    const groups: ClipGroups = {
      favorites: [],
      today: [makeItem("t1"), makeItem("t2")],
      earlier: [],
    };

    render(<PopoverList groups={groups} selectedId="t1" onSelect={noop} />);

    const rows = screen.getAllByRole("option");
    const row1 = rows.find((r) => r.textContent?.includes("content-t1"));
    const row2 = rows.find((r) => r.textContent?.includes("content-t2"));

    expect(row1).toHaveAttribute("aria-selected", "true");
    expect(row2).toHaveAttribute("aria-selected", "false");
  });
});
