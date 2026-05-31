---
id: V2-F3-S08-review
type: review
level: 小功能
parent: V2-F3
children: []
created: 2026-05-31T02:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V2-F3-A12]
evidence: []
author: code-reviewer
---

# 代码审查报告 · V2-F3-S08（选中即译触发纪律，前端纯逻辑）

## 审查范围
- `src/translate/select-trigger.ts`（SelectEvent/SelectAction/resolveSelectAction）+ `select-trigger.test.ts`（A12 4 case）
依据：code-standards（TS 无 any/纯函数/禁装饰注释/YAGNI/穷举 switch）+ 设计§4.3/§八#5。

## 通过项（核心）
§八#5 触发纪律满足：text_selected→show_icon（绝非 translate）、icon_clicked/hotkey_translate→translate、click_elsewhere→dismiss；switch 穷举 SelectEvent 4 成员（noFallthroughCasesInSwitch 编译期保障）；纯函数无副作用；严格 TS 无 any、字面量联合；测试四映射全覆盖 + `expect(action).not.toBe("translate")` 显式拒绝自动译；AAA、非恒真；无越界（无 UI/面板/方向）；无 TODO/装饰注释；函数 ≤50 行。

## 问题清单（Important，无 Critical）
**[I-01] `SelectAction` 含永不返回的 `"none"` 成员，违反 YAGNI（置信度 85）**
- 位置：`select-trigger.ts`（`SelectAction = ... | "none"`）。resolveSelectAction switch 穷举 4 事件无分支返回 "none"，为死值；下游穷举 switch 须处理永不出现分支制造死代码。
- 修复：删 `| "none"`，将来有"无动作"场景再加。

## 结论
**未过（打回）。** 修 I-01（删 "none" 死值）后复审。核心纪律/类型/纯函数/测试均合格，单项修正即过。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-01 已解决**：SelectAction 删 `| "none"`（现 `"show_icon"|"translate"|"dismiss"`），死值移除；resolveSelectAction 穷举 switch 仍编译过（noImplicitReturns 下完整推断），测试不依赖 none。
- 核心触发纪律（text_selected→show_icon 非 translate）实现+测试双保障未破坏；无新增高危。前端 37 测试绿。
