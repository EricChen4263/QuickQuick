# S08 翻译页 Phase 6 测试报告（动态证伪）

## 一、命中校验（档位 1）

| 目标 | 命令 | 结果 |
|------|------|------|
| translate-page | `pnpm test translate-page` | **9/9 passed**，Test Files 1 passed，无空匹配假绿 |
| 全量 | `pnpm test` | **131/131 passed**，Test Files 16 passed |
| app-shell | `pnpm test app-shell` | **6/6 passed**，Test Files 1 passed |

9 个测试均有具名命中（每行 `✓ translate-page.test.tsx > translate-page > ...`），排除空匹配假绿。

## 二、变异 sanity（档位 2，3 处）

### 变异1：注释 `setResult(res)` — 译文不写入 state

- 文件：`TranslatePage.tsx`，`handleTranslate`，`setResult(res)` 注释掉
- 预期变红：**4 个测试变红**（译文显示、copy 前置、连续翻译前置、speak 前置），实际如期
- 复原：`cp /tmp/TranslatePage.tsx.bak` 恢复

### 变异2：注释 `setInputText(item.sourceText)` — 历史回填 input 失效

- 文件：`TranslatePage.tsx`，`handleSelectHistoryItem`，`setInputText` 注释掉
- 预期变红：**1 个测试变红**（历史回填 input，`'' !== 'Hello'`），实际如期
- 复原：同上

### 变异3：`writeToClipboard(result.translated)` → `writeToClipboard("MUTATION_WRONG")`

- 文件：`TranslatePage.tsx`，`handleAction` copy 分支
- 预期变红：**1 个测试变红**（copy 断言 `toHaveBeenCalledWith("你好世界")` 失败，实际调参 `"MUTATION_WRONG"`），实际如期
- 复原：同上

**三处改坏均如期变红，无恒真/旁路测试；git status 快照开工/结束逐行一致，工作树已完全复原。**

## 三、边界探测（档位 3）

以下边界场景均由测试文件中的独立用例覆盖，实测全绿：

| 边界场景 | 测试 | 结果 |
|----------|------|------|
| 空输入 → 翻译按钮 disabled，不调 translateText | `空输入时翻译按钮禁用，不调用 translateText` | 通过 |
| translateText reject → role=alert 显示，不崩溃 | `translateText reject 时显示错误提示（role=alert）不崩溃` | 通过 |
| 连续翻译先成功后失败 → alert 出现 | `翻译成功后再次翻译失败时错误提示出现` | 通过 |
| 空历史 → 占位文案 `暂无翻译历史` | `listTranslateHistory 返回空数组时显示空历史占位文案` | 通过 |
| speak → 调 `speakText(译文)` | `speak 按钮调用 speakText 并传译文` | 通过 |

无发现未覆盖的真实缺陷。

## 四、失败项 / 覆盖缺口

无失败项。以下为观察到的覆盖现状说明：

- 取消 / cancelled flag：`fetchHistory` 中有 cancelled flag 防卸载后写 state，属于 React 生命周期保护，jsdom 环境难以精确触发，当前未单独用例覆盖（功能性风险低，属可接受缺口）。
- listTranslateHistory 取数 reject：静默处理，未覆盖负向历史用例（历史取数失败不阻断主流程，低风险）。

## 五、门禁结论

**放行。**

translate-page 9/9、全量 131/131、app-shell 6/6 全绿；三处变异 sanity 全部如期变红（无恒真/旁路）；边界探测无新缺陷；工作树完全复原。

---

## 修订 R1 验证（V4/F2/S08 R1）

### 改动范围
- `handleAction` 加顶层 try/catch，失败 `setError("操作失败...")`，成功 `setError(null)`（I-1）
- `fetchHistory` catch 加 `console.error`（I-2）
- 测试由 9 增至 10（+1 copy 操作失败显示错误）

### 一、命中校验

| 目标 | 命令 | 结果 |
|------|------|------|
| translate-page | `pnpm test translate-page` | **10/10 passed**，Test Files 1 passed，无空匹配假绿 |
| 全量 | `pnpm test` | **132/132 passed**，Test Files 16 passed |

10 个测试均有具名命中（`✓ translate-page.test.tsx > translate-page > ...`），无空匹配假绿。

### 二、变异 sanity（验 I-1 判别力）

- **改坏处**：`handleAction` catch 块中 `setError("操作失败，请稍后重试")` 替换为注释（// MUTATION: swallowed error），模拟吞错误
- **对应测试**：第10个测试「copy 操作 reject 时显示错误提示（role=alert）」
- **结果**：如期变红——`Tests: 1 failed | 9 passed (10)`，`waitFor` 超时找不到 role=alert
- **复原**：`cp /tmp/TranslatePage.tsx.bak <文件>` 复原，**未使用 git checkout/restore**
- **git 快照**：开工/结束逐行一致（` M src/App.tsx`、` M src/app-shell.test.tsx`、`?? docs/dev-log/v4/f2-ui/s08-translate-page/`、`?? src/panels/translate/`），工作树完全复原

### 三、边界探测

| 边界场景 | 验证方式 | 结论 |
|----------|----------|------|
| copy 成功后旧错误被清（setError(null)） | 代码审查：copy 分支第83行 `setError(null)` 在 await 成功后执行 | 逻辑正确 |
| speak 成功后旧错误被清（setError(null)） | 代码审查：speak 分支第88行 `setError(null)` | 逻辑正确 |
| speak 若抛异常能被捕获 | 代码审查：speak 分支在顶层 try 内，catch 兜底 | 逻辑正确 |
| fetchHistory 取数失败不阻断主流程且记录日志 | 代码审查：第39行 console.error + catch 静默不 rethrow | 符合 I-2 要求 |

无发现未覆盖的真实缺陷。

### 四、失败项 / 覆盖缺口

无失败项。覆盖缺口同原报告（speak 操作失败 → alert 无专门用例，历史取数 reject 无专门用例，风险低，可接受）。

### 五、门禁结论

**放行。**

translate-page 10/10、全量 132/132 全绿；变异 sanity 改坏 handleAction catch 后第10个测试如期变红（判别力确认，非恒真/旁路）；边界探测无新缺陷；工作树完全复原，git 快照逐行一致。
