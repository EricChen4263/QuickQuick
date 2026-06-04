---
id: V5-F6-S03-interaction-review
type: review
level: 小功能
parent: V5-F6-S03
children: []
created: 2026-06-03T05:30:00Z
status: 通过
commit: e838919
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 翻译源卡片交互解耦（V5-F6-S03 变体 A）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src/panels/settings/TranslateSourcePanel.tsx` | diff | ProviderCard 拆热区 + handleSelect/handleToggleConfigure 解耦 + isMounted 修复 |
| `src/panels/settings/settings.css` | diff | 新增 .src-select / .src-radio / .src-cfg-btn 等规则 |
| `src/panels/settings/TranslateSourcePanel.test.tsx` | 新文件 | 解耦行为全覆盖测试（7 条） |

参照：项目解耦设计意图（handleSelect 只设默认不展开）、TypeScript/React 规范（无 any / 函数≤50行 / 嵌套≤3层）、code-standards、tokens.css 变量约束。

---

## Critical 问题

### `handleSelect` 内遗留 `setExpandedId(id)` 破坏解耦（置信度 98）

**文件**：`src/panels/settings/TranslateSourcePanel.tsx`，第 202 行

**问题**：

```
async function handleSelect(id: string) {
  try {
    await setSelectedProvider(id);
    setSelectedId(id);
    setOpError(null);
    setExpandedId(id);   // <-- 与解耦设计直接矛盾
  } catch {
    ...
  }
}
```

`handleSelect` 在成功后执行 `setExpandedId(id)`，将被选中 provider 的 id 写入 `expandedId`。由于 `CredentialForm` 的条件渲染是 `expandedId === provider.id`（L253），点击「设默认」热区 `.src-select` 或 radio 会同时展开该 provider 的凭据表单。

这与本次改动的核心目标「handleSelect 只设默认无展开」完全矛盾，且直接导致以下测试失败：

- `TranslateSourcePanel.test.tsx` L86–103：「点击 radio → 不展开 CredentialForm」，断言 `queryByLabelText("App ID")` 不在文档中，但实际因 `setExpandedId("baidu")` 表单会渲染，断言失败。
- `TranslateSourcePanel.test.tsx` L105–123：「点击 .src-select → 不展开表单」，同样断言 `queryByLabelText("App ID")` 不在文档中，同样失败。

附加副作用：选中无需 Key 的 MyMemory 时，`setExpandedId("mymemory")` 虽不渲染 CredentialForm（因无 `needsKey`），但若此前百度表单已展开，切换选中 MyMemory 会关闭百度表单（`expandedId` 不再等于 `"baidu"`）——这个收起副作用是偶然的，依赖状态污染而非设计意图，随后若再点击选中百度，表单会再次打开，形成非预期的「选中即展开」行为。

**修复**：删除 `handleSelect` 第 202 行的 `setExpandedId(id)`，仅保留三行：

```typescript
async function handleSelect(id: string) {
  try {
    await setSelectedProvider(id);
    setSelectedId(id);
    setOpError(null);
  } catch {
    setOpError("切换翻译源失败，请稍后重试");
  }
}
```

---

## 通过项

### 解耦结构（除上述 bug 外）

- `handleToggleConfigure`（L208-209）：纯 toggle `expandedId`，与 `handleSelect` 路径完全隔离，结构正确。
- `onConfigure` 只绑定到 `provider.needsKey` 条件渲染的 `.src-cfg-btn`（L91-101），MyMemory 无配置按钮，符合设计。
- 渲染层分离正确：`.src-select` onClick → `handleSelect`；`.src-cfg-btn` onClick → `handleToggleConfigure`，两个事件源互不干扰（bug 在 handler 内部，不在事件绑定层）。

### isMounted 修复（明确判定：通过）

`handleCredentialSaved` 使用组件级 `isMounted = useRef(true)`（L115），对应 `useEffect` 在挂载时设 `true`、cleanup 设 `false`（L117-122）。`.then` 守卫 `if (!isMounted.current) return`（L213），正确防止卸载后 setState。相比前一版本的无效 `cancelled` 局部变量，此修复有效。**通过。**

### CSS 变量存在性（明确判定：全部通过）

settings.css 新增规则引用的所有 CSS 变量逐一核对 `src/theme/tokens.css`：

| 变量 | tokens.css 存在 |
|---|---|
| `--hover` | 是（L17 color-mix 定义） |
| `--accent` | 是（L12） |
| `--accent-line` | 是（L16 color-mix 定义） |
| `--accent-soft` | 是（L14 color-mix 定义） |
| `--border` | 是（L11） |
| `--muted` | 是（L10） |
| `--fg` | 是（L9） |
| `--r-sm` | 是（L27） |
| `--surface-2` | 是（L16） |

无硬编码颜色，全部使用 token，**CSS 变量存在性通过。**

### a11y（明确判定：通过）

- `sr-only` radio 保留（L85），`getByRole("radio", { name })` 可命中，无障碍树完整。
- `.src-cfg-btn` 有 `aria-label={配置 ${provider.name}}`（L96），屏幕阅读器可区分多个配置按钮。
- `GearIcon` 有 `aria-hidden="true"`（L32），不污染无障碍树。**通过。**

### 无 any / 函数行数 / 嵌套（明确判定：通过）

- 所有新增状态和函数参数均有明确类型，无 `any`。
- `handleSelect`（6 行）、`handleToggleConfigure`（1 行）、`handleCredentialSaved`（18 行，含 setState 回调）均在 50 行以内。
- 嵌套最深处（`handleCredentialSaved` 的 `.then` → `setConfiguredIds` 回调 → `if`）为 3 层，符合 ≤3 规范。**通过。**

### 测试结构（除被 bug 破坏的用例外，结构设计正确）

- 测试文件 describe 分组清晰：徽标显示 / 设为默认（解耦）/ 配置按钮（解耦），AAA 结构，行为化命名。
- 「配置按钮」组的 5 条用例不依赖 `handleSelect`，不受上述 bug 影响，逻辑独立正确。
- 测试用例 L86–123 的断言设计本身正确（正是这些断言暴露了 bug）。

---

## 问题清单

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| Critical | `handleSelect` 内 `setExpandedId(id)` 破坏解耦，导致「点击热区 → 展开表单」，与设计目标矛盾，测试 L86-103 / L105-123 必然失败 | `TranslateSourcePanel.tsx:202` | 删除该行；`handleSelect` 只调用 `setSelectedProvider` / `setSelectedId` / `setOpError` |

---

## 结论

**打回（必改 1 项）**

`handleSelect` 第 202 行 `setExpandedId(id)` 与本次改动的核心设计目标（选中不展开）直接矛盾，且导致测试 `TranslateSourcePanel.test.tsx` 中验证解耦行为的 2 条用例必然失败。必须删除该行后重新提交。

其余部分（`handleToggleConfigure` 解耦结构、isMounted 修复、CSS 变量全部存在、a11y、无 any、测试设计）均审查通过，无其他阻断项。

---

## 复审（commit e838919）

初审判定 **打回（1 Critical）**：`handleSelect` 内 `setExpandedId(id)` 破坏「选中不展开」解耦。该项已在同一 commit `e838919`（message：「多翻译源端到端接通——动态路由+凭据配置+**解耦交互**」）的提交态内修复，复审核实在已提交代码内：

| 项 | 初审问题 | 修复（已核实位置） |
|---|---|---|
| Critical | `handleSelect` 调 `setExpandedId(id)`，选中即展开，与解耦目标矛盾，2 条解耦测试必败 | `src/panels/settings/TranslateSourcePanel.tsx:197-205` `handleSelect` 仅 `setSelectedProvider`/`setSelectedId`/`setOpError`，无 `setExpandedId`；展开逻辑独立在 `handleToggleConfigure`（:208）。e838919 提交态已核实无该行（文件后由 translate/ 迁至 settings/，逻辑不变）。 |

终态：**通过**。
