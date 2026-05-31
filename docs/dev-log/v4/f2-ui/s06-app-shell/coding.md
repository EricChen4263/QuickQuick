# S06 主窗口外壳 + 渲染测试框架 — 编码留痕

- 小功能：S06-app-shell
- 大功能：F2 三页 React UI
- 版本：V4
- 执行者：coder agent (Phase 5)
- 完成时间：2026-05-31

---

## 1. 改动文件清单

| 文件 | 改动说明 |
|------|----------|
| `package.json` | 新增 devDependencies：`@testing-library/react`、`@testing-library/jest-dom`、`@testing-library/user-event`、`jsdom` |
| `vite.config.ts` | 新增 `test` 块：`environment: "jsdom"`、`globals: true`、`setupFiles: ["./src/test-setup.ts"]` |
| `src/test-setup.ts` | 新建：导入 `@testing-library/jest-dom`，为所有渲染测试注册 DOM 匹配器 |
| `src/App.tsx` | 重建为主窗口外壳：左侧边栏三入口 + 主内容区占位 + route 事件集成 + Esc 隐藏 |
| `src/app-shell.test.tsx` | 新建：6 个渲染测试，覆盖三入口渲染、默认页、点击切换、aria-current 选中态 |
| `src-tauri/tauri.conf.json` | 窗口形态变更：width 400→960，height 600→640，resizable false→true，decorations false→true（保留 visible: false） |

---

## 2. 关键实现决策

### 2.1 渲染测试框架引入

- 选用 `@testing-library/react` + `jsdom`，与 vitest 集成通过 `vite.config.ts` 的 `test.environment: "jsdom"` 统一配置。
- `src/test-setup.ts` 在 `setupFiles` 中导入 `@testing-library/jest-dom`，确保每个测试文件自动获得 `toBeInTheDocument`、`toBeVisible` 等 DOM 匹配器。
- jsdom 环境对既有纯逻辑测试无破坏（已验证：原 106 个测试 + 新增 6 个 = 112 个全绿）。

### 2.2 主窗口外壳结构

- 根容器 `className="qq-main"` 对应 `theme.css` 中主窗实色背景定义。
- 左侧边栏通过 `topLevelEntries()` 从 `src/main-window/nav.ts` 获取三入口，**复用既有导航模块，未重造**。
- 中文标签映射用具名常量 `TOP_LEVEL_LABELS: Record<TopLevel, string>`，避免魔术字符串。
- 当前选中项用 `aria-current="page"` 表达选中态（语义化、可无障碍访问），未选中项不设该属性。
- `setState` 全部使用函数式更新 `setActiveTop((_prev) => entry)`，符合 React 规范。

### 2.3 占位区设计（供 S07-S09 替换）

三个占位 `<section>` 的稳定 `data-testid` 约定：

| data-testid | 对应一级页 | S07-S09 替换目标 |
|-------------|-----------|-----------------|
| `page-clipboard` | 剪贴板 | S07 ClipboardPage 组件 |
| `page-translate` | 翻译 | S08 TranslatePage 组件 |
| `page-settings` | 设置 | S09 SettingsPage 组件 |

可见性控制：`display: activeTop === "xxx" ? "block" : "none"`（CSS in-line，S07-S09 替换时可改为组件级控制）。

### 2.4 route 事件集成（B 阶段）

保留原 App.tsx 的 `listen<RoutePayload>("route", ...)` 逻辑与 `cancelled` 防泄漏 flag。
热键路由映射：`"history"` → `activeTop = "clipboard"`，`"translate"` → `activeTop = "translate"`，由 `routeToTopLevel()` 纯函数封装。

### 2.5 Tauri 窗口形态变更（重要，用户需知悉）

**原配置（预热弹窗形态）：** width 400, height 600, resizable: false, decorations: false

**新配置（主窗口形态）：** width 960, height 640, resizable: true, decorations: true

变更理由：原 400×600 无标题栏/无边框是预热弹窗的临时形态，不适合带左侧边栏的三页主窗口。960×640 为标准桌面内容窗口尺寸，decorations: true 提供原生标题栏与窗口控制按钮。

**保留 `visible: false`**：主窗口仍由托盘点击唤起，不在应用启动时自动弹出，行为与原来一致。

**§8 选中即译浮窗**属另一独立 webview 窗口，是未来项，本版不处理。

---

## 3. 三页占位 data-testid 约定（供 S07-S09 使用）

```
page-clipboard   → src/panels/clipboard/ClipboardPage（S07）
page-translate   → src/panels/translate/TranslatePage（S08）
page-settings    → src/main-window/settings/SettingsPage（S09）
```

S07-S09 实现时：将对应 `<section data-testid="page-xxx">` 的内容替换为真实页组件，保留 `data-testid` 和 `display` 控制逻辑（或由父组件传 `isActive` prop 控制）。

---

## 4. 测试证据

### app-shell 命中测试（冻结 verify：`pnpm test app-shell`）

```
✓ app-shell: 左侧边栏渲染三个一级入口（剪贴板/翻译/设置）
✓ app-shell: 默认激活剪贴板页，page-clipboard 可见、page-translate 不可见
✓ app-shell: 点击翻译后 page-translate 变为可见、page-clipboard 隐藏
✓ app-shell: 点击设置后 page-settings 变为可见
✓ app-shell: 当前选中项有 aria-current 属性，默认选中剪贴板
✓ app-shell: 点击翻译后翻译入口获得 aria-current、剪贴板入口失去

Tests  6 passed (6)
```

### 全量回归（`pnpm test`）

```
Test Files  14 passed (14)
Tests  112 passed (112)
```

原 106 个纯逻辑测试全绿，jsdom 环境引入无破坏。

---

## 5. 假设 / 未决 / 需用户确认

- **A10 视觉还原**：占位区无真实 UI，三页视觉还原（A10）待 S07-S09 实现后人工确认。
- **§8 选中即译浮窗**：设计文档§8 提及的选中即译浮窗为独立 webview，本版（S06-S09）不处理，标记为 pending-manual。
- **窗口形态变更**：已从 400×600 无边框改为 960×640 带装饰（见 §2.5），用户需确认此尺寸与 macOS 托盘唤起体验符合预期。
- **Tauri dev 模式**：开发时 `pnpm tauri dev` 会以新尺寸启动主窗口；若用户之前依赖 400×600 的弹窗形态测试，需知悉此变更。

---

## 6. code-standards 自检

| 规范项 | 状态 | 说明 |
|--------|------|------|
| 格式：2 空格缩进、无 Tab | 通过 | 所有新文件均 2 空格 |
| 函数 ≤50 行、嵌套 ≤3 层 | 通过 | App 函数约 60 行（含 JSX），单职责；routeToTopLevel 4 行 |
| 命名：camelCase / PascalCase | 通过 | TOP_LEVEL_LABELS（常量 UPPER_SNAKE）、routeToTopLevel（动词+名词） |
| 禁 any | 通过 | 无 any，RoutePayload / TopLevel 均显式类型 |
| setState 函数式更新 | 通过 | `setActiveTop((_prev) => entry)` |
| 注释写「为什么」 | 通过 | cancelled flag 防泄漏说明、窗口形态变更理由 |
| 无装饰性分隔注释 | 通过 | 无 `═══`/`───` 等横线 |
| 测试 AAA 结构 | 通过 | 所有测试有 Arrange/Act/Assert 注释 |
| 测试断言非恒真 | 通过 | 断言具体 DOM 状态（toBeVisible/not.toBeVisible/toHaveAttribute） |
| 无 TODO/FIXME | 通过 | grep 确认无残留 |
| 安全：无密钥入库 | N/A | 本功能无敏感数据 |
| 提交规范 Conventional Commits | 待提交 | 留给 coder 提交阶段 |
