---
id: s10-event-const
title: "重构：clipboard-changed 事件名常量化（消除跨文件字面量重复）"
status: done
commit: PENDING
date: 2026-06-02
---

## 重构动机

code-reviewer 标记技术债 I-01：事件名字符串 `"clipboard-changed"` 在三处各以字面量硬编码：

- 后端 `src-tauri/src/lib.rs:260`：`handle.emit("clipboard-changed", ())`
- 前端 `src/panels/clipboard/ClipboardPage.tsx:112`：`listen("clipboard-changed", ...)`
- 测试 `src/panels/clipboard/clipboard-page.test.tsx:475`：断言 `listen` 以 `"clipboard-changed"` 调用

字面量散布意味着重命名事件时需人工同步多处，漏改任一处会造成运行时静默失效。目标是每种语言内建立单一来源（single source of truth），消除同语言内的重复。

## 为什么跨语言只能做"两端注释互指"而非真正共享

Tauri 事件名是运行时字符串，Rust 与 TypeScript 编译体系完全独立，没有机制在编译期强制两端保持一致：

- Rust 的 `const &str` 与 TS 的 `as const` 均属各自语言的编译期常量，无法互相引用。
- 即便通过构建脚本（`build.rs`）生成 TS 类型文件，也只能在构建时同步，而非编译期约束，并且引入额外构建复杂性，超出纯重构范围。

因此本次方案是务实的折中：**每种语言内只定义一处**，并在两处常量的文档注释中互相指向对方，配合 code-review 检查单提醒未来改动需两端同步。

## 改动文件

| 文件 | 变更内容 |
|---|---|
| `src/ipc/events.ts`（新建）| 定义前端常量 `CLIPBOARD_CHANGED_EVENT = "clipboard-changed" as const`，含注释互指后端 |
| `src/panels/clipboard/ClipboardPage.tsx` | import `CLIPBOARD_CHANGED_EVENT`，替换 `listen("clipboard-changed", ...)` 中的字面量 |
| `src/panels/clipboard/clipboard-page.test.tsx` | import 同一常量，替换断言中的 `"clipboard-changed"` 字面量（测试与实现共享同一来源） |
| `src-tauri/src/lib.rs` | 新增 `const CLIPBOARD_CHANGED_EVENT: &str = "clipboard-changed"`，含注释互指前端；替换 `emit` 调用及错误日志中的字面量 |

事件名字面量值 `"clipboard-changed"` 不变，纯结构重构，零行为变化。

## 实跑输出摘要

```
pnpm test       : Test Files 43 passed (43) | Tests 358 passed (358)
pnpm tsc --noEmit : TypeScript: No errors found
cargo check     : Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.96s
cargo test      : 301 passed (24 suites, 7.30s)
```
