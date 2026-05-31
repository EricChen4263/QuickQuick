---
id: V3-F3-S07-code
type: coding_record
level: 小功能
parent: V3-F3
created: 2026-05-31T03:22:52Z
status: 通过
commit: WIP
acceptance_ids: [V3-F3-A08]
author: coder
---

# V3-F3-S07 主窗口三栏壳路由逻辑 — 编码记录

## 实现概要

新增 `src/main-window/nav.ts`，提供三个纯函数实现主窗口导航路由逻辑：

- **一级入口仅三个**：`topLevelEntries()` 固定返回 `["clipboard", "translate", "settings"]`，长度恰为 3，不含 history。
- **历史均为二级**：`subViewsOf("clipboard")` 含 `"history"`（剪贴板历史二级）；`subViewsOf("translate")` 含 `"history"`（翻译历史二级）。settings 下无 history。
- **路由解析**：`resolveNav(top, sub?)` 若 sub 不在该 top 的合法列表内则回退到默认子视图（clipboard→list，translate→workspace，settings→general）。

## 关键决策

| 决策 | 理由 |
|------|------|
| 用 `Record<TopLevel, SubView[]>` 静态映射子视图 | 避免 switch/if 嵌套，可读性高，新增入口只需改映射表 |
| `resolveNav` 回退而非抛错 | 导航状态非法时 UI 应静默降级，符合鲁棒性原则 |
| 所有函数为纯函数、无副作用 | 便于单测，符合设计文档§九.3 纯逻辑要求 |
| `SubView` 用 union type 而非 string | 严格类型，禁 any，tsc 可静态检查合法性 |

## 改动文件

| 文件 | 说明 |
|------|------|
| `src/main-window/nav.ts` | 新增：导航路由纯函数实现 |
| `src/main-window/main-nav.test.ts` | 新增：A08 验收单测（13 个用例） |

## TDD 过程

1. **RED**：先写 `main-nav.test.ts`，import `./nav`（不存在），运行报 `Failed to load url ./nav`，确认为功能缺失导致失败。
2. **GREEN**：写 `nav.ts` 最小实现，运行 `pnpm test main-nav`，13/13 通过。
3. **REFACTOR**：实现已足够简洁（纯函数、无嵌套、单一职责），无需额外重构。

## code-standards 自检

- 格式：2 空格缩进，无尾随空格
- 命名：camelCase 函数，PascalCase type，描述性
- 函数：最长函数 8 行，嵌套 1 层，无 any
- 注释：写「为什么」，无装饰性分隔线，无死代码注释
- 类型：严格 TS，SubView union type，Record 映射
- 测试：AAA 结构，非恒真断言，覆盖正常/边界/负例
- 无 TODO / FIXME

## 验证结果

```
A08=0        # pnpm test main-nav → 13 passed (13)
fe_all=0     # pnpm test → 58 passed (58)
tsc=0        # 无类型错误
deco=1       # 无装饰性注释
todo=1       # 无 TODO/FIXME
```

## 审查打回修复记录（第 1 次，2026-05-31）

code-reviewer 发现两项测试质量 Important，按审查意见修复：

- **Issue-1（恒真 toBeDefined）**：将"默认路由到 clipboard"的 `expect(state.sub).toBeDefined()` 改为 `expect(state.sub).toBe("list")`；将"settings 路由正确"的 `expect(state.sub).toBeDefined()` 改为 `expect(state.sub).toBe("general")`。断言改为具体默认值，消除恒真。
- **Issue-2（弱负例）**：将"无效 sub 回退"的 `expect(state.sub).not.toBe("nonexistent")` 改为 `expect(state.sub).toBe("list")`，明确断言回退到 clipboard 的实际默认子视图值。

DEFAULT_SUB 实际值（来自 `nav.ts`）：clipboard→"list"，translate→"workspace"，settings→"general"。

回归结果：
```
A08=0      # pnpm test main-nav → 13 passed (13)
fe_all=0   # pnpm test → 58 passed (58)
tsc=0      # 无类型错误
```
