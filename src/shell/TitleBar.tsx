/**
 * TitleBar：macOS Overlay 模式自绘标题栏。
 *
 * Overlay 模式下系统红绿灯浮在 webview 左上角，webview 内容延伸到顶部，
 * 因此需要在标题栏左侧留出约 76px 的空间避免标题文字压住红绿灯。
 * 整条加 data-tauri-drag-region 使整个标题栏区域均可拖动移窗。
 *
 * Windows / Linux 注意：titleBarStyle: Overlay 为 macOS 专属；
 * 其他平台窗口控制按钮（最小化/关闭）的自绘实现留待后续版本处理。
 * 当前 TitleBar 在非 mac 上仅显示品牌标题，不崩溃。
 */
export function TitleBar() {
  return (
    <div className="qq-titlebar" data-tauri-drag-region>
      <span className="brand-accent">Quick</span>Quick
    </div>
  );
}
