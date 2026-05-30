---
id: V1-F2-S04-review
type: review
level: 小功能
parent: V1-F2
children: []
created: 2026-05-31T10:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V1-F2-A09, V1-F2-A10, V1-F2-A12]
evidence: []
author: code-reviewer
---

# 审查记录 · 历史面板搜索/筛选/键盘流纯逻辑（V1-F2-S04）

## 审查范围
- `src/panels/history/search.ts` + `history-search.test.ts`（A09）
- `src/panels/history/filter.ts` + `history-filter.test.ts`（A10）
- `src/panels/history/keyboard.ts` + `keyboard-nav.test.ts`（A12）
参照：code-standards（TS 无 any、纯函数、不可变、禁装饰分隔注释）+ 设计§八#4/§九.2。

## 问题清单

### Critical
无。

### Important
**[I-01] 快捷路径返回原数组引用，与 coding.md "返回新数组" 不一致（置信度 82）**
- 位置：`search.ts`（空 query `return items`）、`filter.ts`（filter=all `return items`）
- 问题：两处返回入参引用而非新数组；函数自身不 mutate 入参（断言成立），但 coding.md 称"filter/map 返回新数组"与快捷路径矛盾；调用方改写返回值会静默改原数组。
- 修复（方案A）：两处改 `return [...items]`，并补 `expect(result).not.toBe(items)` 断言。

**[I-02] `resolveEnter` 的 `?? null` 冗余死码（置信度 80）**
- 位置：`keyboard.ts`（`return items[highlight] ?? null`）
- 问题：上方已 `highlight < 0 || highlight >= items.length` 守卫，`items[highlight]` 类型为 `HistoryItem` 不可能 undefined，`?? null` 误导。
- 修复：改 `return items[highlight]`。

## 逐维度核查（通过项）
无 any ✓；纯函数无副作用 ✓；filter 主路径返回新数组 ✓（快捷路径见 I-01）；HistoryFilter 字面量联合 ✓；搜索 trim 判空+大小写不敏感子串 ✓；moveHighlight clamp 正确(count=0→-1) ✓；quickSelectIndex 1~9 映射+非法 null ✓；resolveEnter 负数/越界双守卫 ✓；无装饰注释/无 TODO ✓；测试 AAA、非恒真、边界齐 ✓；无越界 ✓；函数 ≤50 行命名规范 ✓。

## 总结论
**未过（打回）。** 处理 I-01（快捷路径 `[...items]` + 引用断言）+ I-02（移除 `?? null`）。均无运行时 bug、主路径正确，预期一次复查通过。

---

## 复审结论（2026-05-31）

**status=通过**

- **I-01**：search.ts 空 query、filter.ts filter=all 均改 `return [...items]`，history-search/history-filter 测试各补 `expect(result).not.toBe(items)`。
- **I-02**：keyboard.ts resolveEnter 末行改 `return items[highlight]`，移除冗余 `?? null`（越界守卫已保证安全）。
无新引入≥80 高危；纯函数/不可变/无 any 维持。
