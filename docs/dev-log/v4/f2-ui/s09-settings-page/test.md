# S09 设置页 Phase 6 测试报告

**任务**: QuickQuick V4/F2/S09（设置页）  
**执行日期**: 2026-06-01  
**tester 模型**: claude-sonnet-4-6  

---

## 1. 命中校验

### settings-page（N=9）

命令: `pnpm test settings-page --reporter=verbose`

```
Test Files  1 passed (1)
Tests  9 passed (9)
```

9 个测试逐一命中：
- settings-page: 左侧纵向子项栏渲染六个子项（通用/热键/翻译源/隐私/存储/关于） ✓
- settings-page: 默认选中通用，点击热键后右内容切换（DOM 变化） ✓
- settings-page: 热键面板——输入与另一动作相同的键显示已被占用且不调用 setHotkey ✓
- settings-page: 热键面板——输入不冲突键后调用 setHotkey(正确参数) ✓
- settings-page: 隐私面板——添加一项后列表出现该项且调 setExcludeList(含该项) ✓
- settings-page: 隐私面板——删除一项后调 setExcludeList(不含该项) ✓
- settings-page: 翻译源面板——渲染 providers 列表，选一个调 setSelectedProvider(id) ✓
- settings-page: 热键面板加载失败时显示错误提示（role=alert） ✓
- settings-page: 关于面板显示应用名 QuickQuick ✓

**无假绿，N=9 真实命中。**

### 全量（141）

命令: `pnpm test`

```
Test Files  17 passed (17)
Tests  141 passed (141)
```

### app-shell（6）

命令: `pnpm test app-shell --reporter=verbose`

```
Test Files  1 passed (1)
Tests  6 passed (6)
```

---

## 2. 变异 Sanity（3 处，全部如期变红）

备份方式: 改前 `cp <文件> /tmp/<文件>.bak`，还原 `cp /tmp/<文件>.bak <文件>`，严禁 git checkout。

### 变异 1：改坏热键冲突守卫（HotkeyPanel.tsx）

改动: 删除 `handleSave` 中冲突守卫的 `return`，使 `validateRebind` 返回 `ok:false` 时仍继续调 `setHotkey`。

结果：
- `settings-page: 热键面板——输入与另一动作相同的键显示已被占用且不调用 setHotkey` **变红** ×
- 其余 8 个仍绿
- 从 `/tmp/HotkeyPanel.tsx.bak` 已还原

### 变异 2：改坏排除名单添加（PrivacyPanel.tsx）

改动: 注释掉 `handleAdd` 中 `await setExcludeList(newList)` 调用（旁路 IPC）。

结果：
- `settings-page: 隐私面板——添加一项后列表出现该项且调 setExcludeList(含该项)` **变红** ×
- 其余 8 个仍绿
- 从 `/tmp/PrivacyPanel.tsx.bak` 已还原

### 变异 3：改坏子项切换（SettingsPage.tsx）

改动: 将 `onSelect={(section) => setActiveSection(() => section)}` 改为 no-op，点击子项不更新 `activeSection`。

结果：
- 8 个依赖子项切换的测试**全部变红** ×（含"点击热键后右内容切换"及所有需导航到子面板的测试）
- 仅"六个子项渲染"（不依赖切换）保持绿
- 从 `/tmp/SettingsPage.tsx.bak` 已还原

### 还原验证

结束时 `git status --porcelain` 快照与开工完全一致：

```
 M src/App.tsx
?? docs/dev-log/v4/f2-ui/s09-settings-page/
?? src/panels/settings/
```

**无新增/丢失，工作树干净（含原有未提交改动）。**

---

## 3. 边界探测

通过静态分析 + 内联 vitest 验证（6 个边界 case，全部通过）：

| 边界场景 | 实现路径 | 结果 |
|---|---|---|
| `validateRebind("")` 空字符串 | `occupied.includes("")` 为 false → ok:true | 通过，不报错 |
| `validateRebind("A"*200)` 超长键名 | 不在 occupied 中 → ok:true | 通过，不崩溃 |
| `addExcludedApp` 重复项 | `list.includes(app)` 去重，返回副本 | 通过，不增项 |
| `removeExcludedApp` 不存在项 | `filter` 不匹配 → 返回原内容副本 | 通过，不报错 |
| `removeExcludedApp` 空名单 | `[].filter(...)` → 空数组 | 通过，不崩溃 |
| `getHotkeys` reject → role=alert | 已有测试覆盖（case 8），HotkeyPanel 捕获异常显示 loadError | 通过 |

补充分析：
- `getExcludeList` reject 路径：PrivacyPanel 的 `fetchExcludeList` 有 try/catch，catch 时设 `loadError` → 渲染 `<div role="alert">`，不崩溃（代码路径确认，无专用测试但结构与 HotkeyPanel 对称）。
- `getTranslateProviders` 空列表：TranslateSourcePanel 渲染空 radio 列表，不崩溃（无专用测试，但实现为 `.map()` 无守卫崩溃风险）。
- UI 层守卫：PrivacyPanel.handleAdd 有 `trimmed.length === 0` 守卫，空字符串不走 IPC，防止空项入库。

**边界无新缺陷发现。**

---

## 4. 失败项 / 覆盖缺口

**失败项**: 无。

**覆盖缺口（次要，不阻放行）**:
- `getExcludeList` reject → PrivacyPanel `role=alert` 无专用测试（代码结构已覆盖但测试未显式验证）。
- `getTranslateProviders` 空列表渲染无专用测试。
- `setExcludeList` IPC 调用失败（throw）时 `opError` 渲染 `role=alert` 无专用测试。

以上缺口均为补充覆盖场景，不属于验收 A09 冻结项，**不阻放行**。

---

## 5. 门禁结论

**放行。**

- 命中校验：settings-page 9/9 ✓；全量 141/141 ✓；app-shell 6/6 ✓。
- 变异 sanity：3 处改坏均如期变红，测试有真实判别力，非恒真/旁路。
- 边界探测：6 个边界边缘无崩溃无 panic，UI 守卫到位。
- 工作树与开工快照逐行一致，无残留变异。

---

## 修订 R1 验证（I-1：opError 独立 state）

**执行日期**: 2026-06-01
**tester 模型**: claude-sonnet-4-6
**开工 git 快照**: ` M docs/dev-log/v4/acceptance.yaml` / ` M src/App.tsx` / `?? docs/dev-log/v4/f2-ui/s09-settings-page/` / `?? src/panels/settings/`

---

### 1. 命中校验

**settings-page（N=10）**

命令: `pnpm test settings-page --reporter=verbose`

```
Test Files  1 passed (1)
Tests  10 passed (10)
```

10 个测试逐一命中（9 原有 + 1 新增）：
- settings-page: 左侧纵向子项栏渲染六个子项（通用/热键/翻译源/隐私/存储/关于） ✓
- settings-page: 默认选中通用，点击热键后右内容切换（DOM 变化） ✓
- settings-page: 热键面板——输入与另一动作相同的键显示已被占用且不调用 setHotkey ✓
- settings-page: 热键面板——输入不冲突键后调用 setHotkey(正确参数) ✓
- settings-page: 隐私面板——添加一项后列表出现该项且调 setExcludeList(含该项) ✓
- settings-page: 隐私面板——删除一项后调 setExcludeList(不含该项) ✓
- settings-page: 翻译源面板——渲染 providers 列表，选一个调 setSelectedProvider(id) ✓
- settings-page: 热键面板加载失败时显示错误提示（role=alert） ✓
- settings-page: 翻译源面板——setSelectedProvider reject 时列表仍可见且显示 opError 提示 ✓（新增）
- settings-page: 关于面板显示应用名 QuickQuick ✓

**全量（N=142）**

命令: `pnpm test`

```
Test Files  17 passed (17)
Tests  142 passed (142)
```

全量从 141 升至 142，全绿，无假绿。

---

### 2. 变异 Sanity（杀恒真/旁路）

**目标**: 改坏 handleSelect 失败路径（退回旧 bug：setOpError → setLoadError），新增的 R1 第 10 个测试必须变红。

**改动**: `TranslateSourcePanel.tsx` handleSelect catch 分支 `setOpError(...)` → `setLoadError(...)`，成功分支 `setOpError(null)` → `setLoadError(null)`。失败时 loadError 非 null 触发早返回，整个面板被替换，列表消失。

**备份/复原**: 改前 `cp TranslateSourcePanel.tsx /tmp/TranslateSourcePanel.tsx.bak`；复原 `cp /tmp/TranslateSourcePanel.tsx.bak TranslateSourcePanel.tsx`；严禁 git checkout/restore。

**变异结果**:
```
Tests  1 failed | 9 passed (10)
```
- 第 10 个测试「翻译源面板——setSelectedProvider reject 时列表仍可见且显示 opError 提示」**如期变红** ×
  - 失败点: `expect(screen.getByText("Google 翻译")).toBeInTheDocument()` — 面板被 loadError 早返回替换，列表从 DOM 消失
- 其余 9 个仍绿
- 已从备份复原，复原后 10/10 再次全绿

**结论**: 测试有真实判别力，非恒真/旁路。R1 改动有效区分了旧 bug（loadError 替换面板）与新修复（opError 不替换面板）。

---

### 3. 边界探测

动态执行临时边界测试（执行完删除），3 个 R1 边界场景全通过：

| 边界场景 | 实现路径 | 结果 |
|---|---|---|
| 切换成功后 opError 不出现 | handleSelect try 成功 → `setOpError(null)` → queryByRole("alert") 为 null | 通过 |
| reject 后 opError 出现，列表仍可见 | handleSelect catch → `setOpError(...)` → alert 出现，列表 DOM 保留 | 通过 |
| 连续失败再成功：opError 先出现后消失 | 第1次 reject → opError 出现；第2次 resolve → `setOpError(null)` → opError 消失 | 通过 |

说明：边界2-b 测试有 `act()` 使用警告（测试写法问题，非实现缺陷），不影响断言结果。

---

### 4. 结束 git 快照

```
 M docs/dev-log/v4/acceptance.yaml
 M src/App.tsx
?? docs/dev-log/v4/f2-ui/s09-settings-page/
?? src/panels/settings/
```

与开工快照逐行一致，无新增/丢失，无残留变异。

---

### 5. 门禁结论（修订 R1）

**放行。**

- 命中校验：settings-page 10/10（含新增 R1 第 10 个）✓；全量 142/142 ✓；无假绿。
- 变异 sanity：改坏 handleSelect 失败路径为旧 bug（setLoadError），第 10 个测试如期变红（面板被替换列表消失），测试有真实判别力，已从备份复原。
- 边界探测：3 个 R1 边界场景（成功清 opError / reject 出 opError / 连续失败再成功）全通过，无真实缺陷。
- 工作树与开工快照逐行一致，无残留变异。
