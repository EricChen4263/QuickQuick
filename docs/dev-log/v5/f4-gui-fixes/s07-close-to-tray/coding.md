---
id: f4-gui-fixes-s07
title: 修复关闭按钮直接退出（改为隐藏到后台）
status: 实现完成
commit: PENDING
date: 2026-06-02
---

## 根因

`setup_window_focus_hide` 只处理了 `WindowEvent::Focused(false)`（失焦），未处理 `WindowEvent::CloseRequested`，导致点红绿灯 X 走 Tauri 默认行为（销毁最后一个窗口 → 退出 app）。

## 改法

将 `setup_window_focus_hide` 重命名为 `setup_main_window_behavior`，在同一 `on_window_event` 闭包内用 `match` 增加 `CloseRequested` 分支：`stay_in_tray == true` 时调用 `api.prevent_close()` 拦截关闭 + `win.hide()` 隐藏到后台；`stay_in_tray == false` 时不拦截，放行默认退出行为，与失焦分支语义一致。托盘「退出」菜单直接调用 `app_handle().exit(0)`，不经过此事件，两者解耦。调用点（lib.rs 第 140 行）同步更新为新函数名。

## 编译与测试结果

- `cargo check`：exit 0，无 error，无 warning
- `cargo build`：exit 0，`Finished dev profile`
- `cargo test`：exit 0，全绿（15 个 test suite，合计 168 passed, 0 failed）

## GUI 行为需用户实测

窗口事件回调无法单测（需 GUI 运行时）。**请重启 app 后实测**：
1. 点主窗口关闭按钮（红绿灯 X）→ 应隐藏窗口、app 继续在托盘运行。
2. 点托盘图标「退出」→ 应真正退出 app。
