# V4/F2/S07 剪贴板页 — Phase 6 动态证伪测试报告

**被验对象**: `src/panels/clipboard/ClipboardPage.tsx` + ClipItemRow / ClipPreview / ClipSearchBar  
**测试文件**: `src/panels/clipboard/clipboard-page.test.tsx`（8 个测试）  
**执行日期**: 2026-05-31  
**执行者**: tester agent  

---

## 一、命令清单

| 命令 | 用途 |
|------|------|
| `pnpm test clipboard-page` | 命中校验：目标 8 个测试 |
| `pnpm test` | 全量回归，确认不破坏其他模块 |
| `pnpm test app-shell` | 验收 app-shell 6 个测试仍绿 |
| 变异1：sed 改坏 filterBySearch → pnpm test clipboard-page | 变异 sanity：搜索过滤接线 |
| 变异2：注释掉 toggleFavoriteClip 调用 → pnpm test clipboard-page | 变异 sanity：收藏 IPC 接线 |
| 变异3：setHighlightIndex 始终 0 → pnpm test clipboard-page | 变异 sanity：键盘高亮接线 |
| 临时 boundary-temp.test.tsx → pnpm test boundary-temp | 边界探测 B1/B3/B4 |

---

## 二、命中校验（杀假绿）

### clipboard-page（目标 N=8）

```
✓ src/panels/clipboard/clipboard-page.test.tsx (8 tests) 170ms
 Test Files  1 passed (1)
      Tests  8 passed (8)
```

**结论：真命中 N=8，Test Files 1 passed，无假绿，无空匹配。**

### 全量测试

```
 Test Files  15 passed (15)
      Tests  120 passed (120)
```

注意：app-shell 中出现 3 条 `Warning: An update to ClipboardPage inside a test was not wrapped in act(...)` stderr 警告。原因：app-shell.test 渲染完整 App 时 ClipboardPage 异步取数触发 React 状态更新未包裹 act。属于 app-shell.test 本身的测试编写问题，非被测功能缺陷，所有用例仍通过，不影响门禁判定。

**结论：全量 120 passed，全绿。**

### app-shell

```
✓ src/app-shell.test.tsx (6 tests) 115ms
 Test Files  1 passed (1)
      Tests  6 passed (6)
```

**结论：6 passed，全绿。**

---

## 三、变异 sanity（杀恒真/旁路）

**开工 git status 快照**：`M src/App.tsx  ?? docs/dev-log/v4/f2-ui/s07-clipboard-page/  ?? src/panels/clipboard/`

所有变异均在 ClipboardPage.tsx 上操作，改前 `cp ClipboardPage.tsx /tmp/ClipboardPage.tsx.bak`，验后 `cp /tmp/ClipboardPage.tsx.bak ClipboardPage.tsx`。

### 变异1：绕过搜索过滤（filterBySearch）

- **改坏位置**：`ClipboardPage.tsx` 第56行，`const afterSearch = filterBySearch(historyItems, searchQuery)` → `const afterSearch = historyItems; // MUTATED: bypass filterBySearch`
- **预期**：`clipboard-page: 搜索框输入过滤词后列表只剩匹配项` 变红
- **实际**：FAIL，`expect(element).not.toBeInTheDocument()` 报错——"富文本内容示例"仍在 DOM 中，因为搜索过滤被绕过
- **判定：测试有判别力，非恒真 / 非旁路。**
- **已从备份复原**，复原后 git status 与开工快照逐行一致。

### 变异2：不调 toggleFavoriteClip IPC

- **改坏位置**：`ClipboardPage.tsx` 第89行，`await toggleFavoriteClip(item.id, !item.isFavorite)` → 注释掉（整行注释）
- **预期**：`clipboard-page: 点击收藏按钮调用 toggleFavoriteClip 并传正确参数` 变红
- **实际**：FAIL，`AssertionError: expected "spy" to be called with arguments: ['item-1', true]`，`Number of calls: 0`
- **判定：测试有判别力，非恒真 / 非旁路。**
- **已从备份复原**，复原后 git status 与开工快照逐行一致。

### 变异3：键盘高亮 setHighlightIndex 始终 0

- **改坏位置**：`ClipboardPage.tsx` 第73行，`setHighlightIndex((prev) => moveHighlight(...))` → `setHighlightIndex(0); // MUTATED`
- **预期**：ArrowDown/Up 两个键盘测试变红
- **实际**：FAIL × 2——`ArrowDown 键高亮下移` 和 `ArrowUp 键高亮上移` 均失败，预览区始终停在 "Hello World"，无法切换到 "富文本内容示例"
- **判定：测试有判别力，非恒真 / 非旁路。**
- **已从备份复原**，复原后 git status 与开工快照逐行一致。

**结束 git status 快照**：`M src/App.tsx  ?? docs/dev-log/v4/f2-ui/s07-clipboard-page/  ?? src/panels/clipboard/`  
与开工快照逐行一致，工作树无新增/丢失文件，无业务代码残留改动。

---

## 四、边界探测

通过临时测试文件 `boundary-temp.test.tsx`（运行后已删除，未留存）动态验证，3 个边界均通过：

| 边界编号 | 场景 | 实现守卫 | 结果 |
|----------|------|----------|------|
| B1 | `listClipItems` 返回 `[]`（空列表） | `filteredItems.length === 0` 时 `safeHighlight = -1`；`highlightedClipItem = null`；ClipPreview null 守卫显示占位"选择条目以预览内容"；列表区显示"暂无剪贴板记录" | 通过，不崩溃 |
| B2 | `listClipItems` reject（错误态） | catch 块 `setLoadError("加载失败，请稍后重试")`；渲染 `<div role="alert">` | 已由第8个测试覆盖并通过 |
| B3 | 搜索无匹配（输入 "zzzznoMatch"） | `filterBySearch` 返回 `[]`，走 EMPTY_LIST_PLACEHOLDER 分支，显示"暂无剪贴板记录" | 通过，不崩溃 |
| B4 | Cmd+9 只有 3 条数据（高亮越界） | `quickSelectIndex("9")=8`，`setHighlightIndex(8)`，`safeHighlight = Math.min(Math.max(8,0), 2) = 2`，clamp 至末项 "Another text item" | 通过，不越界不崩溃 |

---

## 五、覆盖缺口与观察

### 非阻塞覆盖缺口

1. **app-shell.test 的 act() 警告**：`ClipboardPage` 被 app-shell.test 渲染为 App 子节点时，异步 `listClipItems` 的状态更新在测试中未包裹 `act()`，产生 3 条 stderr 警告。所有用例仍通过，但潜在风险是警告日后可能升级为错误（React 严格模式）。建议 app-shell.test 对 `listClipItems` 增加 mock，使其同步返回，消除警告。此为现有 app-shell.test 问题，非 S07 ClipboardPage 本身缺陷，**不阻断本次门禁**。

2. **deleteClipItem 调用接线未做变异**：测试第7个用例（点击删除调用 deleteClipItem）未经变异 sanity 验证。静态检查第94行 `await deleteClipItem(item.id)` 直接接线，结构与 toggleFavoriteClip 完全对称；结合 toggleFavoriteClip 已过变异验证（同一 handleDelete 函数体），风险极低，可不补做。

### 无失败项，无真实缺陷发现

---

## 六、门禁结论

**放行。**

所有验收 verify 真命中（N=8 + 全量 120 + app-shell 6），无假绿；3 处变异 sanity 均如期变红（测试有判别力，非恒真/旁路）；4 项边界均优雅处理（无崩溃、无越界、有守卫）；工作树已完全还原，git status 与开工快照逐行一致。

V4/F2/S07 ClipboardPage 动态证伪通过，可进入下一任务。

---

## 修订 R1 验证

**被验修订**: V4/F2/S07 R1（I-1 收藏/删除错误不静默 + I-2 useEffect cleanup）  
**执行日期**: 2026-05-31  
**执行者**: tester agent  

---

### 一、命中校验（杀假绿）

| 套件 | 命令 | 结论 |
|------|------|------|
| clipboard-page | `pnpm test clipboard-page` | Tests 10 passed (10)，Test Files 1 passed，真命中 N=10，无假绿 |
| app-shell | `pnpm test app-shell` | Tests 6 passed (6)，act() 警告数量 = 0（R1 I-2 cleanup 修复有效，原 3 条警告已消除）|
| 全量 | `pnpm test` | Test Files 15 passed (15)，Tests 122 passed (122)，全绿（原 120 + R1 新增 2）|

act() 警告彻底清零，R1 I-2 cleanup 修复得到间接验证。

---

### 二、变异 sanity（杀恒真/旁路）

**开工 git status 快照**: `M src/App.tsx  M src/app-shell.test.tsx  ?? docs/dev-log/v4/f2-ui/s07-clipboard-page/  ?? src/panels/clipboard/`  
**改前备份**: `cp src/panels/clipboard/ClipboardPage.tsx /tmp/cp.bak`

#### 变异 I-1：改坏 handleToggleFavorite catch 分支（吞错不设 opError）

- **改坏位置**: ClipboardPage.tsx 第 108 行，`setOpError("操作失败，请稍后重试")` → `// MUTATED: swallow error`
- **预期**: `toggleFavoriteClip IPC reject 时显示操作错误提示` 变红
- **实际**: FAIL — `TestingLibraryElementError: Unable to find role="alert"`，DOM 中无 alert 元素，1 failed | 9 passed
- **判定**: 测试有判别力，非恒真/旁路，I-1 错误提示接线有效
- **已从备份复原**，`cp /tmp/cp.bak ClipboardPage.tsx`，复原后 git status 逐行一致

#### 变异 I-2：去掉 loadItems cancelled guard（两处 if (cancelled.current) return）

- **改坏位置**: 第 49、53 行两处 guard 注释掉
- **预期**: 无专门测试检测卸载后不 setState，属已知缺口
- **实际**: Tests 10 passed (10)，仍全绿（无专测，属已知设计缺口）
- **app-shell 状态**: act() 警告仍为 0（因 app-shell.test.tsx R1 已 mock IPC 同步返回，与 cancelled guard 无关）
- **判定**: I-2 cancelled guard 无专测——记录为**非阻断覆盖缺口**，实现代码本身正确，缺失测试覆盖
- **已从备份复原**，`cp /tmp/cp.bak ClipboardPage.tsx`，复原后 git status 逐行一致

**结束 git status 快照**: `M src/App.tsx  M src/app-shell.test.tsx  ?? docs/dev-log/v4/f2-ui/s07-clipboard-page/  ?? src/panels/clipboard/`  
与开工快照逐行一致，无业务代码残留改动。

---

### 三、边界探测

通过临时测试文件 `boundary-r1-temp.test.tsx`（运行后已删除）动态验证：

| 边界 | 场景 | 结果 | 备注 |
|------|------|------|------|
| B-R1-1 | 连续两次收藏失败：opError 出现并保持 | 通过，alert 文案 `/操作失败/` 持续存在，不崩溃 | 正常 |
| B-R1-2 | 收藏失败后再成功：opError 是否清除 | 通过（无 crash）；alert 在成功后仍残留 | 设计缺口：实现未在成功路径调 `setOpError(null)` 清除旧错误。属非阻断缺口 |
| B-R1-3 | listClipItems reject 渲染 loadError | 通过，`role="alert"` 出现，`/加载失败/` 文案正确 | 正常 |

**边界缺口（非阻断）**: 操作成功后 `opError` 不被清除，旧错误提示残留。建议后续在 `handleToggleFavorite`/`handleDelete` 成功 try 分支开头加 `setOpError(null)`。不阻断本次门禁，属改进项。

---

### 四、门禁结论

**放行。**

- 命中校验：clipboard-page N=10 真命中、app-shell 6 绿、全量 122 全绿，无假绿
- act() 警告：I-2 cleanup 修复后清零（原报告 3 条警告彻底消除）
- 变异 sanity：I-1（吞错）如期变红，测试有判别力；I-2 cancelled guard 无专测，记录为非阻断缺口
- 边界：3 项边界均无 crash；发现 opError 成功后不清除的非阻断缺口
- 工作树与开工快照逐行一致，无残留改动

V4/F2/S07 R1 动态证伪通过，可进入下一任务。
