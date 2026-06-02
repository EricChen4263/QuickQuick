---
id: f3-popover-s05-batch-c1
title: "里程碑4 popover · Batch C1：trans-popover 自动翻译"
status: 实现完成
commit: d5b4c78
date: 2026-06-02
---

## 新建 / 改动文件

- `src/trans-popover/source-text.ts` — pickLatestText(items)：取 [0] 可译文本，图片/空 → null
- `src/trans-popover/source-text.test.ts` — 5 条单元测试
- `src/trans-popover/MiniTranslate.tsx` — 纯展示组件：方向行 + 译文区 + 复制/朗读/展开按钮
- `src/trans-popover/TransPopoverApp.tsx` — 挂载自读 listClipItems()[0]，自动翻译，status 五态
- `src/trans-popover/trans-popover.test.tsx` — 5 条组件集成测试
- `src/trans-popover/popover.css` — 迷你 320 布局，玻璃拟态样式

## 取词偏离决策

前端挂载后直接读 listClipItems()[0]，替代原定 Rust emit 事件方案——避免前端监听就绪前事件已到达的竞态，同时零 Rust 改动。

## TDD 红绿

先写 pickLatestText 单测（空/图片/空白全红），再实现 4 行函数转绿；组件测试先以未实现组件确认红，再实现 TransPopoverApp 转绿。

## 验证结果

- pnpm test：36 文件 311 条全通过（source-text 5 条 + trans-popover 5 条均通过）
- pnpm exec tsc --noEmit：0 错误
- pnpm build：exit 0，三入口产出（dist/index.html + dist/src/clip-popover/index.html + dist/src/trans-popover/index.html）

## C2 衔接点

- onExpand：当前为空函数占位（注释指向 C2），待 C2 接跨窗口跳转 main 逻辑
- 窗口获焦重读：需在 C2 监听 focus 事件重触发 listClipItems + translateText
