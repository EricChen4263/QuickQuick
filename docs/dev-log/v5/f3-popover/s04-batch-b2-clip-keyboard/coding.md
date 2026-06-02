---
id: f3-popover-s04-batch-b2
title: 里程碑4 popover · Batch B2：clip-popover 键盘流
status: 实现完成
commit: 3174754
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

## Batch B review 收口

**M-1 图片 Alt+Enter no-op**：`ClipPopoverApp.tsx` Alt+Enter 分支在 `writeToClipboard` 调用前加守卫 `if (selectedItem.kind === "image") return`，防止把空字符串写入系统剪贴板破坏用户内容。`clip-popover-actions.test.tsx` 新增测试用例「图片条目按 Alt+Enter：writeToClipboard 和 hide 均不被调用」锁定该守卫（TDD 红-绿验证）。

**M-2 aria-label**：`PopoverList.tsx` 两处 `div[role="listbox"]`（空列表与有内容两个分支）均补 `aria-label="剪贴板历史"`，满足 a11y listbox 可访问名称要求。

**已知 v1 限制（不在本批处理）**：Enter 对图片条目走 `pasteToFront(id)`，后端图片粘贴不支持时会 reject，现有 `.catch(console.error)` 不 hide 属可接受的 v1 降级，完整图片粘贴能力留后续专项。

**收口验证**：`pnpm test` 301 passed / `pnpm exec tsc --noEmit` exit 0 / `pnpm build` exit 0。
