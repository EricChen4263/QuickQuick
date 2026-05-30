---
id: V1-F2-S04-code
type: coding_record
level: 小功能
parent: V1-F2
children: []
created: 2026-05-30T23:02:46Z
status: 通过
commit: WIP
acceptance_ids: [V1-F2-A09, V1-F2-A10, V1-F2-A12]
evidence:
  - src/panels/history/search.ts
  - src/panels/history/filter.ts
  - src/panels/history/keyboard.ts
  - src/panels/history/history-search.test.ts
  - src/panels/history/history-filter.test.ts
  - src/panels/history/keyboard-nav.test.ts
author: coder
---

# 编码记录 · V1-F2-S04 历史面板搜索/筛选/键盘流纯逻辑

## 做了什么

实现历史面板前端三条纯逻辑：实时搜索过滤（filterBySearch）、类型筛选（filterByType）、键盘流高亮导航（moveHighlight / quickSelectIndex / resolveEnter）。全部为无副作用纯函数，不涉及 React 组件、DOM 或样式。

## 关键决策与理由

- **HistoryItem 定义在 search.ts 并由 filter.ts / keyboard.ts 导入**：单一数据源，避免类型重复定义；filter 和 keyboard 仅关心行为，不需要拥有类型定义。
- **filterBySearch 对纯空白 query 视为空（trim 后判空）**：用户输入空格不应触发过滤，符合实际交互预期；否则空格会导致列表清空，体验异常。
- **moveHighlight count=0 返回 -1 而非 0**：-1 表示"无有效高亮"语义明确；调用方可据此跳过渲染高亮态，避免对空列表做无意义的 clamp(0,0)。
- **quickSelectIndex 接收 string 而非 KeyboardEvent**：纯函数只做数字→索引映射，不依赖 DOM 事件对象，便于单测且职责单一；调用方负责检测 Cmd 修饰键并提取 event.key 传入。
- **焦点不移动**：moveHighlight 只返回新索引值，完全不操作 DOM 焦点——与设计文档§八#4 "Spotlight 键盘模型：焦点始终在搜索框" 一致；真实 React 组件绑定留后续小功能。
- **测试文件命名带前缀**：per CL-V1-001 校正，文件命名为 history-search.test.ts / history-filter.test.ts / keyboard-nav.test.ts，使 vitest 子串过滤可命中冻结 runner。

## 改动文件

- `src/panels/history/search.ts` — HistoryItem 类型定义 + filterBySearch 实现
- `src/panels/history/filter.ts` — HistoryFilter 类型定义 + filterByType 实现
- `src/panels/history/keyboard.ts` — moveHighlight / quickSelectIndex / resolveEnter 实现
- `src/panels/history/history-search.test.ts` — A09 测试（6 例）
- `src/panels/history/history-filter.test.ts` — A10 测试（5 例）
- `src/panels/history/keyboard-nav.test.ts` — A12 测试（15 例）

## 自测结论（TDD 红-绿-重构）

- RED：先写三个测试文件，运行确认因实现文件缺失而失败（exit=1，"no tests"）。
- GREEN：依次创建 search.ts / filter.ts / keyboard.ts，三个 runner 各自通过：A09=0（6/6）、A10=0（5/5）、A12=0（15/15）。
- 全量：`pnpm test` 5 个文件 31 例全绿；`tsc --noEmit` 无错；`pnpm build` 成功。
- 无装饰性分隔注释（grep 无命中）；无 TODO/FIXME（grep 无命中）。

code-standards 逐项自检：
- 格式：2 空格缩进，行宽均在 120 以内，双引号，语句末无分号（TS 惯例）。
- 函数：每个函数均 ≤ 50 行，参数 ≤ 3 个，嵌套 ≤ 2 层，单一职责。
- 命名：camelCase 函数名，动词+名词（filterBySearch / moveHighlight / resolveEnter），类型 PascalCase（HistoryItem / HistoryFilter）。
- 注释：JSDoc 解释参数/返回值和设计语义（"为什么"），无装饰性分隔符。
- 类型：无 any，导出接口显式类型，使用 interface / type，不可变（filter/map 返回新数组）。
- 测试：AAA 结构，行为化命名（"空 query 返回全部条目"、"ArrowDown 在末尾时 clamp 不越出"等），非恒真断言。
- 安全：纯逻辑函数，无外部依赖，无安全风险项。

## 审查修复记录（code-reviewer 打回第 1 次 → 已修复）

按审查 I-01 快捷路径改 `[...items]` + 引用断言、I-02 移除冗余 `?? null`：

- **I-01**：`search.ts` 空 query 快捷路径 `return items` → `return [...items]`；`filter.ts` filter=all 快捷路径 `return items` → `return [...items]`。分别在 `history-search.test.ts`（"空 query 返回全部条目"）和 `history-filter.test.ts`（"filter=all 返回全部条目"）各补一条 `expect(result).not.toBe(items)` 断言，确保快捷路径返回新数组引用、内容不变。
- **I-02**：`keyboard.ts` `resolveEnter` 末行 `return items[highlight] ?? null` → `return items[highlight]`，上方越界守卫已保证安全，移除冗余空值合并。

回归结论：A09=0（6/6）、A10=0（5/5）、A12=0（15/15）；全量 fe_all=0（31/31）；tsc=0 无错。
