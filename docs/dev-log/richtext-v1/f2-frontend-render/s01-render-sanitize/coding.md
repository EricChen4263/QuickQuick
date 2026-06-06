---
id: RT1-F2-S01-code
type: coding_record
level: 小功能
parent: RT1-F2
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: 2b5d985
acceptance_ids: [RT1-F2-A01, RT1-A-SEC]
evidence:
  - src/panels/clipboard/sanitize-html.ts
  - src/panels/clipboard/ClipPreview.tsx
  - src/clip-popover/PopoverPreview.tsx
author: coder
---

# 编码记录 · RT1-F2-S01 富文本预览渲染 + DOMPurify 清洗

## 做了什么
主窗口预览区与 popover 预览区在富文本条目（kind=richtext + htmlContent）时，经 DOMPurify 清洗后用 `dangerouslySetInnerHTML` 渲染真富文本；否则维持纯文本。列表行不变。

## 关键决策与理由
- **共用封装 `sanitize-html.ts::sanitizeRichHtml`**：两处预览复用，单一清洗入口，避免重复 DOMPurify 调用、便于审计"所有 html 入 DOM 前必清洗"。
- **守卫 `kind==='richtext' && htmlContent!==undefined`**：纯文本/图片/undefined 不误入富文本路径。
- **sanitize 只在渲染层**（设计§五）：后端原样保存未清洗 html（保真），前端渲染前清洗——粘贴/复制回去拿到的是用户原始格式。
- **DOMPurify 默认配置**：默认即剥 `<script>`/事件属性/`javascript:`/`<iframe>`，保真优先不自定义白名单。
- **未装 @types/dompurify**：DOMPurify 3.x 自带 .d.ts，废弃 stub 反成误导依赖。

## 改动文件
- `src/panels/clipboard/sanitize-html.ts`（新）— `sanitizeRichHtml(html)`
- `src/panels/clipboard/ClipPreview.tsx` — `PreviewContent` 组件富文本分支
- `src/clip-popover/PopoverPreview.tsx` — 同样富文本分支
- `package.json`/`pnpm-lock.yaml` — 加 dompurify
- 测试：ClipPreview.test.tsx（新）、PopoverPreview.test.tsx（追加）

## 自测结论（TDD 红-绿-重构）
- 先写富文本渲染 + 纯文本回退 + 恶意剥离测试（RED：仍纯文本渲染），实现后 GREEN。
- 安全测试覆盖四类 payload：`<script>`/`onerror`/`javascript:`/`<iframe>`（I-1 补强后），断言被剥离。
- `pnpm test` 476 passed；`pnpm exec tsc --noEmit` 0 错。
- 审查 I-2（`ClipItem.kind` 收窄为字面量联合）属预存在、并入 RT1-F2-S02 处理。
