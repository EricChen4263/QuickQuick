import { ActionBar, ActionBarHint } from "../components/ActionBar";

/**
 * Popover 底部快捷键提示条（静态展示）。
 * 复用公共 ActionBar 条壳（glass 皮），className 局部恢复 18px 呼吸感。
 */
export function PopoverFooter() {
  return (
    <ActionBar variant="glass" as="footer" className="popover-footer">
      <ActionBarHint kbd="↵" label="粘贴" />
      <ActionBarHint kbd="⌥↵" label="复制" />
      <ActionBarHint kbd="↑↓" label="选择" />
      <ActionBarHint kbd="esc" label="关闭" />
    </ActionBar>
  );
}
