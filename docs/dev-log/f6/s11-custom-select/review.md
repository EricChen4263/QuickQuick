---
id: F6-S11-review
type: review
level: 小功能
parent: F6
children: []
created: 2026-06-04T00:00:00Z
status: 通过
commit: cca1fd7
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 自定义 Select 下拉组件 + 5 处原生 select 替换（F6-S11）

## 审查范围

| 文件 | 说明 |
|---|---|
| `src/components/Select.tsx` | 自定义下拉组件（portal + fixed 定位，键盘导航，禁用项） |
| `src/components/Select.css` | 组件样式（trigger 外观 + 浮层菜单） |
| `src/components/Select.test.tsx` | 行为单测 9 条 |
| `src/panels/translate/DirBar.tsx` | 3 处原生 select 替换（源语 / 目标语 / 翻译源） |
| `src/panels/clipboard/ClipSearchBar.tsx` | 1 处原生 select 替换（类型筛选） |
| `src/panels/settings/StoragePanel.tsx` | 1 处原生 select 替换（图片阈值） |
| `src/translate/translate-actions.ts` | 移除 switch_target / switch_source_retranslate |
| `src/panels/translate/TranslateWorkspace.tsx` | 移除 dict-slot + 无用 ACTION_LABELS 条目 |
| `src/panels/translate/TranslatePage.tsx` | 删除已不可达的 switch_* 分支 |

参照标准：前端规范（禁 any / 函数式 setState / 2 空格 / 函数≤50行）、code-general（函数≤50行 / 嵌套≤3层 / 注释写"为什么"/ 无死代码）、ARIA 实践（listbox 模式）。

---

## 重点检查判定

### 禁 any / 类型安全（通过）

全文搜索无 `any`、`@ts-ignore`、`@ts-expect`，`as HistoryFilter` 类型断言在 FILTER_OPTIONS 由 `Record<HistoryFilter, string>` 的 Object.keys 构建的前提下安全，不构成 unsafe cast。

### 定位逻辑正确性（通过，有保留）

`getBoundingClientRect()` + `position:fixed` 坐标计算无误；openUpward 判定（`spaceBelow < MENU_MAX_HEIGHT && rect.top > spaceBelow`）正确——仅当上方空间也大于下方时才翻转，避免两侧都不足时盲目向上。`resize`/`scroll` 事件关闭菜单防错位逻辑存在，但 scroll 监听有 bug（见 Critical）。

### 事件监听/内存（通过）

mousedown / resize / scroll 均在 `if (!isOpen) return` 短路 + `useEffect` cleanup 中正确成对 removeEventListener；portal 节点随 `isOpen && menuRect !== null` 条件渲染，React 卸载时自动清除；多实例因 isOpen 状态独立互不干扰。

### mousedown vs click 取舍（通过）

选项 `onMouseDown` 调用 `event.preventDefault()` 阻止焦点迁移，使 trigger 的 `onBlur` 不触发；`commitOption` 在 blur 之前调用成功。blur 处通过 `relatedTarget` 判定焦点是否落入菜单，逻辑正确。

### 行为保真（通过）

5 处替换均保留原 value/onChange/options 语义；`needsKey && !configuredIds.has(id)` 禁用逻辑映射到 `SelectOption.disabled`；StoragePanel 的 `Number(value)` 转换保留（`onChange={(value) => { void handleThresholdChange(Number(value)); }}`），无精度丢失风险（档位均为整数）。translate-actions / TranslateWorkspace / TranslatePage 清理经 grep 确认无其他依赖。

---

## 问题列表

### Critical · 置信度 100

**`window.scroll` capture 监听关闭菜单自身滚动**

文件：`src/components/Select.tsx`，第 99 行

```tsx
window.addEventListener("scroll", close, true);   // useCapture = true
```

菜单 `.qq-select-menu` 有 `overflow-y: auto; max-height: 260px`（Select.css 第 55–56 行），当选项超出可见高度时会出现滚动条。`useCapture=true` 的捕获阶段监听器会在事件下行到目标之前就拦截——包括 `<ul>` 内部滚动触发的 `scroll` 事件。结果：用户在菜单内上下滚动时 `close()` 被调用，菜单立即消失，无法选到视口外的选项。

当前语言列表仅 9 项（≈225px < 260px），暂不触发；但只要语言条目增长超过约 10 项，或项目新增选项更多的 Select 用法，此 bug 即激活。这是架构级缺陷，不依赖任何运行时偶然：选项一超高，必现。

**必须修复。** 在 scroll 事件处理器中排除菜单内部滚动：

```tsx
function closeOnExternalScroll(event: Event) {
  const menu = document.getElementById(listboxId);
  // 菜单内部滚动不关闭；仅外部容器滚动关闭（防 fixed 浮层错位）
  if (menu?.contains(event.target as Node)) return;
  setIsOpen(false);
}
window.addEventListener("scroll", closeOnExternalScroll, true);
return () => window.removeEventListener("scroll", closeOnExternalScroll, true);
```

---

### Important · 置信度 100

**CSS `[data-open="true"]` 选择器死代码——菜单展开时 trigger 无高亮样式**

文件：`src/components/Select.css` 第 31 行 / `src/components/Select.tsx` 第 140 行

```css
/* Select.css:31 */
.qq-select[data-open="true"] .qq-select-trigger {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
```

```tsx
/* Select.tsx:140 — 根 div 从未设置 data-open */
<div ref={rootRef} className={`qq-select${className !== undefined ? ` ${className}` : ""}`}>
```

整个 `src/` 中无任何地方写 `data-open` 属性（grep 确认）。该 CSS 规则永远不匹配。效果：鼠标点击打开菜单时 trigger 没有 accent 边框和光晕，用户无法从视觉上感知下拉已展开（仅键盘聚焦时 `:focus-visible` 能提供该样式，鼠标路径完全无反馈）。

**必须修复。** 在根 div 上设置 `data-open={isOpen}` 属性：

```tsx
<div ref={rootRef} data-open={isOpen} className={...}>
```

这样 `[data-open="true"]` 才能匹配，trigger 在展开时获得正确的高亮状态。

---

### Important · 置信度 90

**`Select` 函数体 171 行，违反函数 ≤ 50 行规范**

文件：`src/components/Select.tsx` 第 47–217 行

`code-general` 规范明确：函数 ≤ 50 行；嵌套 ≤ 3 层。`Select` 函数含 3 个 `useEffect`/`useLayoutEffect`、2 个内嵌函数（`commitOption`、`handleTriggerKeyDown`）、大段 JSX，合计 171 行，超出限制约 240%。这不是大函数可以例外的注释说明场景，组件中每块逻辑均可独立抽取。

**建议拆分（非阻塞但项目规范要求）：**
- `useMenuRect(isOpen, triggerRef)` — 封装 `useLayoutEffect` 坐标计算，返回 `menuRect`
- `useCloseOnOutside(isOpen, rootRef, listboxId, onClose)` — 封装 mousedown listener
- `useCloseOnScroll(isOpen, listboxId, onClose)` — 封装 scroll/resize listener（同时修复上述 Critical）
- `commitOption` / `handleTriggerKeyDown` 已是具名函数，可保留或提取

抽取后主函数约 60–70 行（JSX 本身难以再压缩），勉强接受；若加上 3 个 hook 最终可降至 ≤50。

---

## 无其他置信度 ≥80 问题

- **scroll capture 以外**的事件清理（resize、mousedown）正确，无泄漏。
- **ARIA**: `aria-haspopup="listbox"` + `aria-expanded` + `role="listbox"` + `role="option"` + `aria-selected` + `aria-disabled` 均在位；缺少 `aria-controls` 和 `aria-activedescendant` 属于增强最佳实践，非 WCAG 2.1 硬性要求，置信度 65，不报。
- **setState 非函数式**（`setIsOpen(false/true)`）：赋常量值无并发读写风险，React 并发模式下也不会产生 stale 问题；规范条文针对基于 prev 计算的场景，此处置信度 55，不报。
- **`HistoryFilter` 类型断言**：已由 `FILTER_OPTIONS` 构建逻辑证明安全，不报。

---

## 必改项（打回列表）

| # | 位置 | 描述 |
|---|---|---|
| 1 | `src/components/Select.tsx:99` | scroll capture 排除菜单内部滚动（Critical） |
| 2 | `src/components/Select.tsx:140` + `Select.css:31` | 根 div 加 `data-open={isOpen}` 让 CSS 规则生效（Important） |

函数长度（Important·90）建议修复但不阻塞——作为建议一并处理。

---

## 审查结论

**未过（BLOCK）。**

存在 1 个 Critical（scroll capture 关闭菜单自身滚动）+ 1 个 Important·100（data-open 死代码导致 trigger 无展开高亮），需修复后复审。

---

**VERDICT: BLOCK**

`severity(Critical) · confidence(100) · src/components/Select.tsx:99 · window.scroll capture:true 捕获菜单自身 overflow-y 滚动事件并调用 close()，菜单展开后内部滚动立即关闭 · 在 closeOnScroll 回调中加 menu?.contains(event.target) 检查，内部滚动直接 return`

`severity(Important) · confidence(100) · src/components/Select.css:31 + src/components/Select.tsx:140 · CSS 选择器 [data-open="true"] 对应属性从未在根 div 设置，鼠标展开状态下 trigger 无 accent 边框/光晕，视觉无反馈 · 在根 div 加 data-open={isOpen}`

`severity(Important) · confidence(90) · src/components/Select.tsx:47-217 · Select 函数体 171 行，违反 code-general 函数 ≤50 行规范 · 抽取 useMenuRect / useCloseOnOutside / useCloseOnScroll 三个自定义 hook`

---

## 复审（commit cca1fd7）

初审判定 **BLOCK**（1 Critical + 2 Important）。下列阻塞项已全部修复，复审核实在已提交代码内：

| 项 | 初审问题 | 修复（已核实位置） |
|---|---|---|
| Critical | scroll capture 捕获菜单自身滚动即关闭，长列表无法滚动 | `Select.tsx:427-436` `closeOnExternalScroll` + `menu?.contains(event.target)` 内部滚动 return |
| Important·100 | 根 div 未设 `data-open`，trigger 无展开高亮（死 CSS） | `Select.tsx:61` 加 `data-open={s.isOpen}` |
| Important·90 | Select 函数体 171 行超 ≤50 行规范 | 抽出 `useSelectInteractions`/`useMenuRect`/`useCloseOnOutside`/`useCloseOnScroll` 四个 hook |

后续 GUI 实测：下拉定位、长列表滚动、展开高亮、方向箭头、菜单等宽自适应均 OK，用户确认「没问题了」。

终态：**通过**。
