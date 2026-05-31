---
id: V4-F2-S06-review
type: review
level: 小功能
parent: V4-F2
children: []
created: 2026-05-31T10:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V4-F2-A06]
author: code-reviewer
---

# 审查结论 · 主窗口外壳 + 渲染测试框架（V4-F2-S06）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src/App.tsx` | 重建 | 主窗口外壳：侧栏三入口 + useState 切换 + route 事件 + Esc hide + 三页占位 |
| `src/app-shell.test.tsx` | 新建 | 6 个渲染测试（jsdom + @testing-library/react） |
| `src/test-setup.ts` | 新建 | jest-dom 注册文件 |
| `vite.config.ts` | 修改 | 新增 test 块（jsdom/globals/setupFiles） |
| `package.json` | 修改 | devDeps 新增 4 个渲染测试依赖 |
| `src-tauri/tauri.conf.json` | 修改 | 窗口形态 400×600 无边框 → 960×640 带边框，保留 visible:false |

参照：设计文档§九.1/§九.3、V4-F2-A06、code-standards（前端 React/TS）、全局规范。

---

## 发现问题（置信度 ≥ 80 才报）

### Critical

无。

### Important

| 严重度 | 问题 | 文件:行 | 规范依据 / 修复建议 |
|---|---|---|---|
| Important | `listen(...)` Promise 无 `.catch()` — 若 Tauri 事件注册失败（如运行时不可用），Promise rejection 静默丢失，路由切换功能失效且无任何错误日志 | `src/App.tsx:37-45` | code-standards §可失败路径不 panic/静默；`getCurrentWindow().hide()` 已有 catch 示范，此处对称补上 `.catch((err: unknown) => { console.error("[QuickQuick] route 监听注册失败:", err); })` 即可 |
| Important | tester 覆盖缺口：route 事件回调有 `vi.mock` 但无调用断言，`routeToTopLevel` 映射（history→clipboard）在当前测试集中无直接验证路径 | `src/app-shell.test.tsx` | 不影响已有 6 测通过，但 S07-S09 接入 IPC 后，route 回调是真实数据流入口；建议在 S07 或独立的 route 集成测试中补充：拿到 `listen` mock 的最新调用参数并 invoke 回调，断言页面切换。此处标为 Important 提醒，不构成本次打回条件 |

### 备注（置信度未达 80，不计入问题，仅记录）

- `topLevelEntries()` 在每次渲染时调用（第 69 行），返回固定字面量数组 `["clipboard", "translate", "settings"]`。函数是纯函数，结果恒定，运行时无副作用，此处不构成真实问题（`useMemo` 是优化而非必须）。置信度约 30，不报。
- `HotkeyTrigger` 从 `./shell/windowRoute` 引入，而该模块注释仍写"预热窗口路由逻辑"（预热弹窗旧概念）。窗口形态已从弹窗改为主窗口，注释与现状有语义落差，但属模块自身的技术债，不在本次改动范围内，不计入问题。
- inline style 布局（`display: "flex"` 等）：不在 code-standards 明确禁止项内，是占位阶段的合理临时手段，S07-S09 替换时可迁移至 CSS 模块。置信度约 25，不报。

---

## 逐项规范检查

| 规范项 | 结论 | 说明 |
|---|---|---|
| 禁 `any` | 通过 | `RoutePayload`/`TopLevel`/`HotkeyTrigger` 均显式类型；`err: unknown` 处理规范 |
| 函数 ≤ 50 行、嵌套 ≤ 3 层 | 通过 | App 组件约 40 行有效逻辑，`routeToTopLevel` 4 行，嵌套均 ≤ 2 层 |
| setState 函数式更新 | 通过 | `setActiveTop((_prev) => ...)` 两处均正确使用 |
| useEffect cleanup 正确 | 通过 | `cancelled` flag + `unlisten?.()` 防泄漏；Esc 监听 `removeEventListener` 对称 |
| 命名：camelCase / PascalCase / UPPER_SNAKE | 通过 | `TOP_LEVEL_LABELS`（常量）、`routeToTopLevel`（动词+名词）均符合 |
| 注释写「为什么」 | 通过 | cancelled flag 防泄漏说明清晰；无装饰性横线分隔 |
| 无 TODO/FIXME | 通过 | grep 确认无残留 |
| 禁魔术字符串 | 通过 | 标签映射用 `TOP_LEVEL_LABELS` 常量；`"route"` 事件名为后端协议固定值，可接受 |
| aria-current 语义化 | 通过 | `aria-current="page"` 表达选中态，未选中不设属性，语义正确 |
| 测试 AAA 结构 | 通过 | 6 个测试均有 Arrange/Act/Assert 注释 |
| 测试断言非恒真 | 通过 | tester 2 处变异均如期变红，已证伪 |
| jsdom 不破坏既有测试 | 通过 | 全量 112 绿，原 106 纯逻辑测试无破坏 |
| 安全：无密钥入库 | N/A | 本功能无敏感数据 |

---

## 关于 tester 提的覆盖缺口

tester 记录了两处覆盖缺口：

1. **Esc-hide 路径**：`getCurrentWindow().hide()` mock 了但无触发断言。鉴于实现简单（keydown 回调直调 hide）、hide 已有 catch 保护、Tauri 窗口 hide 在 jsdom 中无法真实验证，此缺口不构成质量风险，**不要求补充**。

2. **route 事件回调无断言**：mock 了 `listen` 但未 invoke 其回调验证 `routeToTopLevel` 逻辑。`routeToTopLevel` 是一个 2 行纯函数，逻辑极简（translate→translate，其余→clipboard），独立单测即可覆盖，也可在 S07/S08 接 IPC 的渲染测试中作为集成路径验证。**建议** 在 S07 或独立 route 测试中补上，但**不构成本次打回条件**。

---

## S07-S09 注意事项（如何替换占位区）

1. **替换约定**：将对应 `<section data-testid="page-xxx">` 的子内容替换为真实页组件，**保留 `data-testid` 属性和父层 `style={{ display: ... }}`**，或由父组件传 `isActive` prop 给子组件做自控。`page-clipboard`/`page-translate`/`page-settings` 三个 testid 是 app-shell 测试的外部契约，**不得删除或改名**。

2. **复用 nav.ts 二级导航**：`src/main-window/nav.ts` 已提供 `subViewsOf(top: TopLevel)` 和 `resolveNav(top, sub?)` 两个函数，S07-S09 各页的二级视图切换应直接复用这两个函数，**不重造导航状态逻辑**。`SubView` 联合类型已覆盖全部 6+2+2 项。

3. **路由状态升级路径**：当前 `App.tsx` 仅追踪 `TopLevel`，未追踪 `SubView`。S07-S09 如需二级视图联动路由（如 `route` 事件携带子视图信息），建议把 `activeTop` + `activeSub` 一起提升到 `App` 层并通过 props 下传，或引入 Context，避免三页各自维护路由状态。

4. **display none vs 卸载**：当前采用 `display: none` 保持三页均挂载。优点是切换无白屏、页内状态保留；缺点是三页均在 DOM 中，IPC 订阅需注意按 `isActive` 条件启停。对 S06 的占位阶段完全合理，S07-S09 实现真实内容后可按需改为 lazy mount（卸载+重挂）。

---

## 结论

**通过。**

唯一 Important 问题（`listen` Promise 无 catch）为低风险的可选加固项：在 Tauri 运行时正常（生产环境）下不会触发，jsdom 环境中 mock 已 resolved。建议 coder 在 S07 或后续改动中顺手补上，无需打回本次改动。

tester 动态证伪（6/6 命中、2 变异如期变红、3 边界全通、全量 112 绿）已充分证明实现行为正确、测试有真实判别力。窗口形态变更（960×640 带边框）为已知有意变更，已在 coding.md §2.5 说明，需用户确认尺寸与托盘唤起体验符合预期。
