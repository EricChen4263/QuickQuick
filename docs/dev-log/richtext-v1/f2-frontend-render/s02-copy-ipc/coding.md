---
id: RT1-F2-S02-code
type: coding_record
level: 小功能
parent: RT1-F2
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: 4e0caa3
acceptance_ids: [RT1-F2-A02]
evidence:
  - src/panels/clipboard/ClipPreview.tsx
  - src/clip-popover/ClipPopoverApp.tsx
  - src/ipc/ipc-client.ts
author: coder
---

# 编码记录 · RT1-F2-S02 复制按钮改调 IPC（带富文本）

## 做了什么
把剪贴板条目的「复制」从前端 `navigator.clipboard.writeText`（只纯文本）改为后端 IPC `copyClipToClipboard(id)`（带富文本格式）；并收窄 `ClipItem.kind` 类型（I-2）。

## 关键决策与理由
- **复制改走 IPC**：复用 S04 的 `copy_clip_to_clipboard`（arboard set().html 带纯文本兜底），与「粘贴到前台」同源，体验一致（决策2）。主窗口 `handleCopy` 与 popover Alt+Enter 两处改调，保留各自错误处理与图片 no-op 守卫。
- **kind 收窄为 `"text"|"richtext"|"image"`**（审查 I-2）：消除拼写错误静默通过隐患；连带给测试 fixture 加 `as const`。
- **翻译页 writeToClipboard 不动**：仅改剪贴板条目复制，译文复制路径保留。

## 改动文件
- `src/panels/clipboard/ClipPreview.tsx` — handleCopy 改调 copyClipToClipboard(item.id)
- `src/clip-popover/ClipPopoverApp.tsx` — Alt+Enter 改调 copyClipToClipboard(selectedItem.id)
- `src/ipc/ipc-client.ts` — ClipItem.kind 收窄字面量联合
- `src/panels/clipboard/ClipboardPage.tsx` — 订正过时注释（writeToClipboard→copyClipToClipboard，打回 I-1）
- 测试：clip-preview-actions.test.tsx、clip-popover-actions.test.tsx（vi.mocked 统一，打回 I-2）

## 自测结论（TDD 红-绿-重构）
- 先写 `copy_button_invokes_copy_clip_to_clipboard`、`plaintext_copy_not_regressed`（RED：仍走 writeToClipboard），改调后 GREEN。
- `pnpm test` 477 passed；`pnpm exec tsc --noEmit` 0 错。
- 打回修复（reviewer #1）：I-1 过时注释订正（hints TV1-RETRO-1）+ I-2 测试 mock 改 vi.mocked（禁 any），复审 APPROVE。
