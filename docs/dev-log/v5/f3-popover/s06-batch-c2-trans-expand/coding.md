---
id: f3-popover-s06-batch-c2
title: 里程碑4 popover · Batch C2：trans-popover 展开与获焦重读
status: 实现完成
commit: PENDING
date: 2026-06-02
---

## 改动文件

- `src/trans-popover/retranslate.ts`（新增）：`shouldRetranslate` 纯函数，去重判断
- `src/trans-popover/retranslate.test.ts`（新增）：TDD 4 条单测（红→绿）
- `src/trans-popover/TransPopoverApp.tsx`（修改）：实现 handleExpand + 获焦重读 focus 监听
- `src/trans-popover/trans-popover.test.tsx`（修改）：补展开断言，加 tauri mock，共 6 条测试
- `src/trans-popover/popover.css`（修改）：布局间距与按钮 hover 打磨

## 关键设计

- 展开 v1 限制：仅 emit("route","translate") + show/focus main + hide 自身，不预填文本（主窗翻译输入预填为后续增强）。
- 获焦重读去重：`shouldRetranslate(newText, lastText)` 纯函数，新文本为 null 或与上次相同则跳过，避免首次挂载+首次 focus 双译同一文本。
- TDD：retranslate.ts 先写测试确认 RED（模块不存在报错），写实现后 GREEN（4 tests pass）。

## 验证结果

- `pnpm test`：316 passed (316)，EXIT 0
- `pnpm exec tsc --noEmit`：PENDING
- `pnpm build`：PENDING
