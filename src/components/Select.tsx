import { forwardRef, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import "./Select.css";

/** 单个下拉选项。disabled 项视觉置灰且不可选。 */
export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface SelectProps {
  value: string;
  onChange: (value: string) => void;
  options: SelectOption[];
  /** 无障碍标签，赋给 trigger 的 aria-label */
  ariaLabel?: string;
  /** 追加到根容器的类名，供调用处做布局/外观微调（默认外观由 .qq-select 提供，无需额外类） */
  className?: string;
}

/** 菜单浮层相对视口的定位坐标（fixed 定位，绕开 overflow 容器裁剪） */
interface MenuRect {
  left: number;
  top: number;
  /** 菜单最小宽度 = trigger 宽；菜单实际宽由内容（max-content）撑开，不窄于此值 */
  minWidth: number;
  /** 菜单不超出视口右边的最大宽度，避免长选项溢出屏幕 */
  maxWidth: number;
  /** 下方空间不足时向上翻：以 trigger 顶部为基准、菜单底部对齐 */
  openUpward: boolean;
}

/** 估算菜单最大高度（px），用于判断下方空间是否足够；与 CSS max-height 保持一致 */
const MENU_MAX_HEIGHT = 260;

/** 菜单与 trigger 之间的垂直间隙（px） */
const MENU_GAP = 4;

/** 菜单与视口右边的安全留白（px），防长选项贴边/溢出 */
const VIEWPORT_MARGIN = 8;

/**
 * 自定义下拉组件：替代原生 <select>。
 *
 * 为什么不用原生 select：主窗口 titleBarStyle=Overlay 下，WKWebView 里原生
 * 弹出菜单坐标系错位、CSS 无法纠正。本组件用 JS 自定位 + portal 到 body 的
 * fixed 浮层，绕开原生弹窗与 overflow 容器（如 .tx-scroll）的裁剪。
 *
 * 行为：点击/Enter 展开，点选项或键盘 Enter 选中，点外部 / Esc / blur 关闭；
 * ↑↓ 移动高亮并跳过禁用项，禁用项不可选。
 */
export function Select({ value, onChange, options, ariaLabel, className }: SelectProps) {
  const s = useSelectState(value, options, onChange);
  const selectedIndex = options.findIndex((o) => o.value === value);
  const currentLabel = selectedIndex >= 0 ? options[selectedIndex].label : "";

  return (
    <div
      ref={s.rootRef}
      data-open={s.isOpen}
      className={`qq-select${className !== undefined ? ` ${className}` : ""}`}
    >
      <SelectTrigger
        ref={s.triggerRef}
        ariaLabel={ariaLabel}
        isOpen={s.isOpen}
        label={currentLabel}
        onToggle={s.toggle}
        onKeyDown={s.onKeyDown}
        onBlur={s.onBlur}
      />

      {s.isOpen && s.menuRect !== null && (
        <SelectMenu
          listboxId={s.listboxId}
          ariaLabel={ariaLabel}
          rect={s.menuRect}
          options={options}
          value={value}
          activeIndex={s.activeIndex}
          onCommit={s.commitOption}
          onHover={s.setActiveIndex}
        />
      )}
    </div>
  );
}

interface SelectState {
  isOpen: boolean;
  toggle: () => void;
  activeIndex: number;
  setActiveIndex: React.Dispatch<React.SetStateAction<number>>;
  menuRect: MenuRect | null;
  listboxId: string;
  rootRef: React.MutableRefObject<HTMLDivElement | null>;
  triggerRef: React.MutableRefObject<HTMLButtonElement | null>;
  commitOption: (index: number) => void;
  onKeyDown: (event: React.KeyboardEvent) => void;
  onBlur: (event: React.FocusEvent) => void;
}

/**
 * 收拢 Select 的全部状态、定位与关闭副作用、键盘/失焦交互，
 * 让组件主体只负责派生当前 label 与 JSX 组装（保持函数短小、单一职责）。
 */
function useSelectState(
  value: string,
  options: SelectOption[],
  onChange: (value: string) => void
): SelectState {
  const [isOpen, setIsOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const listboxId = useId();

  const selectedIndex = options.findIndex((o) => o.value === value);
  const menuRect = useMenuRect(isOpen, triggerRef);
  useCloseOnOutside(isOpen, rootRef, listboxId, () => setIsOpen(false));
  useCloseOnScroll(isOpen, listboxId, () => setIsOpen(false));

  // 打开时把高亮初始化到当前选中项（无选中则首个可用项）
  useEffect(() => {
    if (isOpen) {
      setActiveIndex(selectedIndex >= 0 ? selectedIndex : firstEnabledIndex(options, 0, 1));
    }
  }, [isOpen, selectedIndex, options]);

  const interactions = useSelectInteractions({
    isOpen,
    setIsOpen,
    activeIndex,
    setActiveIndex,
    options,
    onChange,
    triggerRef,
    listboxId,
  });

  return {
    isOpen,
    toggle: () => setIsOpen((prev) => !prev),
    activeIndex,
    setActiveIndex,
    menuRect,
    listboxId,
    rootRef,
    triggerRef,
    ...interactions,
  };
}

interface SelectTriggerProps {
  ariaLabel?: string;
  isOpen: boolean;
  label: string;
  onToggle: () => void;
  onKeyDown: (event: React.KeyboardEvent) => void;
  onBlur: (event: React.FocusEvent) => void;
}

/** 下拉触发按钮：展示当前值 + chevron，承载点击 / 键盘 / 失焦交互。 */
const SelectTrigger = forwardRef<HTMLButtonElement, SelectTriggerProps>(function SelectTrigger(
  { ariaLabel, isOpen, label, onToggle, onKeyDown, onBlur },
  ref
) {
  return (
    <button
      ref={ref}
      type="button"
      className="qq-select-trigger"
      aria-label={ariaLabel}
      aria-haspopup="listbox"
      aria-expanded={isOpen}
      onClick={onToggle}
      onKeyDown={onKeyDown}
      onBlur={onBlur}
    >
      <span className="qq-select-value">{label}</span>
      <SelectChevron />
    </button>
  );
});

interface SelectInteractionsParams {
  isOpen: boolean;
  setIsOpen: React.Dispatch<React.SetStateAction<boolean>>;
  activeIndex: number;
  setActiveIndex: React.Dispatch<React.SetStateAction<number>>;
  options: SelectOption[];
  onChange: (value: string) => void;
  triggerRef: React.MutableRefObject<HTMLButtonElement | null>;
  listboxId: string;
}

interface SelectInteractions {
  commitOption: (index: number) => void;
  onKeyDown: (event: React.KeyboardEvent) => void;
  onBlur: (event: React.FocusEvent) => void;
}

/**
 * 封装 Select 的提交 / 键盘 / 失焦交互逻辑，让主组件只负责状态编排与组装。
 * 键盘：Esc 关闭、Enter/Space 切换或选中、↑↓ 移动高亮并跳过禁用项。
 */
function useSelectInteractions(params: SelectInteractionsParams): SelectInteractions {
  const { isOpen, setIsOpen, activeIndex, setActiveIndex, options, onChange, triggerRef, listboxId } =
    params;

  function commitOption(index: number) {
    const option = options[index];
    if (option === undefined || option.disabled === true) return;
    onChange(option.value);
    setIsOpen(false);
    triggerRef.current?.focus();
  }

  function onKeyDown(event: React.KeyboardEvent) {
    if (event.key === "Escape") {
      setIsOpen(false);
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (isOpen) {
        commitOption(activeIndex);
      } else {
        setIsOpen(true);
      }
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!isOpen) {
        setIsOpen(true);
        return;
      }
      const step = event.key === "ArrowDown" ? 1 : -1;
      setActiveIndex((prev) => firstEnabledIndex(options, prev + step, step, prev));
    }
  }

  function onBlur(event: React.FocusEvent) {
    // 焦点移出整个组件（不落在菜单内）时关闭
    const next = event.relatedTarget as Node | null;
    const insideMenu = next !== null && (document.getElementById(listboxId)?.contains(next) ?? false);
    if (!insideMenu) setIsOpen(false);
  }

  return { commitOption, onKeyDown, onBlur };
}

/** 下拉箭头图标（与原生 select 的 chevron 视觉一致）。 */
function SelectChevron() {
  return (
    <svg
      className="qq-select-chevron"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="m6 9 6 6 6-6" />
    </svg>
  );
}

interface SelectMenuProps {
  listboxId: string;
  ariaLabel?: string;
  rect: MenuRect;
  options: SelectOption[];
  value: string;
  activeIndex: number;
  onCommit: (index: number) => void;
  onHover: (index: number) => void;
}

/** 浮层菜单：portal 到 body 的 fixed listbox，绕开 overflow 容器裁剪。 */
function SelectMenu({
  listboxId,
  ariaLabel,
  rect,
  options,
  value,
  activeIndex,
  onCommit,
  onHover,
}: SelectMenuProps) {
  return createPortal(
    <ul
      id={listboxId}
      role="listbox"
      aria-label={ariaLabel}
      className="qq-select-menu"
      style={{
        position: "fixed",
        left: rect.left,
        // 不窄于 trigger（minWidth），但随内容撑开（max-content）；maxWidth 兜底不溢出视口
        minWidth: rect.minWidth,
        maxWidth: rect.maxWidth,
        width: "max-content",
        ...(rect.openUpward ? { bottom: window.innerHeight - rect.top } : { top: rect.top }),
      }}
    >
      {options.map((option, index) => (
        <SelectOptionItem
          key={option.value}
          option={option}
          isSelected={option.value === value}
          isActive={index === activeIndex}
          onCommit={() => onCommit(index)}
          onHover={() => onHover(index)}
        />
      ))}
    </ul>,
    document.body
  );
}

interface SelectOptionItemProps {
  option: SelectOption;
  isSelected: boolean;
  isActive: boolean;
  onCommit: () => void;
  onHover: () => void;
}

/** 单个选项行：标记 selected/disabled，禁用项不响应 hover、点击交由父级守门。 */
function SelectOptionItem({
  option,
  isSelected,
  isActive,
  onCommit,
  onHover,
}: SelectOptionItemProps) {
  const isDisabled = option.disabled === true;
  return (
    <li
      role="option"
      aria-selected={isSelected}
      aria-disabled={isDisabled}
      className={
        "qq-select-option" + (isActive ? " is-active" : "") + (isDisabled ? " is-disabled" : "")
      }
      // mousedown 而非 click：避免 trigger 先 blur 关闭菜单导致 click 落空
      onMouseDown={(event) => {
        event.preventDefault();
        onCommit();
      }}
      onMouseEnter={() => {
        if (!isDisabled) onHover();
      }}
    >
      {option.label}
    </li>
  );
}

/**
 * 展开时按 trigger 的视口坐标算浮层位置；下方空间不足则向上翻。
 * 收起时返回 null，调用方据此不渲染菜单。
 */
function useMenuRect(
  isOpen: boolean,
  triggerRef: React.MutableRefObject<HTMLButtonElement | null>
): MenuRect | null {
  const [menuRect, setMenuRect] = useState<MenuRect | null>(null);

  useLayoutEffect(() => {
    if (!isOpen || triggerRef.current === null) {
      setMenuRect(null);
      return;
    }
    const rect = triggerRef.current.getBoundingClientRect();
    const spaceBelow = window.innerHeight - rect.bottom;
    const openUpward = spaceBelow < MENU_MAX_HEIGHT && rect.top > spaceBelow;
    setMenuRect({
      left: rect.left,
      top: openUpward ? rect.top - MENU_GAP : rect.bottom + MENU_GAP,
      // 菜单不窄于 trigger，但靠 max-content 撑到最长选项；右侧留白防溢出视口
      minWidth: rect.width,
      maxWidth: window.innerWidth - rect.left - VIEWPORT_MARGIN,
      openUpward,
    });
  }, [isOpen, triggerRef]);

  return menuRect;
}

/**
 * 点击组件外部（既不在 root 也不在浮层菜单内）时关闭。
 * 监听 mousedown 以便先于浮层内点击的 click 判定。
 */
function useCloseOnOutside(
  isOpen: boolean,
  rootRef: React.MutableRefObject<HTMLDivElement | null>,
  listboxId: string,
  onClose: () => void
) {
  useEffect(() => {
    if (!isOpen) return;
    function handlePointerDown(event: MouseEvent) {
      const target = event.target as Node;
      const insideRoot = rootRef.current?.contains(target) ?? false;
      const insideMenu = document.getElementById(listboxId)?.contains(target) ?? false;
      if (!insideRoot && !insideMenu) onClose();
    }
    document.addEventListener("mousedown", handlePointerDown);
    return () => document.removeEventListener("mousedown", handlePointerDown);
  }, [isOpen, rootRef, listboxId, onClose]);
}

/**
 * 窗口 resize / 外部滚动时关闭，避免浮层与 trigger 错位。
 * 关键：scroll 用 capture 监听会被菜单自身内部滚动触发——必须排除菜单内部滚动，
 * 否则选项超过 max-height 时滚动菜单即自关闭（源/目标语长列表受影响）。
 */
function useCloseOnScroll(isOpen: boolean, listboxId: string, onClose: () => void) {
  useEffect(() => {
    if (!isOpen) return;
    function closeOnExternalScroll(event: Event) {
      const menu = document.getElementById(listboxId);
      if (menu?.contains(event.target as Node)) return;
      onClose();
    }
    window.addEventListener("resize", onClose);
    window.addEventListener("scroll", closeOnExternalScroll, true);
    return () => {
      window.removeEventListener("resize", onClose);
      window.removeEventListener("scroll", closeOnExternalScroll, true);
    };
  }, [isOpen, listboxId, onClose]);
}

/**
 * 从 start 起按 step 方向找第一个非禁用项的索引；
 * 越界或全程无可用项时回退到 fallback（默认 0）。用于键盘导航跳过禁用项。
 */
function firstEnabledIndex(
  options: SelectOption[],
  start: number,
  step: number,
  fallback = 0
): number {
  for (let i = start; i >= 0 && i < options.length; i += step) {
    if (options[i].disabled !== true) return i;
  }
  return fallback;
}
