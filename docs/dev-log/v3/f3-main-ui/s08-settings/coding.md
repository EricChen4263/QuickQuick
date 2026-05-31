---
id: V3-F3-S08-code
type: coding_record
level: 小功能
parent: V3-F3
created: 2026-05-31T03:30:19Z
status: 通过
commit: WIP
acceptance_ids: [V3-F3-A09, V3-F3-A10]
author: coder
---

# V3-F3-S08 设置改键 UI + 设置子项栏 — 编码记录

## 实现概述

### 改键实时校验拒绝（A09）

`src/main-window/settings/rebind.ts` 导出 `validateRebind(newAccel, occupied)`。

- 输入：新快捷键字符串 + 已占用列表。
- 逻辑：`occupied.includes(newAccel)` 命中即返回 `{ ok: false, error: "已被占用" }`，否则返回 `{ ok: true, accelerator: newAccel }`。
- 判定为严格字符串相等（区分大小写），调用方负责统一格式。
- 纯函数，无副作用，符合实时校验需求。

返回类型为判别联合 `RebindResult = RebindOk | RebindFail`，TypeScript narrowing 在测试与调用侧均可正确收窄。

### 设置六子项（A10）

`src/main-window/settings/sections.ts` 导出三个函数：

- `settingsSections()`：返回固定六项数组副本（`[...SECTIONS]`），顺序为 general / hotkey / translate-source / privacy / storage / about。
- `addExcludedApp(list, app)`：不可变添加，先检查重复再 spread 追加。
- `removeExcludedApp(list, app)`：不可变移除，`filter` 返回新数组。

App 排除名单管理隶属隐私子项，纯函数不可变，符合设计文档 §三 App 排除。

## 关键决策

1. **大小写严格匹配**：`validateRebind` 不做大小写归一化，由调用方（热键录制层）统一输入格式，保持函数职责单一。
2. **`SettingsSection` 复用 `nav.ts` 中已有的 `SettingsSub` 语义**：两者类型字面量一致，但 `sections.ts` 单独声明 `SettingsSection`，避免跨模块隐式耦合；如后续需合并可在重构阶段决策。
3. **`settingsSections()` 返回副本**：每次调用 `[...SECTIONS]`，防止调用方意外 mutate 常量数组。
4. **`addExcludedApp` 去重返回副本而非 Set**：保持 `string[]` 类型与调用侧一致，无需引入 Set 转换开销。

## 改动文件

| 文件 | 说明 |
|---|---|
| `src/main-window/settings/rebind.ts` | 新建：改键实时校验纯函数实现 |
| `src/main-window/settings/rebind-ui.test.ts` | 新建：A09 验收测试（5 cases） |
| `src/main-window/settings/sections.ts` | 新建：设置六子项 + App 排除名单管理纯函数实现 |
| `src/main-window/settings/settings-sections.test.ts` | 新建：A10 验收测试（11 cases） |

## TDD 流程

- RED：先写测试，运行确认因模块不存在而报 `Failed to load url` 失败。
- GREEN：写最小实现，`validateRebind` 用 `includes` 一行判定；`sections.ts` 三函数均 ≤ 10 行。
- REFACTOR：实现已足够简洁，无需重构。

## code-standards 自检

| 项目 | 结论 |
|---|---|
| 禁 `any` | 全部使用严格类型，无 `any` |
| 函数 ≤ 50 行、嵌套 ≤ 3 层 | 最长函数 8 行，嵌套最深 1 层 |
| 纯函数、不可变 | 全部纯函数，spread/filter 不原地 mutate |
| 命名描述性 | `validateRebind`、`addExcludedApp`、`removeExcludedApp`、`settingsSections` |
| 禁装饰注释 | grep 无命中 |
| 禁 TODO/FIXME | grep 无命中 |
| tsc --noEmit | exit=0，无错误 |
| 前端全量测试 | 74 passed，exit=0 |
| A09 rebind-ui | 5 passed |
| A10 settings-sections | 11 passed |

## 审查修复（打回第 1 次 · 2026-05-31）

按 code-reviewer 两项 Important 修复，仅改测试文件，实现不变。

### I-01：settings-sections.test.ts — not.toBe 引用断言

"重复添加去重"用例与"移除不存在的 app"用例各追加 `expect(result).not.toBe(list)`，验证返回新数组引用而非原数组，覆盖不可变保护的关键断言缺失。

### I-02：rebind-ui.test.ts — toMatchObject 强制断言

两处占用拒绝用例原有 `if (!result.ok) { expect(result.error).toBe("已被占用"); }` 卫块在 TypeScript narrowing 下可能被跳过，改为 `expect(result).toMatchObject({ ok: false, error: "已被占用" })`，删去 if 分支，断言无条件执行。

### 回归结论

| 命令 | exit | 通过数 |
|---|---|---|
| pnpm test rebind-ui | 0 | 5 passed |
| pnpm test settings-sections | 0 | 11 passed |
| pnpm test（全量） | 0 | 74 passed |
| tsc --noEmit | 0 | 无错误 |
