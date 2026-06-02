---
id: s08-clip-autorefresh
title: 剪贴板界面自动刷新（事件驱动）测试留痕
status: passed
commit: fcfd997
date: 2026-06-02
---

# 测试留痕：剪贴板自动刷新（s08）

## 开工 git status 快照

```
 M src-tauri/src/lib.rs
 M src/panels/clipboard/ClipboardPage.tsx
 M src/panels/clipboard/clipboard-page.test.tsx
?? docs/dev-log/v5/f4-gui-fixes/s08-clip-autorefresh/
```

---

## 一、命中校验（杀假绿）

### 后端 4 个 should_notify_clip_change 测试

命令：
```
cd src-tauri && rtk proxy cargo test --lib should_notify_clip_change
```

关键输出（running 4 tests，test result: ok. 4 passed）：
```
running 4 tests
test tests::should_notify_clip_change_empty_returns_false ... ok
test tests::should_notify_clip_change_mixed_outcomes_returns_true ... ok
test tests::should_notify_clip_change_with_bumped_returns_true ... ok
test tests::should_notify_clip_change_with_inserted_returns_true ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 81 filtered out; finished in 0.00s
```

结论：4 个目标测试全部命中，非空匹配假绿。

### 前端新增测试（clipboard-page.test.tsx，共 18 个）

命令：
```
pnpm exec vitest run --reporter=verbose
```

新增测试的命中行：
```
✓ src/panels/clipboard/clipboard-page.test.tsx > clipboard-page > clipboard-page: 收到 clipboard-changed 事件后触发 listClipItems 重新加载
```

全套 18 个测试全部通过（Tests 356 passed 全量验证）。

---

## 二、变异 sanity（杀恒真/旁路）

### 后端变异

变异方式：`should_notify_clip_change` 实现从 `!outcomes.is_empty()` 改为恒 `true`。
备份命令：`cp src-tauri/src/lib.rs /tmp/lib.rs.bak`
改坏命令：`sed -i '' 's/!outcomes.is_empty()/true/' src-tauri/src/lib.rs`

变异后跑测试结果：
```
running 4 tests
test tests::should_notify_clip_change_with_bumped_returns_true ... ok
test tests::should_notify_clip_change_with_inserted_returns_true ... ok
test tests::should_notify_clip_change_mixed_outcomes_returns_true ... ok
test tests::should_notify_clip_change_empty_returns_false ... FAILED
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 81 filtered out; finished in 0.00s
```

如期变红：`should_notify_clip_change_empty_returns_false ... FAILED`。测试有真实判别力。

还原命令：`cp /tmp/lib.rs.bak src-tauri/src/lib.rs`
还原后验证：`test result: ok. 4 passed` ——全绿。

### 前端变异

变异方式：`ClipboardPage.tsx` 中 `listen("clipboard-changed", ...)` 改为 `listen("xxx", ...)`。
备份命令：`cp src/panels/clipboard/ClipboardPage.tsx /tmp/ClipboardPage.tsx.bak`
改坏命令：`sed -i '' 's/listen("clipboard-changed"/listen("xxx"/' ...ClipboardPage.tsx`

变异后跑测试结果（其他 17 个通过，新增测试变红）：
```
× src/panels/clipboard/clipboard-page.test.tsx > clipboard-page > clipboard-page: 收到 clipboard-changed 事件后触发 listClipItems 重新加载
  → expected "spy" to be called with arguments: [ 'clipboard-changed', Any<Function> ]
-   "clipboard-changed",
```

如期变红：新增测试真实检验了事件名，非旁路。

还原命令：`cp /tmp/ClipboardPage.tsx.bak src/panels/clipboard/ClipboardPage.tsx`
还原后 git status 与开工快照逐行一致，无残留。

---

## 三、事件名一致性核对

后端 emit（lib.rs:260）：
```rust
handle.emit("clipboard-changed", ())
```

前端 listen（ClipboardPage.tsx:101）：
```tsx
listen("clipboard-changed", () => { ... })
```

字面量完全一致：`"clipboard-changed"`。核对通过。

---

## 四、边界探测

### 1. emit 失败路径（后端）

lib.rs:260-262：`if let Err(e) = handle.emit(...) { eprintln!(...) }`。
emit 失败时打印到 stderr，不 panic、不崩溃。此路径未被单测覆盖（emit 是 Tauri runtime 行为，单测无法触发），属已知可接受范围，行为符合预期（优雅降级）。

### 2. listen 注册失败 catch（前端）

ClipboardPage.tsx:111-113：`.catch((err: unknown) => { console.error(...) })`。
注册失败时打印到 console.error 不崩溃。此路径未被前端测试覆盖（测试中 mockListen 始终 resolve）。属低优先级边界，行为符合预期。

### 3. 卸载时 unlisten 调用（前端）

ClipboardPage.tsx:114-117：cleanup 函数设 `cancelled.current = true` 并调用 `unlisten?.()`。
.then 中的 early-cancel 守卫（line 105-106）处理了"listen 注册完成前组件已卸载"的竞争场景（先调 fn() 立即 unlisten）。
结构完整，逻辑无漏洞，但卸载场景（unmount 触发 cleanup）未被测试显式覆盖。

### 4. IngestOutcome 变体完整性

db.rs 中 IngestOutcome 只有两个变体：`Inserted` 和 `Bumped`，4 个单测覆盖了所有变体（空、Inserted、Bumped、混合），无遗漏。

### 5. loadItems 依赖稳定性（前端）

`loadItems` 以 `useCallback(fn, [])` 定义（空依赖数组），引用稳定不变。
clipboard-changed 订阅 useEffect 的依赖数组为 `[loadItems]`，由于 loadItems 引用稳定，不会触发重复注册/注销，设计正确。

### 边界探测结论

未发现真实缺陷。以上未覆盖路径（emit 失败、listen 注册失败、unlisten 场景）属低优先级，行为均优雅降级，不构成回交条件。

---

## 五、最终结论

**放行（PASS）**

- 后端 4 个单测命中且有判别力（变异即红）
- 前端新增测试命中且有判别力（变异即红）
- 事件名两端完全一致 `"clipboard-changed"`
- 边界无真实缺陷
- 工作树还原干净，与开工快照逐行一致

---

## 结束时 git status --short

```
 M src-tauri/src/lib.rs
 M src/panels/clipboard/ClipboardPage.tsx
 M src/panels/clipboard/clipboard-page.test.tsx
?? docs/dev-log/v5/f4-gui-fixes/s08-clip-autorefresh/
```

与开工快照完全一致，无新增/残留业务代码改动。
