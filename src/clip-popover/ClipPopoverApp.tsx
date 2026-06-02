import React from "react";

/**
 * 剪贴板 popover 占位 UI（Batch A2）
 *
 * 仅用于验证：窗口能弹出、透明毛玻璃生效、Esc 能关。
 * 真实列表/搜索/键盘流在 Batch B 实现。
 */
export default function ClipPopoverApp(): React.ReactElement {
  return (
    <div
      style={{
        padding: "24px",
        fontFamily: "var(--font, system-ui, sans-serif)",
        color: "var(--fg)",
      }}
    >
      <h2 style={{ margin: "0 0 8px", fontSize: "16px", fontWeight: 600 }}>
        剪贴板（占位）
      </h2>
      <p style={{ margin: 0, fontSize: "13px", color: "var(--muted)" }}>
        Batch B 将在此实现剪贴板历史列表与搜索。按 Esc 关闭窗口。
      </p>
    </div>
  );
}
