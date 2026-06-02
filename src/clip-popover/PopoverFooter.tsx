/**
 * Popover 底部快捷键提示条（静态展示）。
 * B2 会接入真实键盘事件，此处只提供视觉占位。
 */
export function PopoverFooter() {
  return (
    <footer className="popover-footer">
      <span className="popover-footer-hint">
        <kbd>↵</kbd> 粘贴
      </span>
      <span className="popover-footer-hint">
        <kbd>⌥↵</kbd> 复制
      </span>
      <span className="popover-footer-hint">
        <kbd>↑↓</kbd> 选择
      </span>
      <span className="popover-footer-hint">
        <kbd>esc</kbd> 关闭
      </span>
    </footer>
  );
}
