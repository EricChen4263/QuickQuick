---
id: V4-F3-S11-review
type: review
level: 小功能
parent: V4-F3
children: []
created: 2026-05-31T16:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V4-F3-A12]
evidence: []
author: code-reviewer
---

# 审查结论 · S11 设计语言 Token 落地

## 审查维度

项目规范（§9.1 设计语言：品牌主色 #3A7CA5 / 中圆角 10px / 深浅双主题随系统 / 主窗实色 / 弹窗毛玻璃）
+ code-standards（前端 TS/CSS：禁 any / 命名 / 函数 ≤ 50 行 / 嵌套 ≤ 3 层 / 颜色常量不散落 / CSS 变量前缀统一 / 无装饰性分隔注释 / 注释写为什么）
+ 关键互通一致性：themeToCssVars 与 theme.css 变量名逐一比对（--qq- 系列，共 8 个）
+ macOS WebKit 兼容：-webkit-backdrop-filter 存在性核查

## 发现问题（置信度 ≥ 80 才报）

无。经全项核查未发现置信度 ≥ 80 的真实问题。

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| — | 无高置信问题 | — | — |

### 低置信观察项（不构成打回条件，仅供参考）

- **darkTheme.accent 对比度（观察，非门禁）**：`#5B9FC4` 在深色 `#141517` 背景上对比度约 4.6:1（WCAG AA），coding 留痕已记录。若 UX 人工审查后认为对比不足，修改 `darkTheme.accent` 一处即生效；本轮不作硬门禁要求。置信度 < 80，不报。

- **theme.css 中 `#3A7CA5` 魔法值（CSS 层不可引用 TS 常量，可接受）**：CSS `:root` 直接写品牌色十六进制属已知限制，非规范违反。若后续引入 CSS 预处理器或 CSS 自定义属性级联可消除，但当前纯 CSS 方案无法规避，置信度不足，不报。

## 逐项核查记录

### token 值忠于 §9.1

| 检查项 | 期望 | 实际 | 结论 |
|---|---|---|---|
| 品牌主色 | `#3A7CA5` | `BRAND_FJORD_TEAL = "#3A7CA5"` | 通过 |
| 中圆角 | `10px` | `RADIUS_MD = "10px"` | 通过 |
| 主窗实色（浅） | 不透明实色 | `bg: "#F5F6F7"` | 通过 |
| 主窗实色（深） | 不透明实色 | `bg: "#141517"` | 通过 |
| 明暗有别 | `lightTheme.bg !== darkTheme.bg` | 确认不同 | 通过 |
| 浅色 accent 用品牌色 | `=== BRAND_FJORD_TEAL` | `accent: BRAND_FJORD_TEAL` | 通过 |
| 深色 accent 可读变体 | hex 格式 + 非空 | `"#5B9FC4"` | 通过 |

### themeToCssVars 与 theme.css 变量名一致性（逐一比对）

| TS 映射键 | CSS :root 变量 | 一致 |
|---|---|---|
| `--qq-bg` | `--qq-bg` | 是 |
| `--qq-surface` | `--qq-surface` | 是 |
| `--qq-text` | `--qq-text` | 是 |
| `--qq-text-muted` | `--qq-text-muted` | 是 |
| `--qq-border` | `--qq-border` | 是 |
| `--qq-accent` | `--qq-accent` | 是 |
| `--qq-radius-md` | `--qq-radius-md` | 是 |
| `--qq-font` | `--qq-font` | 是 |

全部一致，F2 三页 UI 取用不会错位。

### 规范符合性

| 规范项 | 结论 |
|---|---|
| 禁 any | 通过（ThemeTokens + Record<string, string>，无 any） |
| camelCase / PascalCase | 通过 |
| 函数 ≤ 50 行 | 通过（themeToCssVars 10 行） |
| 嵌套 ≤ 3 层 | 通过 |
| 颜色常量不散落（TS 内） | 通过（accent 引用 BRAND_FJORD_TEAL，其余色值集中在 lightTheme/darkTheme 对象） |
| CSS 变量前缀统一 --qq- | 通过 |
| 毛玻璃 -webkit-backdrop-filter | 通过（theme.css L71） |
| 无装饰性分隔注释 | 通过 |
| 无 TODO/FIXME | 通过 |
| 测试断言非恒真 | 通过（.toBe 具体值 + .not.toBe + readFileSync grep 实证） |

### macOS WebKit 兼容

`theme.css` 第 71 行：`-webkit-backdrop-filter: blur(20px) saturate(1.8)` 存在，与标准属性并列声明。符合 macOS WebKit 必需前缀要求。

## 是否合规

完全符合项目规范与 code-standards。token 值与设计文档 §9.1 逐点对齐，CSS 变量名与 TS 映射无任何错位，F2 后续三页 UI 可直接取用。

## 对 F2 三页 UI 的注意事项

### 可用 CSS 变量清单（S11 输出契约）

```
--qq-bg          主窗背景实色（浅 #F5F6F7 / 深 #141517）
--qq-surface     卡片/浮层背景（浅 #FFFFFF / 深 #1E2023）
--qq-text        主文字色（浅 #1A1A1A / 深 #F0F1F3）
--qq-text-muted  次要/辅助文字色（浅 #6B7280 / 深 #9CA3AF）
--qq-border      细描边色（浅 #E2E4E8 / 深 #2D3038）
--qq-accent      品牌强调色（浅 #3A7CA5 / 深 #5B9FC4）
--qq-radius-md   中圆角（10px，固定）
--qq-font        系统原生字体栈（固定）
```

明暗切换由 `theme.css` 的 `@media (prefers-color-scheme: dark)` 纯 CSS 处理，F2 组件无需 JS 感知主题；只需引入 `theme.css` 后使用上述变量即可随系统自动切换。

### 如何在 F2 页面应用 .qq-main 与 .qq-popover

**主窗口根元素**：为 App 外壳根容器（如 `<div id="root">` 或最外层布局 `<div>`）添加类 `.qq-main`：

```tsx
// src/App.tsx 或 AppShell 组件
<div className="qq-main" style={{ height: "100vh" }}>
  {/* 左侧边栏 + 内容页 */}
</div>
```

效果：自动挂载 `background-color: var(--qq-bg)`（实色）、`font-family: var(--qq-font)`、`color: var(--qq-text)`。

**弹窗/浮层**（如 tooltip、context menu、下拉）：为弹窗容器添加类 `.qq-popover`：

```tsx
// 例：类型筛选下拉、翻译历史浮层
<div className="qq-popover">
  {/* 弹窗内容 */}
</div>
```

效果：自动挂载毛玻璃材质（`backdrop-filter: blur(20px) saturate(1.8)`、半透明白/暗背景），深色下自动切换为深色毛玻璃。注意弹窗容器需有 `position: absolute/fixed` 才能触发 backdrop-filter 效果，且父元素不得设置 `transform`（WebKit 限制：transform 会新建 stacking context 使 backdrop-filter 失效）。

**JS 注入（可选，用于动态主题或需要内联 style 时）**：

```tsx
import { themeToCssVars, lightTheme, darkTheme } from "../theme/design-tokens";

// 注入到 document.documentElement（与 CSS @media 方案互补，按需选用）
const theme = prefersDark ? darkTheme : lightTheme;
Object.entries(themeToCssVars(theme)).forEach(([key, value]) => {
  document.documentElement.style.setProperty(key, value);
});
```

通常推荐纯 CSS `@media` 方案（无需 JS）；`themeToCssVars` 留给需要运行时动态覆盖主题的场景。

## 结论

通过。未发现任何置信度 ≥ 80 的问题。S11 输出的 8 个 `--qq-*` CSS 变量契约与 TS 映射完全一致，§9.1 设计规格逐点对齐，F2 三页 UI 可直接取用。
