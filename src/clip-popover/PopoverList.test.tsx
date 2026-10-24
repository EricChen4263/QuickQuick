import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { PopoverList } from "./PopoverList";
import { createInputOwnership } from "./keyboard-nav";
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
    hasThumbnail: false,
    lastModifiedUtc: 1000,
    ...overrides,
  };
}

const noop = () => undefined;

// jsdom 未实现 scrollIntoView，组件 effect 会调用它；全局桩避免渲染期抛错
beforeEach(() => {
  Element.prototype.scrollIntoView = vi.fn();
});

describe("PopoverList 分组渲染", () => {
  it("三组均有条目时渲染收藏/今天/更早三个标题", () => {
    const groups: ClipGroups = {
      favorites: [makeItem("fav1", { isFavorite: true })],
      today: [makeItem("t1")],
      earlier: [makeItem("old1")],
    };

    render(<PopoverList groups={groups} selectedId={null} onSelect={noop} ownership={createInputOwnership()} />);

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

    render(<PopoverList groups={groups} selectedId={null} onSelect={noop} ownership={createInputOwnership()} />);

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

    render(<PopoverList groups={groups} selectedId={null} onSelect={noop} ownership={createInputOwnership()} />);

    expect(screen.queryByText("收藏")).toBeNull();
    expect(screen.getByText("今天")).toBeDefined();
    expect(screen.queryByText("更早")).toBeNull();
  });

  it("全空组时渲染占位文案「剪贴板暂无内容」，不渲染任何分组标题", () => {
    const groups: ClipGroups = { favorites: [], today: [], earlier: [] };

    render(<PopoverList groups={groups} selectedId={null} onSelect={noop} ownership={createInputOwnership()} />);

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

    render(<PopoverList groups={groups} selectedId="t1" onSelect={noop} ownership={createInputOwnership()} />);

    const rows = screen.getAllByRole("option");
    const row1 = rows.find((r) => r.textContent?.includes("content-t1"));
    const row2 = rows.find((r) => r.textContent?.includes("content-t2"));

    expect(row1).toHaveAttribute("aria-selected", "true");
    expect(row2).toHaveAttribute("aria-selected", "false");
  });
});

describe("PopoverList 选中行滚动跟随", () => {
  it("初次渲染时把选中行滚入可视区（block: nearest）", () => {
    const groups: ClipGroups = {
      favorites: [],
      today: [makeItem("t1"), makeItem("t2")],
      earlier: [],
    };

    render(<PopoverList groups={groups} selectedId="t2" onSelect={noop} ownership={createInputOwnership()} />);

    const selectedRow = screen
      .getAllByRole("option")
      .find((r) => r.getAttribute("aria-selected") === "true");
    expect(selectedRow?.scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });
  });

  it("selectedId 变化时滚动到新的选中行", () => {
    const groups: ClipGroups = {
      favorites: [],
      today: [makeItem("t1"), makeItem("t2"), makeItem("t3")],
      earlier: [],
    };

    const { rerender } = render(
      <PopoverList groups={groups} selectedId="t1" onSelect={noop} ownership={createInputOwnership()} />,
    );
    vi.clearAllMocks();

    rerender(<PopoverList groups={groups} selectedId="t3" onSelect={noop} ownership={createInputOwnership()} />);

    const newSelectedRow = screen
      .getAllByRole("option")
      .find((r) => r.getAttribute("aria-selected") === "true");
    expect(newSelectedRow?.textContent).toContain("content-t3");
    expect(newSelectedRow?.scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });
  });

  it("无选中项（selectedId=null）时不触发滚动", () => {
    const groups: ClipGroups = {
      favorites: [],
      today: [makeItem("t1")],
      earlier: [],
    };

    render(<PopoverList groups={groups} selectedId={null} onSelect={noop} ownership={createInputOwnership()} />);

    expect(Element.prototype.scrollIntoView).not.toHaveBeenCalled();
  });
});

describe("PopoverList 悬停与键盘仲裁", () => {
  const groups: ClipGroups = {
    favorites: [],
    today: [makeItem("t1"), makeItem("t2"), makeItem("t3")],
    earlier: [],
  };

  /** 按内容摘要取行（行内文本可能被 <mark> 拆开，因此按容器文本匹配） */
  function rowOf(content: string): HTMLElement {
    const row = screen
      .getAllByRole("option")
      .find((option) => option.textContent?.includes(content));
    if (row === undefined) {
      throw new Error(`列表中不存在内容为 ${content} 的行`);
    }
    return row;
  }

  it("键盘主导时悬停不改选中项（静止指针被滚到别的行下）", () => {
    // Arrange：仲裁器初始由键盘主导，等价于面板刚打开或刚按过方向键
    const onSelect = vi.fn((item: ClipItem) => item.id);
    const ownership = createInputOwnership();
    render(
      <PopoverList groups={groups} selectedId="t1" onSelect={onSelect} ownership={ownership} />,
    );

    // Act：列表滚动把第 2 行送到静止指针下，浏览器补发 mouseenter
    fireEvent.mouseEnter(rowOf("content-t2"));

    // Assert
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("pointer 真实位移后，行上的移动事件把选中交给鼠标", () => {
    const onSelect = vi.fn((item: ClipItem) => item.id);
    const ownership = createInputOwnership();
    render(
      <PopoverList groups={groups} selectedId="t1" onSelect={onSelect} ownership={ownership} />,
    );

    // Act：坐标变化 = 真实位移
    fireEvent.mouseMove(rowOf("content-t3"), { clientX: 12, clientY: 34 });

    // Assert
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0][0].id).toBe("t3");
  });

  it("键盘收回主导权后，同坐标移动与进入都不再改选中项", () => {
    const onSelect = vi.fn((item: ClipItem) => item.id);
    const ownership = createInputOwnership();
    render(
      <PopoverList groups={groups} selectedId="t1" onSelect={onSelect} ownership={ownership} />,
    );
    fireEvent.mouseMove(rowOf("content-t3"), { clientX: 12, clientY: 34 });
    expect(onSelect).toHaveBeenCalledTimes(1);
    onSelect.mockClear();

    // Act：键盘导航收回主导权，随后鼠标没动（同坐标的移动由滚动补发）
    ownership.handToKeyboard();
    fireEvent.mouseMove(rowOf("content-t2"), { clientX: 12, clientY: 34 });
    fireEvent.mouseEnter(rowOf("content-t2"));

    // Assert
    expect(onSelect).not.toHaveBeenCalled();
  });
});
