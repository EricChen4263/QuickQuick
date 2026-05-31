---
id: V3-F3-S07-test
type: test_report
level: 小功能
parent: V3-F3
created: 2026-05-31T03:23:39Z
status: 通过
commit: WIP
acceptance_ids: [V3-F3-A08]
author: tester
---

# 测试报告 · 主窗口 Shell 导航路由（V3-F3-S07）

## 运行命令

```bash
# 1. main-nav 针对性测试（验收 A08）
pnpm test main-nav > /tmp/T7.log 2>&1; echo "A08=$?"

# 2. 前端全量测试
pnpm test > /tmp/T7all.log 2>&1; echo "fe_all=$?"

# 3. TypeScript 类型检查
pnpm exec tsc --noEmit > /tmp/T7t.log 2>&1; echo "tsc=$?"
```

## 执行结果汇总

| 检查项 | 退出码 | 结论 |
|--------|--------|------|
| `pnpm test main-nav`（A08 针对性） | 0 | 1 文件通过，13 用例全 pass |
| `pnpm test`（前端全量） | 0 | 9 文件通过，58 用例全 pass |
| `pnpm exec tsc --noEmit` | 0 | 无类型错误 |

### main-nav 测试输出原文

```
 ✓ src/main-window/main-nav.test.ts (13 tests) 3ms

 Test Files  1 passed (1)
      Tests  13 passed (13)
   Start at  11:23:53
   Duration  366ms (transform 18ms, setup 0ms, collect 16ms, tests 3ms, environment 0ms, prepare 31ms)
```

### 前端全量测试文件列表

```
 ✓ src/smoke.test.ts                               (1 test)
 ✓ src/shell/windowRoute.test.ts                   (4 tests)
 ✓ src/translate/select-trigger.test.ts            (4 tests)
 ✓ src/translate/translate-actions.test.ts         (8 tests)
 ✓ src/panels/history/history-filter.test.ts       (5 tests)
 ✓ src/main-window/main-nav.test.ts               (13 tests)
 ✓ src/panels/history/keyboard-nav.test.ts        (15 tests)
 ✓ src/panels/history/history-search.test.ts       (6 tests)
 ✓ src/panels/history/paste-mode.test.ts           (2 tests)

 Test Files  9 passed (9)
      Tests  58 passed (58)
```

## 用例与验收映射表

| 用例名称 | 验收 ID | 验证行为 | 结果 |
|---|---|---|---|
| 一级入口恰为三项 | V3-F3-A08 | `topLevelEntries` 返回数组长度 === 3 | **pass** |
| 一级入口内容为 clipboard / translate / settings | V3-F3-A08 | 三项一级入口 id 精确匹配，顺序一致 | **pass** |
| 一级入口不含 history | V3-F3-A08 | history 不出现在一级入口中 | **pass** |
| clipboard 的二级视图含 history | V3-F3-A08 | `subViewsOf('clipboard')` 包含 history 二级 | **pass** |
| translate 的二级视图含 history | V3-F3-A08 | `subViewsOf('translate')` 包含 history 二级 | **pass** |
| settings 的二级视图不含 history | V3-F3-A08 | `subViewsOf('settings')` 不含 history | **pass** |
| clipboard 的二级视图含 list | V3-F3-A08 | `subViewsOf('clipboard')` 包含 list 视图 | **pass** |
| translate 的二级视图含 workspace | V3-F3-A08 | `subViewsOf('translate')` 包含 workspace 视图 | **pass** |
| 默认路由到 clipboard 的默认子视图 | V3-F3-A08 | `resolveNav(undefined, undefined)` 落到 clipboard 默认二级 | **pass** |
| translate + history 解析为翻译历史二级 | V3-F3-A08 | `resolveNav('translate', 'history')` 正确定位 | **pass** |
| clipboard + history 解析为剪贴板历史二级 | V3-F3-A08 | `resolveNav('clipboard', 'history')` 正确定位 | **pass** |
| 无效 sub 回退到一级默认子视图 | V3-F3-A08 | 非法 sub 参数时回退行为正确 | **pass** |
| settings 路由正确 | V3-F3-A08 | `resolveNav('settings', ...)` 路由到 settings | **pass** |

## 覆盖缺口

| 缺口 | 说明 | 风险等级 |
|---|---|---|
| UI 渲染层未覆盖 | 测试验证纯逻辑层（`topLevelEntries` / `subViewsOf` / `resolveNav`），React 组件渲染与点击切换行为无 E2E/集成覆盖 | 低（逻辑层已全覆盖，UI 层留 E2E 阶段）|
| Rust 后端无新增测试 | S07 为纯前端 Shell 任务，Rust 侧无变更，无需新增 | 不适用 |

## 结论

**门禁判定：放行**

验收 A08（主窗口导航路由结构）13 个用例全部 pass，前端全量 58 用例 0 失败，TypeScript 类型检查无错误。
导航层级约束（一级仅 clipboard/translate/settings，历史均为二级）通过纯逻辑测试完整验证。
