---
id: s08-clip-autorefresh
title: 剪贴板列表自动刷新（事件驱动）
status: done
commit: PENDING
date: 2026-06-02
---

## 根因

剪贴板界面打开后不自动刷新。后端轮询线程（500ms）已将新条目写入 SQLite，但前端
`ClipboardPage` 只在组件挂载时调用一次 `loadItems`，之后不监听任何通知。后端写库
成功后没有通知前端，前端也没有订阅任何事件，导致列表静止不更新。

## 最终事件名

`clipboard-changed`

（备注：方案文档原提议 `clipboard://changed`，经验证 Tauri 2 对事件名中的 `://`
不报错，但为保持命名一致、与项目其他事件（如 `route`）风格统一，改用不含特殊字符
的 `clipboard-changed`。两端同步使用此名称。）

## 改动文件

### `src-tauri/src/lib.rs`

1. `use tauri::{...}` 补入 `Emitter` trait，使 `AppHandle::emit` 在编译时可见。
2. 新增纯函数 `should_notify_clip_change(outcomes: &[db::IngestOutcome]) -> bool`：
   - 位置：`use` 块紧后，`start_clipboard_poll` 函数之前。
   - 逻辑：`!outcomes.is_empty()`——Inserted 与 Bumped 均代表列表内容/顺序变化，
     空切片（无剪贴板变化）不触发通知。
   - 设计为纯函数，便于单元测试，不依赖 AppHandle。
3. `start_clipboard_poll` 的 `Ok(outcomes)` 分支：调用 `should_notify_clip_change`，
   为真时执行 `handle.emit("clipboard-changed", ())`；失败仅 `eprintln!` 不 panic。

### `src/panels/clipboard/ClipboardPage.tsx`

1. 新增 `import { listen } from "@tauri-apps/api/event"`。
2. 在挂载 useEffect 之后新增一个 useEffect，严格复用 `App.tsx` 的
   `cancelled + unlisten` 范式订阅 `clipboard-changed` 事件：
   - `listen("clipboard-changed", () => { loadItems(cancelled); })`
   - `.then` 中判断 `cancelled.current` 防止卸载竞态。
   - `.catch` 中 `console.error` 记录注册失败。
   - cleanup：置 `cancelled.current = true`，调用 `unlisten?.()`。
   - deps：`[loadItems]`（`loadItems` 是 `useCallback([])` 稳定引用，不会反复重订阅）。

### `src/panels/clipboard/clipboard-page.test.tsx`

1. 顶部新增 `vi.mock("@tauri-apps/api/event", ...)` 和 `import { listen }` + `mockListen`。
2. 新增测试：**"收到 clipboard-changed 事件后触发 listClipItems 重新加载"**：
   - 通过 `mockListen.mockImplementation` 捕获注册的回调函数。
   - 等待挂载时的初始 `listClipItems` 调用（第 1 次）。
   - 断言 `mockListen` 以 `"clipboard-changed"` 和任意 Function 被调用。
   - 手动触发捕获的回调，断言 `listClipItems` 被调用第 2 次。

## TDD 红绿过程

### 后端

**RED**：在 `lib.rs` tests 模块添加 4 个 `should_notify_clip_change` 测试，函数未实现，
`cargo test should_notify_clip_change` 报 `error[E0425]: cannot find function`，exit:101。

**GREEN**：实现 `should_notify_clip_change` + 添加 `Emitter` import + 在 `Ok(outcomes)`
分支调用 `emit`。`cargo test should_notify_clip_change` → 4 passed。

全量 `cargo test` → 301 passed，0 failed。

### 前端

**RED**：测试文件添加 `listen` mock 与新测试，`pnpm test` 报
`AssertionError: expected "spy" to be called with arguments: ['clipboard-changed', Any<Function>]`，
因 ClipboardPage 尚未订阅事件。

**GREEN**：ClipboardPage 添加 `listen` import 与订阅 useEffect。
`pnpm test` → 356 passed（43 test files），0 failed。

## 实跑测试输出摘要

```
# 后端 should_notify_clip_change 专项
cargo test: 4 passed, 296 filtered out (23 suites, 0.00s)

# 后端全量
cargo test: 301 passed (24 suites, 6.55s)

# 前端全量
Test Files  43 passed (43)
      Tests  356 passed (356)

# cargo check
exit:0  (无 warning)

# pnpm tsc --noEmit
TypeScript: No errors found
```
