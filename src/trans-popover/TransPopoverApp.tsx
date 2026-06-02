import React from "react";

/**
 * 翻译 popover 占位 UI（Batch A2）
 *
 * 仅用于验证：窗口能弹出、透明毛玻璃生效、Esc 能关。
 * 真实翻译输入/结果展示在 Batch C 实现。
 */
export default function TransPopoverApp(): React.ReactElement {
  return (
    <div
      style={{
        padding: "16px",
        fontFamily: "var(--font, system-ui, sans-serif)",
        color: "var(--fg)",
      }}
    >
      <h2 style={{ margin: "0 0 8px", fontSize: "14px", fontWeight: 600 }}>
        翻译（占位）
      </h2>
      <p style={{ margin: 0, fontSize: "12px", color: "var(--muted)" }}>
        Batch C 将在此实现翻译输入与结果展示。按 Esc 关闭窗口。
      </p>
    </div>
  );
}
