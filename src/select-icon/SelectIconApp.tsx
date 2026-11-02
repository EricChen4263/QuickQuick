import { selectIconTranslate } from "../ipc/ipc-client";

interface SelectIconAppProps {
  /** 点击后的翻译请求；默认经 IPC 通知 Rust 侧以已取得的选中文字唤起浮窗。可注入以便测试。 */
  onTranslate?: () => Promise<void>;
}

/**
 * 选中图标：光标旁的一个小圆钮，只负责把点击转成翻译请求。
 *
 * 待译文本不经过前端：它在鼠标抬起时已由 Rust 侧取得并持有，前端不读取、不缓存选中文字，
 * 也就不存在前端状态与原生选区不一致的问题。
 */
function SelectIconApp({ onTranslate = selectIconTranslate }: SelectIconAppProps) {
  const handleClick = () => {
    onTranslate().catch((err: unknown) => {
      console.warn("[select-icon] 翻译请求失败:", err);
    });
  };

  return (
    <button
      type="button"
      className="select-icon"
      aria-label="翻译选中文字"
      title="翻译选中文字"
      onClick={handleClick}
    >
      译
    </button>
  );
}

export default SelectIconApp;
