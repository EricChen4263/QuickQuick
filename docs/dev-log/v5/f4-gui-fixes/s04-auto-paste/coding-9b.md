---
id: f4-gui-fixes-s04-9b
title: 真实自动粘贴 9b 接入与焦点编排
status: 实现完成
commit: 143ff71
date: 2026-06-02
---

## 改动摘要

**`src-tauri/src/ipc/system.rs`**：把 `paste_to_front` 从固定 write_back_only 改为走真实后端。

- 新增 `fetch_paste_item(conn, id) -> Result<CapturedItem, DbError>`：DB 查询 + 校验（空 id / 图片拒绝）
- 新增 `map_outcome(PasteOutcome) -> &'static str`：FullPasteDone→"full_paste"，WriteBackOnlyDone→"write_back_only"
- 新增 `paste_orchestrate(probe, backend, item) -> String`：调 `perform_paste_or_degrade`，超时映射 "write_back_only"（不崩）
- 新增 `hide_panel_and_wait(AppHandle)`：优先 hide `clip-popover`，不存在则 hide `main`，sleep 100ms
- 新增 `run_paste_with_backend(AppHandle, item)`：cfg 分流，macOS 用 MacOs* 实现，非 macOS 用 Fallback*
- `paste_to_front` 命令：`fetch_paste_item` 取内容 → `run_paste_with_backend` 编排

## 务实焦点方案与已知限制

当前方案依赖 macOS 在窗口 hide 后自动把焦点还给上一个 App（100ms 延时留给系统完成切换）。

**未实现**：显式 `RecordFrontmost` / `ActivateOriginalApp`（完整 FocusStep 序列）——完整方案需在 popover 打开时记录前台 App，改动面大，列为后续增强。

## outcome 真实化

- trusted=true + changeCount 正常递增 → "full_paste"
- trusted=false → "write_back_only"（不调 send_paste）
- trusted=true 但 changeCount 冻结（超时）→ 剪贴板已写入，映射 "write_back_only"，不向前端暴露错误

## TDD 测试点（新增 T6–T10）

| 编号 | 场景 | 断言 |
|------|------|------|
| T6 | map_outcome(FullPasteDone) | == "full_paste" |
| T7 | map_outcome(WriteBackOnlyDone) | == "write_back_only" |
| T8 | paste_orchestrate trusted=true 正常 | == "full_paste"，send_paste_called=true |
| T9 | paste_orchestrate trusted=false | == "write_back_only"，send_paste_called=false |
| T10 | paste_orchestrate trusted=true 超时 | == "write_back_only"，send_paste_called=false |

## 构建验证

- `cargo build`：EXIT 0，无 error，无 warning
- `cargo test`：81 passed；0 failed（全量回归通过）

## 重申

键盘事件注入（CGEvent Cmd+V）+ 焦点编排需 GUI + Accessibility 权限实测方可验证真实效果，headless 测试仅覆盖逻辑映射分支。
