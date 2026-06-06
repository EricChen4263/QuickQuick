---
id: RT1-F1-S04-code
type: coding_record
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [RT1-F1-A04]
evidence:
  - src-tauri/src/ipc/system.rs
  - src-tauri/src/macos_paste.rs
  - src-tauri/src/lib.rs
  - src/ipc/ipc-client.ts
author: coder
---

# 编码记录 · RT1-F1-S04 还原：粘贴 + 复制（后端）

## 做了什么
让「粘贴到前台」与新增的「复制」IPC 都带富文本：粘贴取数填 html、写回剪贴板用 arboard `set().html`（纯文本兜底）；新增 `copy_clip_to_clipboard(id)` 命令。

## 关键决策与理由
- **`write_item_to_clipboard` 提取为跨平台 helper**（`pub(crate)`，无 cfg）：macos_impl / fallback_impl / 复制命令三处复用，消除平行 set_text 逻辑。`Some(h) if !h.is_empty() => set().html(h, Some(text))`，否则 set_text——空 html 与无 html 同走纯文本。
- **`set().html(h, Some(text))` 纯文本兜底**：目标 app 不支持 HTML 时自动退纯文本（设计§七）。
- **`fetch_clip_for_copy` 委托 `fetch_paste_item`**：复制与粘贴取数语义一致（图片均返 Err，图片复制留后续），DRY。
- **arboard 实写隔离到薄封装**：`write_clip_to_system_clipboard` 归 manual_confirm，取数+组装纯逻辑可单测。

## 改动文件
- `src-tauri/src/ipc/system.rs` — fetch_paste_item 取 html；新增 fetch_clip_for_copy / write_clip_to_system_clipboard / copy_clip_to_clipboard
- `src-tauri/src/macos_paste.rs` — write_item_to_clipboard helper，两个 write_with_marker 调它
- `src-tauri/src/lib.rs` — invoke_handler 注册 copy_clip_to_clipboard
- `src/ipc/ipc-client.ts` — copyClipToClipboard 封装
- `src-tauri/tests/richtext_paste_copy.rs`（新建）— 2 个验收测试

## 自测结论（TDD 红-绿-重构）
- 先写 `fetch_paste_item_includes_html`、`copy_clip_assembles_text_and_html`（RED：unresolved import），实现后 GREEN。
- 实跑全量 `cargo test`：186 passed / 1 failed——唯一失败为预存在且无关的 `traffic_light_position_returns_centered_coords`，除此**无新增失败**（如实报告，未误报全绿）。
- `cargo clippy -- -D warnings` exit 0；`pnpm exec tsc --noEmit` 0 错。
- arboard set().html 实写归 RT1-M01 manual_confirm。
