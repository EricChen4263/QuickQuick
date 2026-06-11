import type { ReactElement, ReactNode } from "react";
import "./ActionBar.css";

/** 条壳皮肤：glass 融入毛玻璃弹窗，surface 用于主窗半透明面板 */
export type ActionBarVariant = "glass" | "surface";

/** 主轴对齐，映射 justify-content */
export type ActionBarAlign = "start" | "end" | "between";

export interface ActionBarProps {
  variant: ActionBarVariant;
  align?: ActionBarAlign;
  /**
   * 根标签：仅弹窗唯一页脚用 "footer"（语义页脚），内容流里的按钮行用默认 "div"。
   * 默认 div，避免在非页脚语境产生多余 contentinfo landmark 干扰屏幕阅读器导航。
   */
  as?: "div" | "footer";
  /** 追加到根元素的类名，供调用处做局部布局微调（如恢复原 gap），不覆盖内置类 */
  className?: string;
  children: ReactNode;
}

/**
 * 公共底部互动条「条壳」：纯容器，只负责布局（flex/对齐/间距/padding/顶分割线/背景变体）。
 * 内部按钮各页继续用公共类 .btn/.btn-primary/.icon-btn，条壳不约束内容样式。
 */
export function ActionBar({
  variant,
  align = "start",
  as = "div",
  className,
  children,
}: ActionBarProps): ReactElement {
  const classes = [
    "qq-action-bar",
    `qq-action-bar--${variant}`,
    `qq-action-bar--align-${align}`,
  ];
  if (className !== undefined) {
    classes.push(className);
  }
  // 首字母大写局部变量，JSX 据此渲染对应原生标签
  const RootTag = as;
  return <RootTag className={classes.join(" ")}>{children}</RootTag>;
}

export interface ActionBarHintProps {
  kbd: string;
  label: string;
}

/** 快捷键提示：kbd 键帽 + 说明文本，供弹窗底部条复用 */
export function ActionBarHint({ kbd, label }: ActionBarHintProps): ReactElement {
  return (
    <span className="qq-action-bar-hint">
      <kbd className="qq-kbd">{kbd}</kbd> {label}
    </span>
  );
}
