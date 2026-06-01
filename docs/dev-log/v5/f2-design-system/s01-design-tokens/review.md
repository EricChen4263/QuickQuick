---
id: V5-F2-S01-review
type: review
level: 小功能
parent: V5-F2
children: []
created: 2026-06-01T15:00:00Z
status: 未过
commit: WIP
acceptance_ids: []
evidence: []
author: code-reviewer
---

# 审查结论 · 里程碑1：设计系统底座 + 应用外壳

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src/theme/tokens.css` | 新增 | OKLch token 短名 + compat alias 段 |
| `src/theme/base.css` | 新增 | CSS reset + 基础排版 |
| `src/theme/components.css` | 新增 | 外壳 / 通用组件样式（qq- 前缀） |
| `src/theme/theme.css` | 修改 | CSS 入口，@import 三文件 |
| `src/theme/themeStore.ts` | 新增 | 纯 TS 主题偏好单例 |
| `src/theme/useTheme.ts` | 新增 | React hook，订阅 themeStore |
| `src/shell/AppShell.tsx` | 新增 | 应用外壳布局组件 |
| `src/shell/SideBar.tsx` | 新增 | 主导航侧边栏 |
| `src/shell/ThemeSwitch.tsx` | 新增 | 三档主题切换按钮组 |
| `src/App.tsx` | 修改 | 接入 AppShell，路由层不变 |
| `index.html` | 修改 | 加 FOUC 防闪脚本 |
| `src/app-shell.test.tsx` | 修改 | 外壳集成测试 |
| `src/theme/themeStore.test.ts` | 新增 | themeStore 单元测试 |
| `src/theme/useTheme.test.ts` | 新增 | useTheme hook 测试 |
| `src/shell/ThemeSwitch.test.tsx` | 新增 | ThemeSwitch 组件测试 |

审查标准：code-standards skill（§1-§12）+ 项目前端规范（2空格/禁any/camelCase/PascalCase/useState函数式更新/注释写为什么/禁装饰性横线/禁死代码）+ 架构方案约定（themeStore 纯 TS 单例 / token 短名 + compat / CSS 三层拆分 / FOUC 脚本与 store 逻辑一致）。

---

## 发现问题（置信度 ≥ 80 才报）

### Critical

无。

### Important

#### I-01：`AppShell.tsx` 静态布局属性写成 inline style，应移入 `qq-main` CSS class

**文件：** `src/shell/AppShell.tsx:19`
**置信度：** 85

```tsx
// 现状：静态布局值写成 inline style
style={{ display: "grid", gridTemplateColumns: "92px 1fr", height: "100vh" }}
```

`display: grid`、`gridTemplateColumns: 92px 1fr`、`height: 100vh` 三个属性全是静态常量，与动态值无关。项目已存在 `qq-main` CSS class（`components.css:8`），其中只定义了 `background/color/font-family`，布局属性分散在 inline style 里，违反"设计系统 class-first"原则——class 只做颜色，布局靠 inline style，两处合才描述一个完整组件，后续维护需来回横跳。

**修复建议：** 将这三个属性移入 `components.css` 的 `.qq-main` 规则，组件只保留 `className="qq-main"`，无需 style prop。

---

#### I-02：`SideBar.tsx` spacer div 同时有 CSS class 和 inline style 重复定义 `flex: 1`

**文件：** `src/shell/SideBar.tsx:66`
**置信度：** 90

```tsx
// 现状：class 已有 flex:1，inline style 再写一遍
<div className="spacer" style={{ flex: 1 }} />
```

`components.css:57` 已定义 `.spacer { flex: 1; }`，inline `style={{ flex: 1 }}` 完全冗余，是典型 DRY 违规（code-standards §1 DRY / §2 格式）。

**修复建议：** 删除 inline style，只保留 `className="spacer"`。

---

#### I-03：`themeStore.ts` 中 `init()` 的 `document` 访问无 `typeof document` 守卫，与 `window` 守卫不对称

**文件：** `src/theme/themeStore.ts:55-65`
**置信度：** 82

```ts
function init(): void {
  pref = readPref(); // localStorage 无守卫

  if (typeof window !== "undefined" && window.matchMedia) {
    // matchMedia 有守卫
    mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    mediaQuery.addEventListener("change", handleMediaChange);
  }

  // ← 此处 document 访问无守卫，在纯 Node/SSR 环境下会抛 ReferenceError
  document.documentElement.dataset["theme"] = resolveTheme(pref);
}
```

文件顶部 JSDoc 注释和规范要求"SSR/jsdom 守卫"。`matchMedia` 有 `typeof window` 守卫，`document` 和 `localStorage` 访问却无对等守卫，形成不对称：任何在 Node.js 环境（如 SSR 渲染、Node 脚本）中 import 该模块都会立即崩溃。当前 Tauri 应用不走 SSR，测试跑在 jsdom（jsdom 模拟了 `document` 和 `localStorage`），所以实际上不会触发——但这是防御性缺失，且文件注释本身声明了 SSR 守卫意图。里程碑3迁移到 IPC 时该函数签名会被改动，届年添加守卫；但如果该模块被 Node 侧脚本误引用（如 vite.config / 构建时分析），当前实现会无声失败。

**修复建议：**
```ts
function init(): void {
  if (typeof window === "undefined") return; // SSR / Node 守卫

  pref = readPref();
  if (window.matchMedia) {
    mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    mediaQuery.addEventListener("change", handleMediaChange);
  }
  document.documentElement.dataset["theme"] = resolveTheme(pref);
}
```

---

## compat alias 覆盖核对

### 现有三页实际用到的 `--qq-*` 变量（grep 全 src/panels 非测试文件）

| 变量名 | 用途来源 |
|---|---|
| `--qq-accent` | ClipItemRow、SectionNav |
| `--qq-accent-bg` | SectionNav（`rgba(58,124,165,0.12)` fallback） |
| `--qq-border` | SectionNav、TranslateHistoryPanel、ClipPreview、ClipItemRow |
| `--qq-danger` | HotkeyPanel、PrivacyPanel、TranslateSourcePanel、TranslatePage、ClipboardPage |
| `--qq-font` | SettingsPage、HotkeyPanel、PrivacyPanel、TranslateSourcePanel、TranslatePage、TranslateWorkspace、ClipboardPage |
| `--qq-radius-md` | ClipItemRow |
| `--qq-surface` | TranslatePage、TranslateHistoryPanel、ClipItemRow、ClipboardPage |
| `--qq-text` | ClipPreview、SectionNav |
| `--qq-text-muted` | SettingsPage、PrivacyPanel、TranslateHistoryPanel、TranslateWorkspace、ClipboardPage |

### `tokens.css` compat 段定义的变量

```css
--qq-bg         → var(--bg)
--qq-surface    → var(--surface)
--qq-text       → var(--fg)
--qq-text-muted → var(--muted)
--qq-border     → var(--border)
--qq-accent     → var(--accent)
--qq-accent-bg  → var(--accent-soft)
--qq-hover-bg   → var(--hover)
--qq-radius-md  → var(--r)
--qq-font       → var(--font)
--qq-danger     → var(--danger)
```

### 核对结论

**覆盖完整，无遗漏。** 现有三页用到的全部 9 个 `--qq-*` 变量均在 compat 段中有对应别名。

额外说明：
- `--qq-bg` 在 compat 中有定义，panels 代码中未直接使用（`design-tokens.ts` 的 `themeToCssVars` 输出中有，但 panels 未调用该函数），不影响正确性。
- `--qq-hover-bg` 定义了别名，panels 中亦未使用，属于"防御性预留"，无害。

---

## 逐项规范核查

| 规范项 | 结论 | 说明 |
|---|---|---|
| 禁 `any` | 通过 | 全部 5 个 TS/TSX 文件均无 `any`；`catch (err: unknown)` 用 `unknown` |
| 2 空格缩进 | 通过 | 全部文件 2 空格，无 Tab |
| camelCase / PascalCase | 通过 | 变量 `camelCase`，组件 `PascalCase`，CSS 变量短名 `--bg/--fg` 与 compat `--qq-*` 均符合 |
| 禁装饰性横线注释 | 通过 | 全部 TS/TSX/CSS 文件无 `===`/`---`/`━━━` 等横线分隔 |
| 函数 ≤ 50 行 | 通过 | 最长 `_reset()` ~10 行，`init()` ~10 行；组件函数均 ≤ 30 行 |
| 嵌套 ≤ 3 层 | 通过 | themeStore 内部函数最深 2 层；组件 JSX 最深 3 层 |
| setState 函数式更新 | 基本通过 | `setActiveTop((_prev) => top)` 和 `(_prev) => routeToTopLevel(...)` 均用函数式；`setLocalPref(getPref())` 直接赋来自外部 store 的值（非基于 prev 计算），语义合理，不属于违规 |
| useEffect cleanup | 通过 | `useTheme` 返回 unsub；`App.tsx` route 监听用 cancelled flag + unlisten；keydown listener 有移除 |
| 注释写「为什么」 | 通过 | themeStore 顶部 JSDoc 说明 4 条设计决策；`init()` 注释说明"初始写入不触发 listeners 原因"；index.html FOUC 脚本有完整说明 |
| 无死代码 | 通过 | 无注释掉的代码；无未使用导出（tsc 严格模式通过） |
| 公共 API 文档注释 | 通过 | `getPref`/`getResolved`/`setPref`/`subscribe` 均在文件顶部 JSDoc 覆盖；`useTheme` hook 有 JSDoc |
| 无魔术值 | 通过 | `STORAGE_KEY = "qq-theme-pref"` 具名常量；CSS token 值有注释 |
| 禁 TODO/FIXME | 通过 | grep 确认全部文件无残留 |
| TypeScript strict | 通过 | `tsc --noEmit` 零错误；`ThemePref`/`ResolvedTheme`/`Listener` 类型均显式声明 |
| 安全 | N/A | 无密钥、无敏感数据 |

---

## 架构落地核查

### themeStore 单例正确性

- **纯 TS 无 React 依赖**：通过。仅 import TS 类型，无 React import，满足里程碑4 popora 复用的前提。
- **subscribe/unsubscribe 无泄漏**：通过。`subscribe()` 返回 `() => listeners.delete(listener)`；`useTheme` 在 `useEffect` cleanup 中调用返回的 unsub；`ThemeSwitch` 通过 `useTheme` 间接管理，均无泄漏路径。
- **dataset.theme 写入唯一集中**：通过。仅 `applyResolved()` 和 `init()` 两处操作 `dataset.theme`，均在 `themeStore.ts` 内，组件层不直接写。
- **matchMedia 监听清理**：通过。`_reset()` 中 `mediaQuery.removeEventListener("change", handleMediaChange)` 正确清理；生产代码的模块单例随页面销毁，无需额外清理。

### FOUC 脚本与 themeStore 逻辑一致性

index.html 内联脚本：
```js
var pref = localStorage.getItem("qq-theme-pref") || "auto";
if (pref === "light" || pref === "dark") {
  resolved = pref;
} else {
  resolved = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
document.documentElement.dataset.theme = resolved;
```

themeStore `resolveTheme`：
```ts
if (currentPref === "light") return "light";
if (currentPref === "dark") return "dark";
return mediaQuery?.matches ? "dark" : "light";
```

**一致性：通过。** localStorage key（`"qq-theme-pref"`）、auto 分支逻辑（matchMedia dark 判断）、回退值（light）三项完全一致。FOUC 防护有效。

### CSS @import 顺序

`theme.css`：`tokens.css` → `base.css` → `components.css`，tokens 最先，依赖关系正确。

### 可访问性

- `nav aria-label="主导航"` 通过
- `aria-current={activeTop === entry.key ? "page" : undefined}` 通过（undefined 时不渲染属性，符合 ARIA spec）
- `ThemeSwitch` 按钮有 `title`（可访问名）+ `aria-pressed`，`role="group" aria-label="外观"` 通过
- 导航 button 无需 `type="button"`（浏览器默认 type=submit 在 form 外无影响，但显式声明更清晰，属低优可选改进）

---

## 中低优先级 / 可选改进（不作为打回条件）

1. **`SideBar.tsx` 导航 button 建议显式 `type="button"`**（置信度 65）：虽然 form 外无 submit 影响，但显式声明是防御性好习惯，避免将来被放入 form 时行为改变。
2. **`AppShell` `<main>` 无 `aria-label`**（置信度 55）：页面有多个地标时建议标记 main 内容区域，当前单 main 无歧义，可选。
3. **`tokens.css` 毛玻璃 token `rgba()` 写法**（置信度 40）：`--glass: rgba(255, 255, 255, 0.72)` 混用了 `rgba()` 与其他 `oklch()` 写法，不影响正确性，风格略不统一，属纯 nitpick。

---

## 必改项总结

| 编号 | 文件 | 问题 | 改动量 |
|---|---|---|---|
| **M1** | `src/shell/AppShell.tsx:19` + `src/theme/components.css:8-12` | 将 `display:grid/gridTemplateColumns/height:100vh` 从 inline style 移入 `.qq-main` CSS class | ~5 行迁移 |
| **M2** | `src/shell/SideBar.tsx:66` | 删除 spacer div 的 `style={{ flex: 1 }}`，CSS class 已有定义 | 删 1 项 |
| **M3** | `src/theme/themeStore.ts:55-65` | 在 `init()` 函数首行加 `if (typeof window === "undefined") return` 守卫，保持与 matchMedia 守卫对称 | +1 行 |

---

## 结论

**未过——3 项 Important 需修复后方可通过。**

代码整体质量较高：类型系统严格（tsc 零错误）、禁 any 彻底、无死代码、FOUC 脚本与 store 逻辑完全一致、compat alias 覆盖完整（9/9 无遗漏）、subscribe/unsubscribe 无泄漏、测试结构清晰（AAA）。3 项 Important 均为小改动（inline→class 迁移 + 1行删除 + 1行守卫），改完后可直接通过，无需重新完整审查，复审只核对三项修复点即可。
