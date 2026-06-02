---
id: f3-popover-s03-batch-b1-review
title: "Batch B 规范审查"
type: review
level: 小功能
parent: f3-popover
created: 2026-06-02T00:00:00Z
status: 审查通过
commit: 3174754
acceptance_ids: []
author: code-reviewer
---

# Review · Batch B（clip-popover 完整 UI）

## 审查范围

`git diff 273c548 3174754 --` 覆盖以下文件：

- `src/components/KindIcon.tsx`（新建）
- `src/panels/clipboard/ClipItemRow.tsx`（改：KindIcon 提取）
- `src/clip-popover/grouping.ts` + `grouping.test.ts`
- `src/clip-popover/keyboard-nav.ts` + `keyboard-nav.test.ts`
- `src/clip-popover/ClipPopoverApp.tsx`（扩展）
- `src/clip-popover/PopoverList.tsx`（新建）
- `src/clip-popover/PopoverPreview.tsx`（新建）
- `src/clip-popover/PopoverFooter.tsx`（新建）
- `src/clip-popover/popover.css`（新建）
- `src/clip-popover/clip-popover-actions.test.tsx`（新建）

审查标准：项目规范 + code-standards skill。

---

## 问题清单

### 高危（High）

无。

### 中（Important）

**[M-1] Alt+Enter 对图片条目写空字符串到剪贴板**
- 文件：`src/clip-popover/ClipPopoverApp.tsx` 行 100
- 说明：图片条目的 `content` 字段在 db.rs 中以 `unwrap_or_default()` 填充，实际为空字符串。用户选中图片条目按 Alt+Enter 时，`writeToClipboard(selectedItem.content)` 会向系统剪贴板写入空字符串，覆盖原有内容而无实质数据，行为令人困惑。
- 置信度：85
- 建议：在 handleKeyDown 的 Alt+Enter 分支加 kind 守卫——图片条目 Alt+Enter 静默跳过或改走 `getClipImageOriginal` 写图片内容；最小改动是：`if (selectedItem.kind === "image") return;`（与页脚提示「复制」行为一致地排除图片）。

**[M-2] listbox 缺 aria-label，无障碍不完整**
- 文件：`src/clip-popover/PopoverList.tsx` 行 107、114
- 说明：ARIA listbox 角色必须有可访问名称（aria-label 或 aria-labelledby），否则辅助技术无法向用户宣告列表用途。两处 `div[role="listbox"]` 均未标注。
- 置信度：80
- 建议：两处均添加 `aria-label="剪贴板历史"`，与搜索框 aria-label 对应。

### 低（Low）/ 建议

**[L-1] KindIcon 提取为纯搬迁，行为无变化，import 路径正确**
- `ClipItemRow.tsx` 改从 `../../components/KindIcon` 导入，路径正确；组件 SVG 结构与原版逐字节一致，无逻辑变动。没有为 KindIcon 单独添加测试文件，但其渲染行为已通过 clipboard-page.test.tsx 中 ClipItemRow 的集成渲染覆盖（间接），无测试覆盖断层。

**[L-2] grouping.ts isToday 时区语义正确，测试用本地时间字符串规避跨时区 flaky**
- 测试中 `new Date("2026-06-02T12:00:00")` 无 Z 后缀，与本地正午对齐，不依赖时区假设；生产中 `lastModifiedUtc` 是真实 UTC ms，`Date.now()` 也是 UTC ms，`getFullYear/getMonth/getDate` 返回本地日历日——这与「今天」的用户直觉一致，实现正确。

**[L-3] useEffect 依赖数组分析——无无限循环风险**
- `ClipPopoverApp.tsx` 行 61 的 useEffect 依赖 `[visibleFlatList, selectedId]`：selectedId 变化 → effect 运行 → stillVisible=true 则不 setState → 稳定；非 stable 情况 → setSelectedId(first) → 再次运行 → stillVisible=true → 稳定。依赖声明完整，无遗漏，无循环。

**[L-4] promise 处理符合规范**
- Enter/Alt+Enter 两个分支均有 `.then(hide).catch(console.error)` 链，失败时不调 hide、错误有日志，与设计意图一致。测试 `clip-popover-actions.test.tsx` 第 107 行明确验证失败不 hide，策略落实。

**[L-5] CSS token 使用全面，无硬编码颜色**
- popover.css 全部颜色引用 tokens.css 变量（--fg/--muted/--accent/--glass/--hover/--surface-2/--border/--danger 等），均已在 tokens.css 亮/暗两套中定义，无绕过 token 的写死颜色。`background: transparent` 属于透明意图，不计入硬编码。

**[L-6] advanceSelection 正确复用 moveHighlight，无重写 clamp**
- `keyboard-nav.ts` 行 30 直接调 `moveHighlight(currentIndex, key, flatIds.length)`，边界 clamp 由 moveHighlight 保证；currentId 为 null 或不在列表时均返回 flatIds[0]，符合设计文档注释。12 个测试用例覆盖了所有边界。

**[L-7] 搜索框 type="search" + role="searchbox" 冗余但无害**
- `type="search"` 在部分浏览器/WebView 自带清除按钮，Tauri WebView 中行为可能与桌面端不同，但不构成功能 bug；role="searchbox" 覆盖默认隐式角色，ARIA 语义正确。

---

## 结论

**审查通过（无未决高危）。**

存在 2 个中级问题：[M-1] 图片条目 Alt+Enter 写空字符串（行为困惑，建议修复）；[M-2] listbox 缺 aria-label（无障碍不完整）。两者均不阻断核心功能，不构成高危，可在 Batch B2 或后续迭代修复。核心逻辑（分组、键盘导航、promise 处理、CSS token 规范、KindIcon 提取）均符合项目规范。
