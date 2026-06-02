---
id: f3-popover-s04-batch-b2
title: 里程碑4 popover · Batch B2：clip-popover 键盘流
status: 实现完成
commit: PENDING
date: 2026-06-02
---

## 改动文件

- `src/clip-popover/keyboard-nav.ts`（新建）：纯函数 `advanceSelection`，复用 `moveHighlight` 做 ↑↓ id 推进，currentId 不在列表或 null 时选第一项。
- `src/clip-popover/ClipPopoverApp.tsx`（修改）：添加 `handleKeyDown`，绑到搜索框 `onKeyDown`；新增 import `pasteToFront`、`writeToClipboard`、`getCurrentWindow`、`advanceSelection`。
- `src/clip-popover/keyboard-nav.test.ts`（新建）：12 条纯函数单测，覆盖中段 ↑↓、首尾 clamp、null currentId、不在列表、空列表。
- `src/clip-popover/clip-popover-actions.test.tsx`（新建）：5 条集成测试，mock ipc-client / browser-api / tauri window，覆盖 Enter 粘贴、Alt+Enter 复制、↓ 移动、↓+Enter 粘贴第二项、失败不 hide。
- `src/clip-popover/PopoverFooter.tsx`（未改）：footer 文案 ↵/⌥↵/↑↓/esc 与实际键位一致，无需修改。

## TDD 红绿

RED：`keyboard-nav.test.ts` 因 `./keyboard-nav` 模块不存在而 FAIL；`clip-popover-actions.test.tsx` 因 App 无键盘逻辑，pasteToFront/hide 未被调用而 FAIL（5 条）。GREEN：实现后两组测试全绿（12 + 5 = 17 条新增，全套 300 条通过）。

## 动作失败策略

Enter/Alt+Enter 动作 reject 时：catch 后 `console.error` 记录错误，**不调 hide**，窗口保持打开让用户察觉失败；成功才 hide。

## 图片项 Alt+Enter 决策

图片项 `content` 为空字符串（`kind="image"` 条目不存文本内容），Alt+Enter 会复制空字符串到剪贴板，行为无害但无意义。本批按现有 `content` 复制，不引原图数据（避免大 base64 写剪贴板的复杂性）；图片项的完整复制能力留待后续 B3/专项处理。

## 验证结果

```
pnpm test:   PASS (300) FAIL (0)
pnpm tsc:    TypeScript: No errors found  EXIT:0
pnpm build:  ✓ built in 362ms  EXIT:0
```
