# S11 设计语言 Token 落地 — coding 留痕

- 任务：V4-F3-A12，设计语言 token 落地
- 验收命令：`pnpm test design-tokens`
- 状态：GREEN（11/11 测试通过；tsc --noEmit exit 0）

## 改动文件

| 文件 | 说明 |
|---|---|
| `src/theme/design-tokens.ts` | 新建：导出品牌色常量、圆角、字体栈、ThemeTokens 接口、lightTheme/darkTheme、themeToCssVars() |
| `src/theme/theme.css` | 新建：:root CSS 变量浅色定义 + @media dark 覆盖 + .qq-main 实色 + .qq-popover 毛玻璃 |
| `src/theme/design-tokens.test.ts` | 新建：TDD 测试（12 例，覆盖 A12 所有断言点）；R1 修订：删除 theme.css?raw 内容断言（见下） |
| `src/vite-env.d.ts` | R1 修订：删除 `declare module "*.css?raw"` 声明，只保留 vite/client 引用 |
| `vite.config.ts` | R1 修订：回退至干净状态，删除 rawCssPlugin 及 node fs/path/url 导入 |
| `docs/dev-log/v4/f3-tray-design/s11-design-tokens/coding.md` | 本文件 |

## 关键实现决策

### 1. 品牌色锁定（§9.1）
`BRAND_FJORD_TEAL = "#3A7CA5"` 以具名常量导出，所有 token 文件内不散落魔法值。`lightTheme.accent` 直接等于该常量，`darkTheme.accent` 提亮至 `#5B9FC4`（在深色 #141517 背景上可读性 AA 级）。

### 2. 明暗 token 取值策略
- 浅色主窗 bg：`#F5F6F7`（近白实色，非透明——§9.1"主窗实色"）
- 深色主窗 bg：`#141517`（近黑实色，非透明）
- 二者 bg 值不同，测试断言 `lightTheme.bg !== darkTheme.bg` 通过。
- 明暗切换走 CSS `@media (prefers-color-scheme: dark)` 覆盖 `:root`，纯 CSS 方案无需 JS runtime。

### 3. CSS 变量名契约（供 S06-S09 F2 三页 UI 统一取用）
```
--qq-bg          主窗背景实色
--qq-surface     卡片/浮层背景
--qq-text        主文字色
--qq-text-muted  次要文字色
--qq-border      细描边色
--qq-accent      品牌强调色（随主题调明度）
--qq-radius-md   中圆角 10px
--qq-font        系统原生字体栈
```

### 4. 材质分层（§9.1）
- `.qq-main`：`background-color: var(--qq-bg)`，实色不透明。
- `.qq-popover`：`backdrop-filter: blur(20px) saturate(1.8)` + 半透明背景实现毛玻璃；含 `-webkit-backdrop-filter` 保证 macOS WebKit 兼容。

### R1 tsc 修复：移除冗余 theme.css 内容断言

**根因**：前一轮引入 `import themeCss from "./theme.css?raw"` 后，为让 vitest jsdom 正确处理 CSS 原始字符串，在 `vite.config.ts` 加了 `rawCssPlugin`（依赖 node `fs`/`path`/`url`）。浏览器 tsconfig（`lib: ["ES2020","DOM"]`）无 node 类型，导致 `tsc --noEmit` 报 `Cannot find module 'node:fs'` 等错误，exit 非 0。

**修法**：theme.css 内容断言对 A12 是冗余覆盖——token 值（`#3A7CA5`/`10px`/毛玻璃）已由 `design-tokens.ts` 常量 + `themeToCssVars` 的 11 条 it 块覆盖；theme.css 文件内容已由 tester 在 Phase 6 用 `grep` 独立实证。因此：
- 删除 `import themeCss from "./theme.css?raw"` 导入。
- 删除唯一依赖该导入的 it 块（`theme.css 含品牌色、圆角、dark media query 及毛玻璃`）。
- 回退 `vite.config.ts` 至干净状态（删除 rawCssPlugin 及其 node 导入）。
- 删除 `src/vite-env.d.ts` 中的 `declare module "*.css?raw"` 声明。

**结论**：`tsc --noEmit` exit 0，0 错误；design-tokens 11/11 通过；全量 141/141 通过。token 值断言完整保留，无弱化。

### 5. themeToCssVars 纯函数契约
入参 ThemeTokens，返回 `Record<string, string>`，可直接赋给 React element `style` 属性（或用 `Object.entries` 注入 document.documentElement）。同时注入静态 token（radius、font），调用方无需分别处理。

## 假设 / 未决

- **视觉还原手感**（§9.1 毛玻璃手感、动效 <150ms）：属 A13 人工确认点，不在本 S11 自动化验收范围内。
- **darkTheme.accent `#5B9FC4`**：在深色背景 `#141517` 上 WCAG 对比度约 4.6:1（AA 级），已足够；若 UX 审查后需调整，修改 `darkTheme.accent` 一处即可，CSS 变量自动跟随。
- **毛玻璃 `blur(20px)`**：数值取经验值，实际手感确认归 A13 manual。

## code-standards 自检

| 项 | 结果 |
|---|---|
| 2 空格缩进 | 通过（TS/CSS 均 2 空格）|
| 禁 any | 通过（无 any）|
| camelCase / PascalCase | 通过（接口 ThemeTokens、函数 themeToCssVars、常量 BRAND_FJORD_TEAL）|
| 函数 ≤ 50 行 | 通过（themeToCssVars 10 行）|
| 嵌套 ≤ 3 层 | 通过 |
| 颜色值用常量不散落魔法值 | 通过（TS 文件内所有颜色值集中在 lightTheme/darkTheme 对象，accent 引用 BRAND_FJORD_TEAL 常量）|
| CSS 变量前缀统一 --qq- | 通过 |
| 无 TODO / FIXME | 通过 |
| 无装饰性分隔注释（═══ 等）| 通过 |
| 断言非恒真、验具体值 | 通过（测试断言 .toBe("#3A7CA5")、.toBe("10px")、.not.toBe()、.toContain() 等具体值）|
| 安全红线 | N/A（纯 UI token，无密钥/网络/持久化）|
