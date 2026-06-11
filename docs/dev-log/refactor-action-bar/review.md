---
id: refactor-action-bar-review
type: review
level: 小功能
parent: refactor-action-bar
children: []
created: 2026-06-11T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: []
evidence:
  - "git diff HEAD (unstaged) — 8 文件改动"
  - "新增文件全文读取：ActionBar.tsx / ActionBar.css / ActionBar.test.tsx"
  - "接入方全文读取：PopoverFooter.tsx / MiniTranslate.tsx / ClipPreview.tsx(150行起) / TranslateWorkspace.tsx(85行起)"
  - "CSS 上下文：clipboard.css container-type 行确认(line 213) / translate.css tx-cta 区块 / trans-popover/popover.css 完整"
  - "theme/components.css .btn 定义确认 display:inline-flex"
  - "ARIA 规范：footer 在非区段内容祖先中暴露为 contentinfo landmark，页面不得有多个 contentinfo"
  - "复审：ActionBar.tsx 全文 / ActionBar.test.tsx 全文 / PopoverFooter.tsx 全文 / translate.css:102-116"
author: code-reviewer
---

# 审查结论 · ActionBar 条壳公共组件（refactor-action-bar）

## 审查范围

新建 `src/components/ActionBar.tsx/.css/.test.tsx`，并在五处接入：
- A：`src/clip-popover/PopoverFooter.tsx`（kbd 提示条）
- B：`src/trans-popover/MiniTranslate.tsx`（译文卡操作行）
- C：`src/panels/translate/TranslateWorkspace.tsx`（tx-cta + tx-actions 两处）
- D：`src/panels/clipboard/ClipPreview.tsx`（preview-actions）

对应 CSS：`clip-popover/popover.css`、`trans-popover/popover.css`、`translate/translate.css`、`clipboard/clipboard.css`。

## 发现问题（置信度 ≥ 80 才报）

### F1 · Critical · 置信度 90

**`<footer>` 在非页脚语境产生多余 contentinfo landmark，破坏辅助技术导航**

- `src/components/ActionBar.tsx:36` — 根源，硬编码 `<footer>`
- `src/panels/translate/TranslateWorkspace.tsx:97` — tx-cta（翻译 CTA 行）
- `src/panels/translate/TranslateWorkspace.tsx:145` — tx-actions（译文操作行）
- `src/panels/clipboard/ClipPreview.tsx:163` — preview-actions（预览操作栏）
- `src/trans-popover/MiniTranslate.tsx:27` — mini-actions（译文卡操作行）

**问题**：HTML 规范规定，`<footer>` 不在任何区段内容元素（`<article>`/`<section>`/`<aside>`/`<nav>`）内时，浏览器/辅助技术将其暴露为 `contentinfo` landmark。上述四处接入点的祖先链全为 `<div>`，均不存在区段内容元素，因此每处 ActionBar 都会产生一个 `contentinfo` landmark。其中 TranslateWorkspace 在同一页面渲染两个 ActionBar（tx-cta + tx-actions），产生 2 个 `contentinfo`。ARIA landmark 规范要求页面/文档级 `contentinfo` 唯一，多个 `contentinfo` 会污染屏幕阅读器的 landmark 导航（NVDA/VoiceOver F 键/landmark 快捷跳转），误导视障用户理解页面结构。

**A（PopoverFooter）豁免**：整个 clip-popover 弹窗只产生一个 `contentinfo`，且该条确实是弹窗底部快捷键提示区，语义上成立，不作为问题。

**建议修复**：
- ActionBar 默认渲染 `<div>`，新增 `as?: "div" | "footer"` prop（默认 `"div"`）
- `PopoverFooter` 传 `as="footer"` 保持其弹窗页脚语义
- 其余 B/C/D 接入点不传 `as`，使用默认 `<div>`，消除多余 landmark

---

### F2 · Important · 置信度 82

**tx-cta 的 gap: 10px 被静默丢失**

- `src/panels/translate/translate.css:111-114`

**问题**：旧 `.tx-cta` 有 `gap: 10px`（翻译按钮与字符数之间的视觉呼吸感）。引入 ActionBar 后，局部 override 规则 `.tx-cta.qq-action-bar { border-top: none; padding: 0; }` 抹掉了 padding，但未恢复 gap，导致使用 `qq-action-bar--surface` 默认的 `gap: 8px`，原设计 10px 被丢失。

**建议修复**：在 `.tx-cta.qq-action-bar` 块补 `gap: 10px;`：
```css
.tx-cta.qq-action-bar {
  border-top: none;
  padding: 0;
  gap: 10px;   /* 恢复原 tx-cta 呼吸感，surface 变体默认 8px 偏窄 */
}
```

---

## 通过项

- **CSS 特异性覆盖**：`.tx-cta.qq-action-bar`（0,2,0）> `.qq-action-bar--surface`（0,1,0），border-top/padding 覆盖可靠，与加载顺序无关。
- **B 等宽按钮**：`.btn` 定义为 `display:inline-flex`，`flex:1` + `justify-content:center` 均有效。
- **D container query**：`@container (max-width:520px)` 块完整保留，容器测量对象（`.clip-preview` 的 `container-type:inline-size`）未变，响应式仍生效。
- **A gap:18px 覆盖**：popover.css 页面层后加载，`.popover-footer { gap:18px }` 同特异性后者胜，覆盖成立。
- **死代码**：`popover-footer-hint`/`mini-btn` 类已从 HTML 和 CSS 同步清除，无残留。
- **测试质量**：ActionBar.test.tsx 7 条用例均非恒真/旁路/循环论证，覆盖 glass/surface 变体、align 默认值、children 透传、className 追加不覆盖内置类、ActionBarHint 文本。
- **规范符合**：2 空格缩进、禁 any、具名导出、函数 ≤50 行、注释写原因、无装饰性横线分隔注释，全部通过。

## 必改项（打回原因）

1. **F1**：`ActionBar.tsx:36` 改 `<footer>` → `<div>`，加 `as` prop，PopoverFooter 传 `as="footer"`，其余接入点移除 footer 语义。（正确性/可访问性红线）
2. **F2**：`translate.css` 的 `.tx-cta.qq-action-bar` 补 `gap: 10px`。（视觉回归，建议同批修）

## 初审裁决

**BLOCK — 未过**

F1 属可访问性正确性问题（多余 contentinfo landmark），置信度 90，达 Critical 门槛。F2 为视觉回归 Important，建议随 F1 一并修复。

---

## 复审结论（2026-06-11）

### 复审范围

仅复核 F1、F2 两项修复，并检查是否引入新问题。

### F1 复核

`src/components/ActionBar.tsx`（全文读取确认）：

- `as?: "div" | "footer"` 已加入 `ActionBarProps` 接口（line 17），类型安全，无 any，联合类型穷举正确。
- 解构默认值 `as = "div"`（line 30），默认渲染 `<div>` 符合修复方向。
- `const RootTag = as`（line 43）+ `<RootTag ...>`（line 44）—— JSX 动态标签需首字母大写局部变量，实现正确，TypeScript 能推导出 `RootTag` 为 `"div" | "footer"` 字面量类型，JSX 元素合法，无类型安全问题。
- `PopoverFooter.tsx:9` 传 `as="footer"`，弹窗唯一页脚保留 contentinfo landmark，符合预期。
- B/C/D 接入点均不传 `as`，渲染 `<div>`，不再产生多余 contentinfo landmark。**F1 完全修复，无新问题引入。**

`ActionBar.test.tsx`（全文读取确认）：

- 原「footer 根元素」用例已拆为两条：
  - `"不传 as 默认渲染 div 根元素且含 glass 变体类"`（line 6-12）：`tagName === "DIV"` + glass 类双断言，非恒真，覆盖默认值路径。
  - `"as=footer 渲染 footer 根元素"`（line 14-21）：`tagName === "FOOTER"` 精确断言，非恒真。
- 两条用例均直接渲染被测组件，非旁路。**测试修改正确。**

### F2 复核

`src/panels/translate/translate.css:111-116`（读取确认）：

```css
.tx-cta.qq-action-bar {
  border-top: none;
  padding: 0;
  /* 恢复原 CTA 行 10px 间距（surface 默认 8px 偏窄） */
  gap: 10px;
}
```

`gap: 10px` 已补入，注释说明原因。**F2 完全修复，无新问题引入。**

### 新问题扫描

无新问题引入。`RootTag` 动态标签模式是 React + TypeScript 的标准惯用法，无运行时或类型风险。

### 复审裁决

**APPROVE — 通过**

F1（contentinfo landmark 多余）与 F2（gap 丢失）均已正确修复，无新问题引入。coder 报告 `pnpm run build` exit 0、`pnpm test` 514 passed，与静态审查结论一致。
