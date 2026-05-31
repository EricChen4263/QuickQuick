---
id: V3-F3-S08-test
type: test_report
level: 小功能
parent: V3-F3
created: 2026-05-31T03:31:51Z
status: 通过
commit: WIP
acceptance_ids: [V3-F3-A09, V3-F3-A10]
author: tester
---

# V3-F3-S08 设置改键 UI + 设置子项栏 — 测试报告

## 执行命令

```bash
# A09 改键冲突校验
pnpm test rebind-ui > /tmp/T8.log 2>&1
# A10 设置子项栏
pnpm test settings-sections > /tmp/T8b.log 2>&1
# 前端全量
pnpm test > /tmp/T8all.log 2>&1
# TypeScript 类型检查
pnpm exec tsc --noEmit > /tmp/T8t.log 2>&1
```

## 结果汇总

| 命令 | exit code | 结果 |
|------|-----------|------|
| `pnpm test rebind-ui` | 0 | 通过 |
| `pnpm test settings-sections` | 0 | 通过 |
| `pnpm test`（全量） | 0 | 通过 |
| `pnpm exec tsc --noEmit` | 0 | 通过 |

## A09 — 改键冲突拒绝（rebind-ui.test.ts）

测试文件：`src/main-window/settings/rebind-ui.test.ts`
运行结果：**5 passed (5)**，耗时 340ms

| # | 用例名称 | 结果 | 验收点 |
|---|----------|------|--------|
| 1 | 新快捷键已在占用列表中，返回 ok:false 且 error 为"已被占用" | 通过 | A09 核心拒绝路径 |
| 2 | 占用列表含多项时，任意命中均返回 ok:false error:"已被占用" | 通过 | A09 多项占用 |
| 3 | 新快捷键不在占用列表中，返回 ok:true 且携带 accelerator | 通过 | A09 空闲键允许 |
| 4 | 占用列表为空时，任何键均通过 | 通过 | A09 边界：空列表 |
| 5 | 大小写不同视为不同键（空闲） | 通过 | A09 大小写区分 |

## A10 — 设置六子项 + App 排除名单（settings-sections.test.ts）

测试文件：`src/main-window/settings/settings-sections.test.ts`
运行结果：**11 passed (11)**，耗时 375ms

| # | 用例名称 | 结果 | 验收点 |
|---|----------|------|--------|
| 1 | 返回恰好六项 | 通过 | A10 六项数量 |
| 2 | 包含 general / hotkey / translate-source / privacy / storage / about | 通过 | A10 固定六项 ID |
| 3 | 顺序固定：general 在最前，about 在最后 | 通过 | A10 顺序 |
| 4 | 向空列表添加一个 app，返回含该 app 的新列表 | 通过 | A10 排除名单新增 |
| 5 | 不改变原数组（不可变） | 通过 | A10 不可变性 |
| 6 | 重复添加同一 app 去重，不产生重复项 | 通过 | A10 去重 |
| 7 | 添加不同 app 正常追加 | 通过 | A10 追加多项 |
| 8 | 移除存在的 app，返回不含该项的新列表 | 通过 | A10 排除名单移除 |
| 9 | 不改变原数组（不可变） | 通过 | A10 不可变性（移除） |
| 10 | 移除不存在的 app，返回与原列表相同内容的新列表 | 通过 | A10 移除不存在项 |
| 11 | 移除后列表为空时返回空数组 | 通过 | A10 边界：清空 |

## 前端全量（74 用例）

运行器：Vitest v2.1.9，耗时 419ms

| 测试文件 | 用例数 | 结果 |
|----------|--------|------|
| src/smoke.test.ts | 1 | 通过 |
| src/panels/history/paste-mode.test.ts | 2 | 通过 |
| src/main-window/settings/rebind-ui.test.ts | 5 | 通过 |
| src/translate/translate-actions.test.ts | 8 | 通过 |
| src/main-window/settings/settings-sections.test.ts | 11 | 通过 |
| src/panels/history/history-search.test.ts | 6 | 通过 |
| src/shell/windowRoute.test.ts | 4 | 通过 |
| src/panels/history/history-filter.test.ts | 5 | 通过 |
| src/translate/select-trigger.test.ts | 4 | 通过 |
| src/main-window/main-nav.test.ts | 13 | 通过 |
| src/panels/history/keyboard-nav.test.ts | 15 | 通过 |
| **合计** | **74** | **11/11 文件通过** |

## 覆盖缺口

本次改动涉及 `rebind.ts`（校验逻辑）和 `settings-sections.ts`（子项列表 + 名单操作）两个模块，测试文件对两个模块的核心逻辑路径（正常路径、边界、不可变性、去重）均有覆盖，无明显缺口。

UI 层（React 组件）尚无快照/交互测试，属于既有整体覆盖策略，非本任务新增缺口。

## 结论

- A09 exit=0，5/5 通过
- A10 exit=0，11/11 通过
- 前端全量 74/74 通过，tsc 类型检查 exit=0
- **门禁结论：允许进入下一任务**
