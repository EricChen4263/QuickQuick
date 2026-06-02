---
id: f3-popover-s06-batch-c2
title: 里程碑4 popover · Batch C2：trans-popover 展开与获焦重读
status: 实现完成
commit: 53e064b
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

## Batch C review 收口

- **M-1**：`lastTextRef.current = text` 移至翻译成功后（`setStatus("done")` 同处），失败路径不更新 ref，使同文本再获焦可重试；补回归测试：模拟首次 reject 进错误态，focus 触发同文本 → translateText 被再次调用（从 1 次增到 2 次）。
- **M-2**：加 `translatingRef`（`useRef(false)`），函数入口若已在翻译中则直接 return，done/error 两条结束路径均由 `finally` 块置回 false，防 focus 与挂载并发竞争 setState。
- **L-1**：text 为 null 时显式 `if (text === null) { setStatus("empty"); return; }`，收窄后 `text` 自然为 string，去掉 `translateText(text!)` 非空断言。
- **L-3**：`pickLatestText` 在取 content 前显式判断 `kind !== "text" && kind !== "richtext"` 时返回 null，image 项无论 content 是否为空均不可译；补两条单测：图片 content 非空返回 null、富文本 content 返回正文。

## 最终验证结果（Batch C 收口）

- `pnpm test`：319 passed (319)，37 文件，EXIT 0
- `pnpm exec tsc --noEmit`：TypeScript: No errors found，EXIT 0
- `pnpm build`：三入口产出，EXIT 0，built in 353ms
