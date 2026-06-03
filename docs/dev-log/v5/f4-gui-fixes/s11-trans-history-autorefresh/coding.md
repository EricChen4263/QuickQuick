---
id: s11-trans-history-autorefresh
title: 翻译历史栏自动刷新（事件驱动）
status: done
commit: 25297c7
date: 2026-06-03
---

## 根因

快捷翻译（Cmd+Shift+T 唤出的 trans-popover）翻译完，主窗「翻译页」的历史栏不出现新记录，用户误以为「快捷翻译的文本没存进历史」。

实际上后端 `translate_text` 命令的 `translate_text_impl`（`src-tauri/src/ipc/translate.rs:193`）翻译成功后**无条件**调用 `add_translate_history` 写库——记录确实存进了 `translate_history` 表。真正的毛病是**前端历史栏不刷新**：`TranslatePage` 只在组件挂载、以及在主面板内手动翻译后（`handleTranslate` 直接 `fetchHistory`）才读历史，之后不监听任何通知；后端写历史也不 emit 任何事件。

对照剪贴板链路（s08）：后端写库后会 emit `clipboard-changed`，前端 `ClipboardPage` listen 后重读列表——翻译历史这条链路缺失这个环节。本次即补上等价机制。

## 最终事件名

`translate-history-changed`

（与 s08 `clipboard-changed`、s10 事件名抽常量约定一致：不含特殊字符、两端各定义一处具名常量、注释互指。）

## 改动文件

### `src/ipc/events.ts`

新增 `export const TRANSLATE_HISTORY_CHANGED_EVENT = "translate-history-changed" as const;`，与既有 `CLIPBOARD_CHANGED_EVENT` 并列，注释注明「与后端 `src-tauri/src/ipc/translate.rs` 同名常量必须一致，改动需两端同步」。

### `src-tauri/src/ipc/translate.rs`

1. `use tauri::State;` 扩展为 `use tauri::{AppHandle, Emitter, State};`（引入 `AppHandle` 类型与 `Emitter` trait，使 `app.emit` 可见）。
2. 新增模块级常量 `const TRANSLATE_HISTORY_CHANGED_EVENT: &str = "translate-history-changed";`，doc 注释注明与前端必须一致（仿 lib.rs 的 `CLIPBOARD_CHANGED_EVENT` 注释风格）。
3. `#[tauri::command] translate_text` 命令：
   - 签名新增 `app: AppHandle` 参数（Tauri 按类型自动注入，不依赖参数位置）。
   - `with_db(...)` 的结果先存入 `result`；仅当 `result.is_ok()`（翻译成功）时 `app.emit(TRANSLATE_HISTORY_CHANGED_EVENT, ())`，emit 失败仅 `eprintln!` 记录不 panic、不影响翻译结果返回（仿 lib.rs:270 优雅降级）。
   - **翻译失败（Err）不 emit**——避免历史栏无谓空刷。
   - `translate_text_impl` 纯函数保持原样不动（仍负责写库），emit 只在命令层做。

### `src/panels/translate/TranslatePage.tsx`

1. 新增 `import { listen } from "@tauri-apps/api/event"` 与 `import { TRANSLATE_HISTORY_CHANGED_EVENT } from "../../ipc/events"`。
2. 在挂载 useEffect 之后新增订阅 useEffect，**严格复用 `ClipboardPage` 订阅 `clipboard-changed` 的 cancelled+unlisten 范式**：
   - `listen(TRANSLATE_HISTORY_CHANGED_EVENT, () => { void fetchHistory(cancelled); })`
   - `.then` 中早取消守卫（`cancelled.current` 为真时立即 `fn()` unlisten，防「注册完成前已卸载」竞态）。
   - `.catch` 中 `console.error` 记录注册失败。
   - cleanup：置 `cancelled.current = true` 并调 `unlisten?.()`。
   - deps：`[fetchHistory]`。`fetchHistory` 为 `useCallback(fn, [])` 稳定引用，effect 只在挂载/卸载各跑一次，无重订阅抖动（tester + reviewer 双重确认）。

### `src/panels/translate/translate-page.test.tsx`

1. 顶部新增 `vi.mock("@tauri-apps/api/event", ...)` + `import { listen }` + `import { TRANSLATE_HISTORY_CHANGED_EVENT }` + `mockListen`。
2. 新增测试「收到 translate-history-changed 事件后触发 listTranslateHistory 重新加载」：
   - `mockListen.mockImplementation` 捕获注册的回调。
   - 等待挂载时初始 `listTranslateHistory`（第 1 次）。
   - 断言 `mockListen` 以 `TRANSLATE_HISTORY_CHANGED_EVENT` + 任意 Function 注册。
   - 手动触发捕获的回调，断言 `listTranslateHistory` 被调用第 2 次。

## TDD 红绿过程

**RED**：先写前端新增测试，`TranslatePage` 尚未订阅任何事件，`mockListen` 调用次数 0 →
`AssertionError: expected "spy" to be called with arguments: [ undefined, Any<Function> ] — Number of calls: 0`。错误类型正确（功能未实现，非语法/环境错误）。

**GREEN**：实现 4 处改动后，前端测试转绿。

后端无新增单测：emit 是命令层薄胶水、依赖 Tauri runtime/AppHandle，无法纯单测（与 s08 emit 同理）；emit 为「成功即发」无决策分支，不存在可抽取的纯函数接缝（抽 `fn should_notify() -> true` 属 YAGNI）。`translate_text_impl` 既有单测已覆盖写库逻辑，保持全绿。

## 实跑测试输出摘要

```
# 前端全量
Test Files  43 passed (43)
      Tests  364 passed (364)

# 后端全量
cargo test -p quickquick: 67 passed; 0 failed + doc-tests 1 passed

# TypeScript
pnpm tsc --noEmit: No errors

# 格式
cargo fmt -p quickquick --check: exit 0

# Clippy
cargo clippy -p quickquick: No issues
```
