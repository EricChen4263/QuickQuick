---
id: V5-F2-S06-review
type: review
level: 小功能
parent: V5-F2
children: []
created: 2026-06-01T17:30:00Z
status: 未过
commit: WIP
acceptance_ids: []
evidence: []
author: code-reviewer
---

# 审查结论 · 里程碑2：主窗三页视觉重塑

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src/panels/clipboard/ClipboardPage.tsx` | 改造 | 双栏布局重塑，引入 OnboardCard |
| `src/panels/clipboard/ClipItemRow.tsx` | 改造 | 设计系统类名，aria-hidden 图标 |
| `src/panels/clipboard/ClipPreview.tsx` | 改造 | EmptyState 复用，图片原图加载 |
| `src/panels/clipboard/ClipSearchBar.tsx` | 改造 | 设计系统类名 |
| `src/panels/clipboard/OnboardCard.tsx` | 新增 | 首次运行辅助功能权限引导卡片 |
| `src/panels/clipboard/clipboard.css` | 重写 | 短名 token 全替换 |
| `src/panels/translate/TranslatePage.tsx` | 改造 | 翻译工作区/历史栏协调层 |
| `src/panels/translate/TranslateWorkspace.tsx` | 改造 | 纯展示组件重塑 |
| `src/panels/translate/TranslateHistoryPanel.tsx` | 改造 | 历史侧栏 EmptyState 复用 |
| `src/panels/translate/DirBar.tsx` | 新增 | 语言方向栏抽取 |
| `src/panels/translate/translate.css` | 重写 | 短名 token 全替换 |
| `src/panels/settings/SettingsPage.tsx` | 改造 | 六子项路由 + GeneralPanel/StoragePanel/AboutPanel 内联 |
| `src/panels/settings/SectionNav.tsx` | 改造 | aria-current 驱动选中态 |
| `src/panels/settings/HotkeyPanel.tsx` | 改造 | HotkeyRow 子组件 + 占位 SettingToggle |
| `src/panels/settings/TranslateSourcePanel.tsx` | 改造 | ProviderCard + sr-only radio + badge 三态 |
| `src/panels/settings/PrivacyPanel.tsx` | 改造 | chip-row 排除名单 + 占位 toggle |
| `src/panels/settings/PanelHeader.tsx` | 新增 | 面板大标题区抽取 |
| `src/panels/settings/SettingGroup.tsx` | 新增 | 带边框分组容器 |
| `src/panels/settings/SettingRow.tsx` | 新增 | 标签+控件行 |
| `src/panels/settings/SettingToggle.tsx` | 新增 | ARIA switch button 封装 |
| `src/panels/settings/useGeneralSettings.ts` | 新增 | 通用设置本地 state hook，里程碑3接入点 |
| `src/panels/settings/settings.css` | 改造 | 迁入通用组件类 + D2 专属类 |
| `src/components/EmptyState.tsx` | 新增 | 通用空态组件 |
| `src/theme/tokens.css` | 改造 | 删除 compat 别名段 |
| `src/theme/components.css` | 改造 | 追加 .sr-only + 注释迁移说明 |
| `src/shell/SideBar.tsx` | 改造 | 套用设计系统侧边栏类名 |
| 测试：app-shell/translate-page/settings-page/*.test | 改造/新增 | +3 新测试覆盖 D2 结构 |

---

## 一、高优先级问题（置信度 ≥ 80，需修改）

### 1. [ARIA 违规·高] TranslateHistoryPanel：`button[role="option"]` 无 listbox 父容器

**文件**：`src/panels/translate/TranslateHistoryPanel.tsx:47-54`

**问题**：`role="option"` 元素在 ARIA 规范中要求必须是 `role="listbox"`（或 `combobox`/`grid`）的直接拥有子项（owned child）。当前 `.tx-hist-list` 是一个无语义 `<div>`，不具备 listbox 角色，导致 `role="option"` 孤立存在——屏幕阅读器无法正确识别选项之间的关系，ARIA 验证工具（如 axe-core）会报 `required-context` 错误。

**补充**：`aria-selected="false"` 被硬编码为 `"false"` 字符串，即使历史条目被"回填"选中逻辑也不会更新，与实际选中状态脱节。

**建议**：方案 A（推荐）——将 `.tx-hist-list` 改为 `<div role="listbox" aria-label="翻译历史列表">`，历史条目仍为 `button[role="option"]`；`aria-selected` 改为接受并追踪当前选中 id 的动态绑定。方案 B——将历史条目改为普通按钮（去掉 `role="option"`），`aria-label` 保留，以朴素语义表达可点击的历史项——更简单，无需 listbox 父容器，且行为意图更接近"按钮列表"而非"选择框"。

**置信度**：90

---

### 2. [内联样式未迁移·中高] HotkeyPanel 和 ClipboardPage 的关键布局仍用 inline style

**文件**：
- `src/panels/settings/HotkeyPanel.tsx:59-60,78,83`
- `src/panels/clipboard/ClipboardPage.tsx:154,157-159`

**问题（HotkeyPanel）**：`HotkeyRow` 的主布局容器 (`div.set-row`) 通过 `style={{ flexDirection: "column", ... }}` 覆盖了 `.set-row` 的默认 flex 方向，内部 input 的外观也全用 inline style (`style={{ width:180, padding:... border:... }}`)。D2 编码文档声明「样式从 inline style 迁移到设计系统组件类」，HotkeyRow 是最重要的改造目标，但遗漏了 input 和布局容器的迁移，形成自相矛盾：同一面板里 SettingToggle 完全用类，HotkeyRow 大量残留 inline。

**问题（ClipboardPage）**：根容器 `<div style={{ display:"grid", gridTemplateColumns:"340px 1fr", ... fontFamily:"var(--font)" }}>` 和错误横幅 `<div role="alert" style={{ ... }}>` 仍全用 inline style。`fontFamily` 尤其多余——`body` 在 `base.css` 中已设置 `font-family: var(--font)`，此处 inline 是无效冗余。

**建议**：将 HotkeyRow 的布局变体抽为 `.set-row.column` CSS 变体；input 外观复用 `.src-input` 或在 settings.css 新增 `.set-input`；ClipboardPage 根容器 layout 抽为 `.clip-view` CSS 类；错误横幅可用 `.tx-error` 同款模式写进 clipboard.css。

**置信度**：85

---

### 3. [内联样式未迁移·中高] TranslateSourcePanel、PrivacyPanel 错误提示全用 inline style

**文件**：
- `src/panels/settings/TranslateSourcePanel.tsx:40,104,123`
- `src/panels/settings/PrivacyPanel.tsx:76,113,131,137`

**问题**：`cursor: pointer` 硬编码在 ProviderCard JSX 的 `style` 属性中而非 `.src-card` CSS 规则里；错误提示 `div[role="alert"]` 的颜色 `color: "var(--danger)"` 也全用 inline；PrivacyPanel 排除名单 input 的样式与 HotkeyPanel 中相同的 input 完全重复（同款 `style={{ width:180, padding:"4px 8px", ... }}`），违反 DRY 原则。

**建议**：`.src-card` 追加 `cursor: pointer` 规则；错误提示样式复用 `.tx-error` 或抽成 `.alert-error` 通用类；input 外观抽为共享 CSS 类。

**置信度**：82

---

## 二、中低优先级 / 可选改进（简列）

1. **[风格] `setActiveSection(() => section)` 的函数式更新形式不必要**（`SettingsPage.tsx:138`）：`section` 不依赖前一个 state，直接 `setActiveSection(section)` 更清晰，函数式更新应用于依赖 prev 的场景，此处用法增加了阅读成本。置信度 75，不作为强制改项但值得注意。

2. **[风格] `ClipboardPage` 主函数体 ~155 行，超出函数 ≤50 行上限**：`ClipboardPage` 是 React 函数组件，其协调职责合理，行数超出是因为大量状态声明和内联 JSX 视图，实际逻辑行约 60 行（state 声明占大部分）。可将操作横幅和列表区拆为子组件，但属于锦上添花。置信度 70（判定 code-standards §3 原则，但项目内同类组件惯例如此）。

3. **[可访问性·轻微] `ProviderCard` 使用 `<div onClick>` 而非 `<button>`** (`TranslateSourcePanel.tsx:37-57`)：`<div>` 的 click handler 不具备原生键盘可访问性（Enter/Space 触发），屏幕键盘用户无法用键盘操作。虽然内部有 `<input type="radio">` 保留了键盘访问，但 `<div onClick>` 拦截了 click，可能导致双触发或 radio change 与外层 click 的冲突。建议将整个卡片改为 `<label>` 包裹 radio，label 可交互天然无障碍，也不需要 `sr-only` 隐藏 radio。置信度 72。

4. **[CSS·轻微] `.src-card` 缺失 hover/focus-within 样式**：settings.css 中 `.src-card` 无 hover 状态，在视觉上无交互反馈（`cursor: pointer` 也目前是 inline style），与 `.clip-row:hover`、`.hist-row:hover` 等其他可交互行的一致性缺失。置信度 70。

5. **[logoAbbr 边界] `logoAbbr` 函数对空字符串或单字符名称无防御**（`TranslateSourcePanel.tsx:12-14`）：`name.slice(0, 2).toUpperCase()` 对空串返回空字符串，视觉上 logo 方块为空。实际 provider 数据来自 IPC，可能包含异常数据。置信度 65（实际数据路径受控，风险低）。

6. **[测试·轻微] `HotkeyPanel.handleSaved` 调用 `fetchHotkeys(cancelled)` 但不处理返回的 Promise**（`HotkeyPanel.tsx:118-121`）：`fetchHotkeys` 是 async 函数，`handleSaved` 是同步函数且不 `void`/`await`——在 strict 模式下可能触发 unhandled promise warning。置信度 68（当前测试已通过，实际运行未发现报错，但存在最佳实践隐患）。

---

## 三、行为契约保持核查

**结论：三页重塑真正只改了视觉，IPC/状态/键盘流/错误处理逻辑均未动。** 具体验证：

| 维度 | 核查结论 |
|---|---|
| 剪贴板 IPC 路径 | `listClipItems` / `deleteClipItem` / `toggleFavoriteClip` 调用位置、参数、错误处理均未变 |
| 键盘流 | `handleKeyDown`（Arrow/Enter/Cmd+数字）逻辑完整保留，只是 JSX 结构类名变了 |
| 翻译 IPC 路径 | `translateText` / `listTranslateHistory` / `getTranslateProviders` / `setSelectedProvider` 等调用链完整，错误降级逻辑（历史取数失败不阻断主流）保留 |
| 设置热键 IPC | `getHotkeys` / `setHotkey` 及 `validateRebind` 冲突校验逻辑完整保留 |
| 隐私 IPC | `getExcludeList` / `setExcludeList` 及 `addExcludedApp` / `removeExcludedApp` 逻辑完整保留 |
| 翻译源 IPC | `getTranslateProviders` / `getSelectedProvider` / `setSelectedProvider` 完整保留 |
| 错误处理 | 各面板的 loadError/opError 双 state 模式完整保留，role="alert" 展示逻辑不变 |
| cancelled ref 模式 | 所有 useEffect 的卸载防写均保留 |

---

## 四、待接项抽象与测试演进合理性

### 待接项抽象：合格

| 占位项 | 接入点 | 评价 |
|---|---|---|
| `useGeneralSettings`（开机自启/托盘/自动更新） | `useState` 本地，注释说明里程碑3替换为 IPC 读写，组件接口不变 | 接口设计合理，`GeneralSettings` 类型化，里程碑3只需替换 hook 内部实现 |
| `enterToPaste` 占位 toggle（HotkeyPanel） | `useState(true)` 本地，注释说明里程碑3接入 | 同 SettingToggle 接口，零改动接入点正确 |
| `pauseCapture` / `skipSensitive` 占位 toggle（PrivacyPanel） | `useState` 本地，注释说明 | 同上，接入点正确 |
| `StoragePanel` 进度条静态占位 | aria-hidden 占位，注释说明里程碑3替换为 IPC 读取 | 注释清晰，无 TODO 关键词残留 |
| `OnboardCard.onOpenSystemSettings` noop | 注释「里程碑3 接 IPC」 | 清晰 |

**注意**：`useGeneralSettings` 的三个 setter（`setLaunchOnLogin` 等）直接暴露了 React `setState` 函数引用。里程碑3接 IPC 时，若要在 setter 中触发 IPC 调用，需要在 hook 内部包装，组件侧 `onChange={setLaunchOnLogin}` 的调用方式不变——这一接口设计是合理的。

### 测试演进：合理

**settings-page.test 中 radio 的可访问性测试**：`getByRole("radio", { name: "DeepL" })` 确实能命中 `.sr-only` radio（position:absolute+1px+clip 对 DOM 查询透明），测试真实覆盖了翻译源选择 IPC 调用路径，非橡皮图章。

**新增 3 个测试（批次D2）**的断言质量：
- `.src-card` badge 三态测试：验证了具体文字（"默认"/"待配置"/"无需 Key"）和选中切换后的状态变化，非恒真。
- 热键双 input 测试：`getAllByRole("button", { name: "保存" })` 断言数量为 2，`getByDisplayValue` 验证具体值，非恒真。
- chip 结构测试：`getByRole("button", { name: "删除 Xcode" })` 具体 aria-label 命中，非恒真。

**`mockSetSelectedProvider` 未被 verify** 在 badge 三态测试中：该测试调用了 `user.click(radio)` 但只断言了 badge 文字变化，没有断言 `setSelectedProvider` 被调用——这实际上是合理的分工，IPC 调用在独立的"选一个调 setSelectedProvider(id)"测试中已验证，badge 三态测试专注视觉结构，避免测试重复。

**`useGeneralSettings.test.ts`**：5 个测试覆盖了初始值、三个 setter 独立切换、反复 toggle，断言具体布尔值，质量可接受。

---

## 五、可访问性审查小结

| 检查项 | 结论 |
|---|---|
| 图标 `aria-hidden="true"` | 全部主要图标已加，ClipItemRow/ClipPreview/SectionNav/TranslateHistoryPanel 均覆盖 |
| `aria-current="page"` 驱动导航选中态 | SideBar + SectionNav 均正确实现 |
| `aria-label` on 搜索/筛选 | ClipSearchBar input `aria-label="搜索剪贴板内容"` ✓，select `aria-label="类型筛选"` ✓ |
| `role="switch"` + `aria-checked` | SettingToggle 正确实现，`type="button"` 防表单提交 ✓ |
| `.sr-only` radio 可访问性 | 实现正确，position:absolute+1px+clip+white-space:nowrap 为标准方案，测试可命中 ✓ |
| `role="listbox"` + `role="option"` | 剪贴板列表 `role="listbox"` ✓；**翻译历史 `button[role="option"]` 缺 listbox 父容器 × （高优先级问题 #1）** |
| `aria-label` on 关闭/删除按钮 | OnboardCard 关闭 ✓，chip 删除 `aria-label="删除 ${app}"` ✓ |

---

## 六、CSS 正确性与组织审查

| 检查项 | 结论 |
|---|---|
| `--qq-*` token 残留 | 无残留，grep EXIT:1 ✓ |
| compat 别名删除 | tokens.css 已删，grep 无遗漏使用方（所有 var() 均用短名） ✓ |
| @import 链 | theme.css → tokens.css → base.css → components.css，顺序正确 ✓ |
| 三页 CSS 短名 token | clipboard.css / translate.css / settings.css 全用短名 ✓ |
| 装饰性分隔注释 | 无 ════/────/━━━ 等横线分隔，CSS 注释均为语义说明 ✓ |
| `.set-group/.set-row` 迁移 | 已从 components.css 迁至 settings.css，注释标注原因 ✓ |
| `.sr-only` 放置层 | 放 components.css（组件辅助类），不放 base.css（reset/排版），分层合理 ✓ |
| `cursor: pointer` 缺失 | `.src-card` CSS 规则中无 cursor 声明，当前靠 inline style 实现 —— 见高优先级问题 #3 |

---

## 七、总体结论

**状态：未过 — 需修改后通过**

必改项（建议在进入里程碑3前修复）：

1. **TranslateHistoryPanel ARIA 问题**（高优先级 #1）：`button[role="option"]` 缺 listbox 父容器，违反 ARIA 规范，屏幕阅读器无法正确解析历史列表关系。修复方案二选一：给 `.tx-hist-list` 加 `role="listbox"`，或将 `role="option"` 去掉改为普通按钮。

2. **HotkeyRow inline style 未迁移**（高优先级 #2 的一部分）：与里程碑2"样式迁移"改造目标直接矛盾，HotkeyRow 的布局和 input 外观应通过 CSS 类管理。

3. **PrivacyPanel input inline style 与 HotkeyPanel 重复**（高优先级 #3 的一部分）：同款 inline style 在两处重复，需抽为共享 CSS 类。

可选项（不阻塞里程碑3，但建议清理）：

- ClipboardPage 根容器 layout 抽为 CSS 类，去掉 fontFamily inline（base.css 已全局设置）
- TranslateSourcePanel `cursor: pointer` 移入 `.src-card` CSS
- `ProviderCard` 考虑改为 `<label>` 包裹 radio 的无障碍标准做法

**合格项**（无需改动）：行为契约完整保持、待接 toggle 抽象质量高、测试演进断言非恒真、CSS token 迁移彻底、代码无 any/装饰注释/TODO 残留、所有主要图标 aria-hidden 覆盖。
