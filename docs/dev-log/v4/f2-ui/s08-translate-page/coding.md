# S08 翻译页 — 编码留痕

- 小功能：S08-translate-page
- 大功能：F2 三页 React UI
- 版本：V4
- 执行者：coder agent (Phase 5)
- 完成时间：2026-06-01

## 1. 改动文件清单

| 文件 | 改动说明 |
|------|----------|
| `src/panels/translate/TranslatePage.tsx` | 新建：翻译页根组件，协调 IPC 调用、历史取数、操作分发、错误处理 |
| `src/panels/translate/TranslateWorkspace.tsx` | 新建：工作区子组件，输入框 + 翻译按钮 + 原文/译文对照 + 操作按钮条 |
| `src/panels/translate/TranslateHistoryPanel.tsx` | 新建：历史右栏子组件，按时间倒序渲染历史条目列表，点击触发回填 |
| `src/panels/translate/browser-api.ts` | 新建：浏览器 API 薄封装（navigator.clipboard / speechSynthesis），隔离 jsdom 限制使 vi.mock 可替换 |
| `src/panels/translate/translate-page.test.tsx` | 新建：8 个渲染测试，覆盖翻译主流/历史渲染/历史回填/复制/朗读/错误/连续失败/空输入/空历史 |
| `src/App.tsx` | 改：page-translate section 内容换为 `<TranslatePage />`；`app-shell.test.tsx` 补 `listTranslateHistory` / `translateText` 永 pending mock 及 `browser-api` mock，消除 act 外异步警告 |
| `src/app-shell.test.tsx` | 改：补 `translateText` / `listTranslateHistory` 永 pending mock 和 `browser-api` mock，保持 6 个 app-shell 测试全绿无 act 警告 |

## 2. 关键实现决策

### 2.1 三栏布局：主窗左导航由 App.tsx 提供，翻译页自管「工作区 + 历史」两栏

`TranslatePage` 根元素为水平 flex 容器：左侧 `flex:1` 区域嵌套垂直 flex 的工作区（含错误提示 + `TranslateWorkspace`），右侧固定宽 240px 的 `TranslateHistoryPanel`（`aside`）。左侧导航由父级 `App.tsx` 的 `<nav>` 提供，翻译页不重复渲染导航，符合单一职责。

### 2.2 原文/译文上下对照

`TranslateWorkspace` 渲染顺序：输入区（textarea + 翻译按钮）在上，译文区（方向标识 + 译文文本 + 操作按钮条）在下，天然实现上下对照结构，无需额外容器。`result !== null` 条件渲染译文区，无结果时只显示输入区。

### 2.3 availableActions() 操作集复用

`TranslateWorkspace` 直接调用 `availableActions()`（来自既有 `src/translate/translate-actions.ts`）取操作列表，过滤掉 `save_history`（该操作对用户无可见按钮语义）后渲染按钮条。`resolveTranslateAction` 做命令字符串到类型的映射。两个函数均已有完整测试，本任务直接复用，无重造。

### 2.4 copy/speak 用浏览器 API，经 browser-api.ts 隔离

`navigator.clipboard.writeText` 和 `window.speechSynthesis.speak` 被封装在 `src/panels/translate/browser-api.ts` 中。隔离原因：jsdom 的 secure-context 限制导致直接调用 `navigator.clipboard` 时 vi.mock 对全局对象的拦截不稳定；独立模块可被 `vi.mock("./browser-api", ...)` 精确替换，测试不依赖环境 API 能力。

### 2.5 历史回填工作区：点击条目直接填 state，不再发起新翻译

`handleSelectHistoryItem` 接收 `TranslateHistoryItem`，直接将 `inputText` 设为 `item.sourceText`，将 `result` 设为由历史字段构造的 `TranslateResult` 对象（`translated / sourceLang / targetLang`），同时清除 `error`。此方式避免无谓网络请求，也保证点击历史时工作区即刻回显，体验更快。

### 2.6 save_history 去重：translate_text 后端已自动写历史

后端 `translate_text` 命令执行时已自动写入翻译历史（V4-F1 实现）。`handleAction` 的 `save_history` 分支不再重复写入，仅调 `fetchHistory` 刷新列表。`TranslateWorkspace` 中操作按钮条也过滤掉 `save_history`，用户不见此按钮，消除混乱。翻译成功后的 `fetchHistory` 调用已覆盖自动刷新需求。

### 2.7 async handler try/catch + cancelled flag 健壮性

- `fetchHistory` 用 `useCallback` 包裹，接收 `cancelled: { current: boolean }` 参数，在 `setHistory` 前检查 `cancelled.current`，防止组件卸载后 Promise resolve 写入已销毁 state。历史取数失败静默处理（catch 空），不阻断主翻译工作流。
- `handleTranslate` 用 `try/catch/finally`：成功路径更新 result + 刷新历史；失败路径 `setError("翻译失败，请稍后重试")`，`finally` 保证 `setIsLoading(false)`。
- 两个 handler 均为 async 函数，错误不静默吞掉（翻译失败走 `setError`，历史失败仅注释说明原因）。

### 2.8 App.tsx 接入方式

`page-translate` section 内容从占位文字替换为 `<TranslatePage />`，`data-testid="page-translate"` 和 `display` 控制保持不变，app-shell 测试契约不破。`TranslatePage` import 加在 `ClipboardPage` import 之后，顺序与 nav 定义一致。

### 2.9 app-shell.test.tsx 补 mock 的必要性

TranslatePage 挂载时 `listTranslateHistory` 发起异步请求，若 reject 或 resolve 在 `act()` 边界外发生则产生 act 警告（S07 已有相同处理经验）。解法：在 `app-shell.test.tsx` 中让 `listTranslateHistory` / `translateText` 返回永远 pending 的 Promise，并 mock `browser-api`，彻底断开异步 state 更新与 act 边界的关联。

## 3. 假设 / 未决 / 需用户确认

- **真实翻译网络往返**：`translateText` IPC 依赖后端翻译服务，实际网络往返质量属 V4-F1-A02-H01 manual 任务，本任务测试全为 mock。
- **§8 选中即译浮窗**：选中文字弹出翻译浮窗属另一系统窗口方案，不在本翻译页范围内，归未来项。
- **视觉还原 A10**：布局骨架和 CSS 变量占位已建，精确视觉还原（字体、间距、色彩完全对齐设计稿）待 manual 阶段评审。
- **历史倒序排序**：`TranslateHistoryPanel` 按 `items` 数组顺序渲染，倒序由后端 `listTranslateHistory` 保证，前端无额外排序逻辑（假设后端已保证）。
- **switch_target / switch_source_retranslate**：当前实现简化为重发当前输入文本（调 `handleTranslate`），真实语言切换逻辑（更新目标语参数）归后续迭代。

## 4. 测试证据

### translate-page 命中测试（冻结 verify：`pnpm test translate-page`）

```
✓ translate-page: 输入文本点击翻译后调用 translateText 并显示译文和语言方向
✓ translate-page: 翻译历史列表渲染——显示各历史条目的 sourceText 和 translatedText
✓ translate-page: 点击历史某项后工作区回填（input 变为该项 sourceText，结果显示其 translatedText）
✓ translate-page: copy 按钮调用 navigator.clipboard.writeText 并传译文
✓ translate-page: translateText reject 时显示错误提示（role=alert）不崩溃
✓ translate-page: 翻译成功后再次翻译失败时错误提示出现
✓ translate-page: 空输入时翻译按钮禁用，不调用 translateText
✓ translate-page: listTranslateHistory 返回空数组时显示空历史占位文案
✓ translate-page: speak 按钮调用 speakText 并传译文

Tests  9 passed (9)
```

### app-shell 命中测试（接入后，冻结 verify：`pnpm test app-shell`）

```
Tests  6 passed (6)（无 act 外异步警告）
```

### 全量回归（`pnpm test`）

```
Test Files  16 passed (16)
     Tests  131 passed (131)
```

tsc 类型检查：仅 `src/theme/design-tokens.test.ts` 存在 3 个预存错误（与本任务无关，前序阶段已记录），其余文件类型检查通过。

## 修订 R1（reviewer I-1/I-2）

- 执行者：coder agent
- 修订时间：2026-06-01

### 改法

**I-2（`fetchHistory` catch 静默）**

原 `catch {}` 块空，历史取数失败完全无日志。改为 `catch (err)` 并加 `console.error("[QuickQuick] 翻译历史取数失败:", err)`，保持 cancelled guard 不变，不影响主流程。

**I-1（`handleAction` copy/speak 无 try/catch）**

原 copy/speak 分支裸 `await`，失败静默。改法：给整个 `handleAction` 主体加顶层 `try/catch`，catch 内 `setError("操作失败，请稍后重试")`，复用现有 `role="alert"` 错误态。copy 和 speak 成功路径各自加 `setError(null)` 清旧错误（switch_target/save_history 路径不清错是因为它们内部调 `handleTranslate`/`fetchHistory`，由那些路径自己管错误态）。

### 补测试

新增测试名：

```
translate-page: copy 操作 reject 时显示错误提示（role=alert）
```

断言：`writeToClipboard` reject 时，`role="alert"` 出现且文案包含"失败"。证明 I-1 修复有判别力（原实现该测试红，修后绿）。

### 测试结论

**translate-page（冻结 verify：`pnpm test translate-page`）**

```
✓ translate-page: 输入文本点击翻译后调用 translateText 并显示译文和语言方向
✓ translate-page: 翻译历史列表渲染——显示各历史条目的 sourceText 和 translatedText
✓ translate-page: 点击历史某项后工作区回填（input 变为该项 sourceText，结果显示其 translatedText）
✓ translate-page: copy 按钮调用 navigator.clipboard.writeText 并传译文
✓ translate-page: translateText reject 时显示错误提示（role=alert）不崩溃
✓ translate-page: 翻译成功后再次翻译失败时错误提示出现
✓ translate-page: 空输入时翻译按钮禁用，不调用 translateText
✓ translate-page: listTranslateHistory 返回空数组时显示空历史占位文案
✓ translate-page: speak 按钮调用 speakText 并传译文
✓ translate-page: copy 操作 reject 时显示错误提示（role=alert）

Tests  10 passed (10)
```

**全量回归（`pnpm test`）**

```
Test Files  16 passed (16)
     Tests  132 passed (132)
```

## 5. code-standards 自检

| 规范项 | 状态 | 说明 |
|--------|------|------|
| 格式：2 空格缩进、无 Tab | 通过 | 所有新文件均 2 空格 |
| 函数 ≤50 行、嵌套 ≤3 层 | 通过 | TranslatePage 最长函数 handleAction 约 20 行；各子组件单职责，无深嵌套 |
| 命名：camelCase / PascalCase | 通过 | ACTION_LABELS、EMPTY_HISTORY_PLACEHOLDER 具名常量；handler 命名动词+名词 |
| 禁 any | 通过 | 无 any，所有 props 均有 interface 类型定义；TranslateResult / TranslateHistoryItem 从 ipc-client 导入 |
| setState 函数式更新 | 部分 N/A | 本组件 setActiveTop 用函数式 `(_prev) => entry`；TranslatePage 的 setHistory/setResult/setError 均为直接值设置（不依赖前值，正确） |
| 注释写「为什么」 | 通过 | browser-api 模块注释说明隔离原因；save_history 分支注释说明去重理由；cancelled flag 注释说明防泄漏原因 |
| 无装饰性分隔注释 | 通过 | 无 ═══/─── 等横线 |
| 列表 key 用稳定 id | 通过 | TranslateHistoryPanel `key={item.id}`；TranslateWorkspace 操作按钮 `key={action}`（action 为枚举字面量，稳定） |
| 测试 AAA 结构 | 通过 | 所有 9 个测试有 Arrange/Act/Assert 注释 |
| 测试断言非恒真、非旁路 | 通过 | 断言具体文本值（`"你好世界"`）和调用参数（`toHaveBeenCalledWith("你好世界")`），无 `toBeDefined` 弱断言 |
| 错误不静默（主流程） | 通过 | handleTranslate catch 写 setError；历史取数失败静默（注释说明原因：不阻断主工作流） |
| 无 TODO/FIXME | 通过 | 无残留 |
| 安全：无密钥入库 | N/A | 本功能无敏感数据；translateText 入参为用户输入文本，不含凭证 |
