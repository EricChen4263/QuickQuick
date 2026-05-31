---
id: V3-F3-S07-review
type: review
level: 小功能
parent: V3-F3
children: []
created: 2026-05-31T05:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F3-A08]
evidence: []
author: code-reviewer
---

# V3-F3-S07 主窗口三栏壳路由 — 审查记录

## 审查范围
- `src/main-window/nav.ts`（TopLevel/SubView/topLevelEntries/subViewsOf/resolveNav）+ `main-nav.test.ts`
依据：code-standards（TS 无 any、纯函数、禁恒真断言、禁装饰注释）+ 设计§九.3。

## 实现本体（nav.ts）：合规无问题
一级仅三入口（TopLevel="clipboard"|"translate"|"settings"，topLevelEntries 长度3不含history）✓；历史均二级（clipboard/translate 含 history，settings 无）✓；resolveNav 非法 sub 回退 DEFAULT_SUB 不抛错 ✓；纯函数无副作用、无 any（两处类型断言有合理语义保障）✓；函数 ≤8 行；无装饰注释/TODO/死值。

## 问题清单（Important，测试质量；无 Critical）
**[Issue-1] 恒真断言 `toBeDefined()`（置信度 85）**
- 位置：`main-nav.test.ts`（"默认路由到 clipboard"/"settings 路由正确" 两处 `expect(state.sub).toBeDefined()`）。state.sub 类型 SubView 不含 undefined，断言恒真无检测力。
- 修复：改断言具体默认值（如 `expect(state.sub).toBe("list")` / settings 用其**实际**默认子视图名）。

**[Issue-2] 弱负例断言（置信度 82）**
- 位置：`main-nav.test.ts`（"无效 sub 回退" 用 `not.toBe("nonexistent")`，任何非该值都过，不验回退到具体默认）。
- 修复：改 `expect(state.sub).toBe(<clipboard实际默认子视图>)`。

## 结论
**未过（打回）。** 修 Issue-1（两处恒真改具体值）+ Issue-2（弱负例改具体默认值）后复审。实现本体合规、符合设计§九.3。注：用代码里实际的默认子视图名（DEFAULT_SUB），非臆测。

---

## 复审结论（2026-05-31）

**status = 通过**

- **Issue-1 已解决**：两处 toBeDefined() 改为对照 DEFAULT_SUB 的精确 toBe（clipboard→"list"、settings→"general"），消除恒真。
- **Issue-2 已解决**：无效 sub 回退改 `toBe("list")`（clipboard 实际默认），非弱负例。
nav.ts 实现本体未动；一级三入口/历史二级核心测试完整；无新增高危；main-nav 13 测试全过。
