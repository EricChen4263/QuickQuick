import { ActionBar, ActionBarHint } from "../components/ActionBar";
import { selectHints, type ModifierState } from "./footer-hints";

interface PopoverFooterProps {
  /** 当前按下的修饰键；徽标集合随它切换（见 selectHints） */
  modifiers: ModifierState;
}

/**
 * Popover 底部快捷键提示条。
 * 复用公共 ActionBar 条壳（glass 皮），className 局部恢复 18px 呼吸感。
 * 提示集合由当前修饰键决定：默认层含数字直达，按住 ⌃ / ⌘ / ⌥ 时换成对应的组合键。
 */
export function PopoverFooter({ modifiers }: PopoverFooterProps) {
  return (
    <ActionBar variant="glass" as="footer" className="popover-footer">
      {selectHints(modifiers).map((hint) => (
        <ActionBarHint key={hint.kbd} kbd={hint.kbd} label={hint.label} />
      ))}
    </ActionBar>
  );
}
