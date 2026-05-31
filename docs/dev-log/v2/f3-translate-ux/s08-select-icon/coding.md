---
id: V2-F3-S08-code
type: coding_record
level: 小功能
parent: V2-F3
children: []
created: 2026-05-31T01:30:16Z
status: 通过
commit: WIP
acceptance_ids: [V2-F3-A12]
evidence:
  - src/translate/select-trigger.ts
  - src/translate/select-trigger.test.ts
author: coder
---

# 编码记录 · V2-F3-S08 选中即译触发纪律

## 做了什么

实现选中文字的触发纪律：用户选中文字后只冒出一个小图标，必须由用户主动点击图标或按 Cmd+Shift+T 快捷键才触发翻译，禁止选中自动弹译。

## 关键决策与理由

- **纯函数 `resolveSelectAction`**：输入 `SelectEvent`，输出 `SelectAction`，无副作用、无外部状态依赖，天然可测、可复用。否决了直接在组件内写 if-else，因为那样难以独立测试触发纪律逻辑。
- **穷举 switch + TypeScript union type**：`SelectEvent` 定义为字面量联合类型，switch 分支穷举所有成员，编译器会在遗漏分支时报错（`noFallthroughCasesInSwitch: true`），提供编译期安全保障。
- **不引入状态机**：当前四个事件到四个动作的映射是纯映射关系，状态机会增加复杂度而无收益；若后续需要表达"icon_shown 状态下才能 click"的时序约束，再引入。

## 改动文件

- `src/translate/select-trigger.ts` — 新增：`SelectEvent` / `SelectAction` 类型定义 + `resolveSelectAction` 纯函数实现
- `src/translate/select-trigger.test.ts` — 新增：A12 验收测试，4 个 case 覆盖全部事件映射，含"选中不等于 translate"的显式断言

## 自测结论（TDD 红-绿-重构）

- **RED**：先写测试文件，运行确认因实现文件不存在而失败（`Failed to load url ./select-trigger`）。
- **GREEN**：写最小实现 `resolveSelectAction`，4 个测试全部通过（`Tests 4 passed`）。
- **REFACTOR**：实现已是最简形式，switch 穷举 union type，无冗余，不需重构。

code-standards 逐项自检：

| 项目 | 结论 |
|------|------|
| 格式：2 空格缩进、行宽、引号、分号 | 通过 |
| 函数：单一职责、≤50 行、无嵌套 | 通过（函数 12 行）|
| 命名：描述性、动词+名词 | 通过（`resolveSelectAction`）|
| 注释：写"为什么"、无死代码、无装饰分隔符 | 通过（`deco=1`）|
| 类型：无 any、公共接口显式类型、无魔术值 | 通过（`tsc=0`）|
| 测试：AAA 结构、行为化命名 | 通过 |
| 无 TODO/FIXME | 通过（`todo=1`）|
| 全量测试绿 | 通过（37 passed）|

## 审查修复 · 打回第 1 次（I-01 YAGNI）

按 code-reviewer 审查意见删除 `SelectAction` 联合类型中永不返回的 `"none"` 成员。
`resolveSelectAction` 的 switch 穷举所有 `SelectEvent` 成员，函数返回值只有 `"show_icon"` / `"translate"` / `"dismiss"` 三种，`"none"` 是死值，违反 YAGNI。

改动：`src/translate/select-trigger.ts` 第 7 行，删除 `| "none"`，类型改为 `"show_icon" | "translate" | "dismiss"`。

回归结论：select-trigger 4 passed、全量 37 passed、tsc=0，穷举 switch 无需 "none" 分支，编译通过。
