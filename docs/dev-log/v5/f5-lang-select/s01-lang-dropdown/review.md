---
id: V5-F5-S01-review
type: review
level: 小功能
parent: V5-F5
children: []
created: 2026-06-02T00:00:00Z
status: 通过（条件已满足）
commit: PENDING
acceptance_ids: []
author: code-reviewer
---

# 审查结论 · 翻译方向栏改为左右语言下拉框 + 后端支持显式源语

> **条件解除（2026-06-02，编排器只读核对）**：原"有条件通过"的唯一阻塞项——`.lang-selects .wrap`/`.lang-selects .wrap svg` 定位规则缺失致下拉箭头视觉破损——已由后续 CSS 修复补齐（见 coding.md「review 修复」节）；低优先级第34行 `lang-pill` 死注释一并修正。reviewer 已预授"纯样式补全无需重跑动态测试"，编排器只读核对确认两条规则与 `.src-select .wrap svg` 对齐、无 lang-pill 残留、tsc + DirBar 测试绿。状态由"未过"更新为"通过"。

## 审查维度

项目规范 + code-standards：格式 / 命名 / 函数 / 注释 / 类型 / 性能 / 测试 / 安全。

审查文件范围：
- `src-tauri/src/translate/lang.rs`
- `src-tauri/src/ipc/translate.rs`
- `src-tauri/tests/ipc_translate.rs`
- `src/panels/translate/languages.ts`
- `src/panels/translate/DirBar.tsx`
- `src/ipc/ipc-client.ts`
- `src/panels/translate/TranslatePage.tsx`
- `src/panels/translate/TranslateWorkspace.tsx`
- `src/panels/translate/translate.css`

## 发现问题（置信度 ≥ 80 才报）

| 严重度 | 置信度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|---|
| Important | 90 | `.lang-selects .wrap` 缺少 `position: relative`；`.lang-selects .wrap svg` 缺少定位样式，导致语言下拉框内的 SVG 箭头图标不会绝对定位到 select 右侧，以文档流渲染，视觉破损 | `translate.css`（缺失规则，参考第77-107行 `.src-select .wrap` 已有样式） | 补充 `.lang-selects .wrap { position: relative; }` 和 `.lang-selects .wrap svg { position: absolute; right: 7px; top: 50%; transform: translateY(-50%); width: 13px; height: 13px; color: var(--muted); pointer-events: none; }` |
| Low | 85 | 注释过时：`.dir-bar` 注释仍写"lang-pill 左对齐"，但 `.lang-pill` 已在本次改造中删除；该注释描述失实，属于违反"禁注释掉的死代码/过时注释"规范的遗留 | `translate.css:34` | 将注释改为 `/* 语言方向栏：lang-selects 左对齐 + src-select 推到右侧 */` |

## 是否合规

**后端（Rust）**：整体合规。
- `is_explicit_source` / `resolve_direction_with_source` 逻辑正确：None、空串 trim、"auto" 三种无效情况均被 match 覆盖；`unwrap()` 在 `is_explicit_source` 已返回 true（非 None）时安全。
- `AUTO_SOURCE` 具名常量替代魔术字符串，符合规范。
- 函数长度均在 50 行内；嵌套不超过 3 层。
- 测试 AAA 结构规范，命名行为化，覆盖主路径 + 边界。
- `translate_text` Tauri 命令字段名（`source`/`target`）与前端 invoke 对象 key 完全一致，Tauri 按名映射正确。

**前端（TypeScript/React）**：整体合规，但存在 CSS 定义缺失（见上）。
- `languages.ts`：`SOURCE_LANGUAGES` / `TARGET_LANGUAGES` 类型化，`filter` 复用避免重复定义，符合 DRY。
- `DirBar.tsx`：纯展示组件，受控 select，`aria-label` 完整，无 `any`，无死代码。
- `TranslatePage.tsx`：`source=auto` 时传 `undefined`，符合后端语义；`handleTranslate` 依赖数组完整。
- `TranslateWorkspace.tsx`：`onSwap`/`isSwappable` 等旧 prop 已干净移除，无悬空 props。
- `ipc-client.ts`：函数签名参数顺序与 invoke 对象名无关（Tauri 按字段名映射），对齐正确。
- 删 swap 操作干净，未见死代码残留。

**CSS**：`.lang-pill` 类已从 CSS 删除（无孤立规则），但存在两处问题见上表。

## 结论

打回。必须修的项：

1. **（Important）补充 `.lang-selects .wrap` 和 `.lang-selects .wrap svg` 的 CSS 定位规则**（`translate.css`），否则语言下拉框的箭头图标视觉破损——SVG 以文档流出现在 select 之后，而非叠加在右侧。

2. **（Low）更新 `.dir-bar` 注释**（`translate.css:34`），把 `lang-pill` 改为 `lang-selects`，消除过时注释。

第2项为低优先级，如产品节奏紧可与第1项一并处理，但第1项 CSS 缺失属于确定性视觉 bug，必须在放行前修复。
