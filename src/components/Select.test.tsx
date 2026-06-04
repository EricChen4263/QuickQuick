import { describe, it, expect, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Select } from "./Select";
import type { SelectOption } from "./Select";

const OPTIONS: SelectOption[] = [
  { value: "all", label: "全部" },
  { value: "text", label: "纯文本" },
  { value: "image", label: "图片" },
];

const OPTIONS_WITH_DISABLED: SelectOption[] = [
  { value: "mymemory", label: "MyMemory" },
  { value: "baidu", label: "百度翻译", disabled: true },
  { value: "deepl", label: "DeepL" },
];

describe("Select", () => {
  it("初始收起：trigger 显示当前 value 对应 label，菜单不渲染", () => {
    render(<Select value="text" onChange={vi.fn()} options={OPTIONS} ariaLabel="类型筛选" />);

    const trigger = screen.getByRole("button", { name: "类型筛选" });
    expect(trigger).toHaveTextContent("纯文本");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("点击 trigger 展开菜单，再点击收起", async () => {
    const user = userEvent.setup();
    render(<Select value="all" onChange={vi.fn()} options={OPTIONS} ariaLabel="类型筛选" />);

    const trigger = screen.getByRole("button", { name: "类型筛选" });
    await user.click(trigger);
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    expect(trigger).toHaveAttribute("aria-expanded", "true");

    await user.click(trigger);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("点选项触发 onChange 带该 value 并关闭菜单", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<Select value="all" onChange={onChange} options={OPTIONS} ariaLabel="类型筛选" />);

    await user.click(screen.getByRole("button", { name: "类型筛选" }));
    await user.click(screen.getByRole("option", { name: "图片" }));

    expect(onChange).toHaveBeenCalledWith("image");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("点击组件外部关闭菜单", async () => {
    const user = userEvent.setup();
    render(
      <div>
        <Select value="all" onChange={vi.fn()} options={OPTIONS} ariaLabel="类型筛选" />
        <button type="button">外部</button>
      </div>
    );

    await user.click(screen.getByRole("button", { name: "类型筛选" }));
    expect(screen.getByRole("listbox")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "外部" }));
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("按 Esc 关闭菜单", async () => {
    const user = userEvent.setup();
    render(<Select value="all" onChange={vi.fn()} options={OPTIONS} ariaLabel="类型筛选" />);

    await user.click(screen.getByRole("button", { name: "类型筛选" }));
    expect(screen.getByRole("listbox")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("键盘 ArrowDown 移动高亮，Enter 选中当前高亮项", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<Select value="all" onChange={onChange} options={OPTIONS} ariaLabel="类型筛选" />);

    const trigger = screen.getByRole("button", { name: "类型筛选" });
    trigger.focus();
    // 展开后高亮在当前选中项 all(索引0)，ArrowDown 到 text(索引1)
    await user.keyboard("{Enter}");
    await user.keyboard("{ArrowDown}{Enter}");

    expect(onChange).toHaveBeenCalledWith("text");
  });

  it("键盘 ArrowUp 向上移动高亮", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<Select value="image" onChange={onChange} options={OPTIONS} ariaLabel="类型筛选" />);

    const trigger = screen.getByRole("button", { name: "类型筛选" });
    trigger.focus();
    // 当前选中 image(索引2)，展开后 ArrowUp 到 text(索引1)
    await user.keyboard("{Enter}");
    await user.keyboard("{ArrowUp}{Enter}");

    expect(onChange).toHaveBeenCalledWith("text");
  });

  it("禁用项视觉置灰且点击不触发 onChange", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<Select value="mymemory" onChange={onChange} options={OPTIONS_WITH_DISABLED} ariaLabel="翻译源" />);

    await user.click(screen.getByRole("button", { name: "翻译源" }));
    const disabledOption = screen.getByRole("option", { name: "百度翻译" });
    expect(disabledOption).toHaveAttribute("aria-disabled", "true");

    await user.click(disabledOption);
    expect(onChange).not.toHaveBeenCalled();
    // 点禁用项不关闭菜单
    expect(screen.getByRole("listbox")).toBeInTheDocument();
  });

  it("键盘导航跳过禁用项", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<Select value="mymemory" onChange={onChange} options={OPTIONS_WITH_DISABLED} ariaLabel="翻译源" />);

    const trigger = screen.getByRole("button", { name: "翻译源" });
    trigger.focus();
    // 当前 mymemory(0)，ArrowDown 应跳过 baidu(1,禁用) 落到 deepl(2)
    await user.keyboard("{Enter}");
    await user.keyboard("{ArrowDown}{Enter}");

    expect(onChange).toHaveBeenCalledWith("deepl");
  });

  it("当前选中项标记 aria-selected", async () => {
    const user = userEvent.setup();
    render(<Select value="text" onChange={vi.fn()} options={OPTIONS} ariaLabel="类型筛选" />);

    await user.click(screen.getByRole("button", { name: "类型筛选" }));
    expect(screen.getByRole("option", { name: "纯文本" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("option", { name: "图片" })).toHaveAttribute("aria-selected", "false");
  });

  it("展开时 root 容器标记 data-open=true，收起时为 false", async () => {
    const user = userEvent.setup();
    const { container } = render(
      <Select value="all" onChange={vi.fn()} options={OPTIONS} ariaLabel="类型筛选" />
    );

    const root = container.querySelector(".qq-select");
    expect(root).toHaveAttribute("data-open", "false");

    await user.click(screen.getByRole("button", { name: "类型筛选" }));
    expect(root).toHaveAttribute("data-open", "true");
  });

  it("菜单内部滚动不关闭菜单（scroll target 在 listbox 内）", async () => {
    const user = userEvent.setup();
    render(<Select value="all" onChange={vi.fn()} options={OPTIONS} ariaLabel="类型筛选" />);

    await user.click(screen.getByRole("button", { name: "类型筛选" }));
    const listbox = screen.getByRole("listbox");

    // 模拟菜单内部滚动：事件 target 在 listbox 内
    act(() => {
      listbox.dispatchEvent(new Event("scroll", { bubbles: true }));
    });

    expect(screen.queryByRole("listbox")).toBeInTheDocument();
  });

  it("外部滚动关闭菜单（scroll target 在 listbox 外）", async () => {
    const user = userEvent.setup();
    render(<Select value="all" onChange={vi.fn()} options={OPTIONS} ariaLabel="类型筛选" />);

    await user.click(screen.getByRole("button", { name: "类型筛选" }));
    expect(screen.getByRole("listbox")).toBeInTheDocument();

    // 模拟外部滚动：事件 target 为 document（不在 listbox 内）
    act(() => {
      document.dispatchEvent(new Event("scroll", { bubbles: true }));
    });

    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("options 为空时点 trigger 展开空 listbox 且不崩", async () => {
    const user = userEvent.setup();
    render(<Select value="" onChange={vi.fn()} options={[]} ariaLabel="空选择" />);

    await user.click(screen.getByRole("button", { name: "空选择" }));
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    expect(screen.queryAllByRole("option")).toHaveLength(0);
  });

  it("value 不在 options 内时 trigger 显示空、不崩", () => {
    render(<Select value="ghost" onChange={vi.fn()} options={OPTIONS} ariaLabel="类型筛选" />);

    expect(screen.getByRole("button", { name: "类型筛选" })).toHaveTextContent("");
  });

  it("全部禁用时键盘 Enter 不触发 onChange（无可选项）", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    const allDisabled: SelectOption[] = [
      { value: "a", label: "A", disabled: true },
      { value: "b", label: "B", disabled: true },
    ];
    render(<Select value="a" onChange={onChange} options={allDisabled} ariaLabel="全禁用" />);

    const trigger = screen.getByRole("button", { name: "全禁用" });
    trigger.focus();
    await user.keyboard("{Enter}");
    await user.keyboard("{Enter}");

    expect(onChange).not.toHaveBeenCalled();
  });
});
