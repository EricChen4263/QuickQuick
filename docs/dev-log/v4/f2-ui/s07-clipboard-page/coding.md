# S07 剪贴板页 — 编码留痕

- 小功能：S07-clipboard-page
- 大功能：F2 三页 React UI
- 版本：V4
- 执行者：coder agent (Phase 5)
- 完成时间：2026-05-31

## 1. 改动文件清单

| 文件 | 改动说明 |
|------|----------|
| `src/panels/clipboard/ClipboardPage.tsx` | 新建：剪贴板页根组件，双栏布局，IPC 取数，搜索/筛选/键盘流/收藏/删除 |
| `src/panels/clipboard/ClipSearchBar.tsx` | 新建：搜索栏子组件（搜索框 + 类型筛选下拉） |
| `src/panels/clipboard/ClipItemRow.tsx` | 新建：列表单行子组件（摘要 + 收藏标记 + 操作按钮） |
| `src/panels/clipboard/ClipPreview.tsx` | 新建：右侧预览子组件（完整内容 + 空占位） |
| `src/panels/clipboard/clipboard-page.test.tsx` | 新建：8 个渲染测试，覆盖取数渲染/搜索/类型筛选/键盘流/收藏/删除/错误态 |
| `src/App.tsx` | 改：page-clipboard section 内容替换为 `<ClipboardPage/>`；route listen 链尾补 `.catch` |

## 2. 关键实现决策

### 2.1 双栏结构

左栏固定宽 320px（搜索栏 + 列表），右栏 flex:1 展示预览。左栏内部搜索栏固定，列表区 overflow-y: auto 可滚动。

### 2.2 ClipItem → HistoryItem 适配

`toHistoryItem()` 纯函数做一次性映射：`text = clip.content`，`kind = clip.kind === "richtext" ? "richtext" : "text"`（兼容后端可能扩展的其他 kind 值）。过滤逻辑完全复用 `history/search.ts` 和 `history/filter.ts`，无重造。

### 2.3 复用 search / filter / keyboard

- `filterBySearch(historyItems, searchQuery)` 做搜索词过滤
- `filterByType(afterSearch, typeFilter)` 做类型筛选，两步管道串联
- `moveHighlight` / `quickSelectIndex` / `resolveEnter` 全量复用 keyboard.ts

### 2.4 刷新策略

`loadItems` 用 `useCallback` 包裹（依赖数组为空，函数稳定）。收藏/删除操作后调 `await loadItems()` 重新取全量列表，保证数据与后端一致，无乐观更新复杂度。

### 2.5 App.tsx 接入 + route catch 补丁

- `page-clipboard` section 内容替换为 `<ClipboardPage/>`，保留 `data-testid="page-clipboard"` 和 `display` 控制——app-shell 测试契约不破。
- `listen(...).then(...)` 链尾补 `.catch((err: unknown) => { console.error("[QuickQuick] route 监听注册失败:", err); })`，对称于已有的 hide().catch，错误不吞。

### 2.6 函数拆分策略

主组件 `ClipboardPage` 约 90 行（含空行/注释），拆为：
- `ClipSearchBar`：搜索框 + 筛选下拉，接收 props 无状态
- `ClipItemRow`：单行渲染，`truncateSummary` 纯函数
- `ClipPreview`：预览区，条件渲染

## 3. 假设 / 未决 / 需用户确认

- **Enter 粘贴回写**：`resolveEnter` 已调用取出选中项，实际写回剪贴板（§8）归 manual 任务，本版仅做预览强调。
- **视觉还原 A10**：布局骨架已建，精确视觉还原待 manual 阶段。
- **收藏排序**：后端 `listClipItems` 已保证收藏优先，前端无需重排。
- **高亮索引跨过滤重置**：搜索词/类型筛选变化时重置 `highlightIndex = 0`，避免越界。

## 4. 测试证据

### clipboard-page 命中测试（冻结 verify：`pnpm test clipboard-page`）

```
✓ clipboard-page: 挂载后调用 listClipItems 并渲染所有条目内容
✓ clipboard-page: 搜索框输入过滤词后列表只剩匹配项
✓ clipboard-page: 类型筛选选 richtext 后只剩 richtext 条目
✓ clipboard-page: ArrowDown 键高亮下移，右侧预览内容随之变化
✓ clipboard-page: ArrowUp 键高亮上移，右侧预览内容随之变化
✓ clipboard-page: 点击收藏按钮调用 toggleFavoriteClip 并传正确参数
✓ clipboard-page: 点击删除按钮调用 deleteClipItem 并传正确 id
✓ clipboard-page: listClipItems 失败时显示错误提示而非崩溃

Tests  8 passed (8)
```

### app-shell 命中测试（接入后，冻结 verify：`pnpm test app-shell`）

```
Tests  6 passed (6)
```

无需在 app-shell.test.tsx 补 ipc-client mock：ClipboardPage 挂载时 listClipItems 调 invoke，
jsdom 环境下 invoke 返回 rejected Promise，ClipboardPage catch 后显示错误 UI，不影响 app-shell
的导航/可见性断言。

### 全量回归（`pnpm test`）

```
Test Files  15 passed (15)
     Tests  120 passed (120)
```

已完成 App.tsx 接入。

## 5. code-standards 自检

| 规范项 | 状态 | 说明 |
|--------|------|------|
| 格式：2 空格缩进、无 Tab | 通过 | 所有新文件均 2 空格 |
| 函数 ≤50 行、嵌套 ≤3 层 | 通过 | 各子组件单职责；handleKeyDown/handleToggleFavorite 均 ≤15 行 |
| 命名：camelCase / PascalCase | 通过 | toHistoryItem、EMPTY_LIST_PLACEHOLDER、SUMMARY_MAX_LENGTH 等具名常量 |
| 禁 any | 通过 | 无 any，ClipItem/HistoryItem/HistoryFilter 均显式类型 |
| setState 函数式更新 | 通过 | setHighlightIndex(prev => ...) 等函数式更新 |
| 注释写「为什么」 | 通过 | 适配函数、刷新策略、clamp 逻辑有注释 |
| 无装饰性分隔注释 | 通过 | 无 ═══/─── 等横线 |
| 测试 AAA 结构 | 通过 | 所有测试有 Arrange/Act/Assert 注释 |
| 测试断言非恒真 | 通过 | 断言具体文本/调用参数（getAllByText/toHaveBeenCalledWith） |
| 无 TODO/FIXME | 通过 | 无残留 |
| 安全：无密钥入库 | N/A | 本功能无敏感数据 |

## 修订 R1（reviewer I-1/I-2）

执行者：coder agent（Phase 5 修订）
时间：2026-05-31

### 改动说明

**I-2 修复：loadItems useEffect 加 cancelled flag cleanup**

- `loadItems` 签名改为 `async (cancelled: { current: boolean })`，在 `setItems` / `setLoadError` 前加 `if (cancelled.current) return` guard。
- `useEffect` 内声明 `const cancelled = { current: false }`，cleanup 返回 `() => { cancelled.current = true; }`，确保卸载后 async resolve 不写入已卸载组件 state。
- `handleToggleFavorite` / `handleDelete` 中刷新调 `loadItems` 时传入局部 `cancelled`（操作期间组件不卸载，cancelled 始终 false，保留 guard 结构一致性）。

**I-1 修复：handler 错误不再静默**

- `handleToggleFavorite` / `handleDelete` 加 `try/catch`：成功路径 await IPC + 刷新列表；catch 分支调 `setOpError("操作失败，请稍后重试")`。
- 新增 `opError` state（`string | null`），在主 JSX return 顶部渲染 `role="alert"` div，让用户看到失败而非误以为成功。
- 两个 handler 返回类型显式标注为 `Promise<void>`；`ClipItemRow` 对应 props 类型同步更新为 `(item: ClipItem) => Promise<void>`。

**act 警告消除：app-shell.test.tsx 补 IPC mock**

- 原 coding.md 注释「无需补 ipc-client mock」是错误判断，app-shell 测试实际存在 ClipboardPage 挂载后 listClipItems reject 触发 act 外 state 更新的警告（原始代码无 ClipboardPage 时 0 警告，S07 引入后产生）。
- 修法：在 `app-shell.test.tsx` 顶层加 `vi.mock("./ipc/ipc-client", ...)` 让 `listClipItems` 返回永远 pending 的 Promise，彻底避免异步 state 更新进入 act 边界外。app-shell 测试只验证导航结构，不关心剪贴板数据。

### 补测（新增 2 个，位于 clipboard-page.test.tsx）

- `clipboard-page: toggleFavoriteClip IPC reject 时显示操作错误提示`：mock `toggleFavoriteClip` reject，点击收藏按钮后断言 `role="alert"` 出现且文本匹配 `/操作失败|失败/`。
- `clipboard-page: deleteClipItem IPC reject 时显示操作错误提示`：mock `deleteClipItem` reject，点击删除按钮后断言 `role="alert"` 出现且文本匹配 `/操作失败|失败/`。

### 各测试结论

| 套件 | 命令 | 结论 |
|------|------|------|
| clipboard-page | `pnpm test clipboard-page` | Tests 10 passed (10)（8 既有 + 2 新） |
| app-shell | `pnpm test app-shell` | Tests 6 passed (6)；act 警告 0 条 |
| 全量回归 | `pnpm test` | Test Files 15 passed (15)；Tests 122 passed (122) |
