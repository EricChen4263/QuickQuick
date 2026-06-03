---
id: s15-hotkey-live-register
title: 改热键运行时即时生效（修「改了不生效」）
status: done
commit: pending
date: 2026-06-03
---

## 根因

设置页改热键后不生效（要重启 app 才行）。

铁证 `src-tauri/src/ipc/settings.rs` 的 `SystemHotkeyRegistrar::register`：注释自述「仅做冲突检测，不实际绑定回调（回调在 lib.rs setup 阶段统一绑定）」，实现只 `is_registered` 查冲突。全局快捷键回调**只在启动期** `register_hotkeys`（lib.rs）绑一次。

所以 `set_hotkey` 命令改键时：
- 新键**从未在运行时注册**（按了没反应）；
- 旧键**仍挂在系统**（旧键仍触发）；
- 只有重启读 hotkey.json 后，`register_hotkeys` 才注册新键。

与「暂停捕获/排除名单」等用 AtomicBool/RwLock 做到运行时即时生效的设置项对照，热键这条漏了「运行时即时应用」环节。与 s14 按键捕获无关，是早就存在的 backend 缺陷，改键变方便后暴露。

## 修法：改键运行时即时生效（注销旧键 + 注册新键并绑回调）

### `src-tauri/src/lib.rs`
- 新增纯函数 `popover_label_for_action(action) -> &'static str`（History→"clip-popover"、Translate→"trans-popover"），可单测。
- 新增可复用 `register_action_shortcut(handle, action, accelerator)`：按 action 绑定对应 popover 回调（`on_shortcut` → `trigger_popover`）。启动期与运行时改键共用，消除回调逻辑重复。
- `register_hotkeys` 重构为遍历两动作循环调用 `register_action_shortcut`，保持「注册失败仅 eprintln 降级不 panic」。

### `src-tauri/src/ipc/settings.rs`
`set_hotkey` 命令（`set_hotkey_impl` 纯函数保持不变，仍负责冲突检测+持久化）：
1. 调 `set_hotkey_impl` **之前**用 `get_hotkeys_impl(&path)` 读出该 action 的**旧加速键**。
2. `set_hotkey_impl(...)?` 持久化（失败提前返回，运行时不动，保证一致性）。
3. 成功后运行时即时应用：`old != new` 时 `unregister(old)`（「未注册」静默忽略），再 `register_action_shortcut(new)`，失败映射为 String 返回前端（此时已持久化，重启仍生效）。

## TDD 红绿
`popover_label_for_action` 两条映射测试：RED（函数不存在 E0425）→ GREEN（2 passed）。

运行时注册/注销是 Tauri runtime glue，与既有 `register_hotkeys` 同理不可纯单测——靠编译验证 + GUI 实测。

## 实跑输出摘要
```
# 后端全量
cargo test -p quickquick: 319 passed; 0 failed（tester 连跑 3 次无 flaky）
# 编译（关键：global_shortcut on_shortcut/unregister 签名）
cargo build -p quickquick: exit 0
# fmt / clippy
cargo fmt -p quickquick --check: exit 0
cargo clippy -p quickquick: 无新增 warning
```

## 已知边界 / 决策留痕
- **应用次序「先 unregister(old) 再 register(new)」**：若 register(new) 失败（仅 OS 级异常；新键已过冲突检测，正常不会失败），旧键已注销 → 该动作运行时两键皆失效**至重启**。tester + reviewer 均评估为**可接受**：失败概率极低、持久化已完成、错误已反馈前端、重启 `register_hotkeys` 自动补录。更保守的「先 register(new) 成功再 unregister(old)」留作未来防御选项，本次不做（YAGNI）。
- **re-save 完全相同的键**会被 `is_registered` 冲突检测误报「已被占用」——**既有行为**，本次未引入回归，未处理。
- **GUI 实测项**：改键后**不重启**直接按新键应弹对应 popover、旧键应失效。需用户验（改了 Rust，要重新 build）。
