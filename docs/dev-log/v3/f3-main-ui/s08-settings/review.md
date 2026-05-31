---
id: V3-F3-S08-review
type: review
level: 小功能
parent: V3-F3
children: []
created: 2026-05-31T04:20:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F3-A09, V3-F3-A10]
evidence: []
author: code-reviewer
---

# V3-F3-S08 代码审查报告（设置改键 UI + 设置子项栏）

## 审查范围
- `src/main-window/settings/rebind.ts`+`rebind-ui.test.ts`（A09）
- `src/main-window/settings/sections.ts`+`settings-sections.test.ts`（A10）
依据：code-standards（禁 any/纯函数/不可变/非恒真/禁装饰注释）+ 设计§九.3/§二/§三。

## 实现质量：合规
validateRebind 判别联合（占用→{ok:false,error:"已被占用"}/空闲→{ok:true}）；settingsSections 恰六项顺序与§九.3 一致；addExcludedApp 去重+不可变 [...list]/removeExcludedApp filter 新数组；无 any/纯函数/无装饰注释/TODO；函数 ≤8 行。

## 问题清单（Important，测试质量；无 Critical）
**[I-01] 排除名单 dedup/no-op 测试缺引用不等断言（置信度 82）**
- 位置：`settings-sections.test.ts`（"重复添加去重" + "移除不存在 app" 用例仅 toEqual 内容，无 `not.toBe(list)`；若实现改 `return list` 原引用测试仍过，不可变约束失保护）。
- 修复：两用例各加 `expect(result).not.toBe(list);`。

**[I-02] rebind error 断言藏在 if 卫块（置信度 80）**
- 位置：`rebind-ui.test.ts`（占用拒绝用例 error 文案断言在 `if(!result.ok){...}` 内，ok 失败时卫块跳过、error 断言静默不执行）。
- 修复：改 `expect(result).toMatchObject({ ok: false, error: "已被占用" });` 删 if 卫块。

## 结论
**未过（打回）。** 修 I-01（排除名单 not.toBe 引用断言）+ I-02（rebind toMatchObject）后复审。实现本体合规。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-01 已解决**：settings-sections.test.ts "重复添加去重"+"移除不存在 app" 两用例各加 `expect(result).not.toBe(list)`，覆盖不可变语义。
- **I-02 已解决**：rebind-ui.test.ts 两处占用拒绝改 `expect(result).toMatchObject({ok:false,error:"已被占用"})`，删 if 卫块，强制断言失败路径。
实现本体未动；无新增高危；rebind-ui 5 + settings-sections 11 全过。
