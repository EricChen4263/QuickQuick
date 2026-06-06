---
id: RT1-F1-S02-code
type: coding_record
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [RT1-F1-A02]
evidence:
  - src-tauri/src/pipeline.rs
  - src-tauri/tests/richtext_capture.rs
author: coder
---

# 编码记录 · RT1-F1-S02 捕获层读 HTML + 变化检测纳入 html

## 做了什么
让生产剪贴板后端 `ArboardBackend` 真正读取 HTML：`read()` 用 `cb.get().html().ok()` 填 `snapshot.html`；变化检测哈希纳入 html，使"同纯文本但新增/变更格式"能被检测到从而走 ingest 补写。

## 关键决策与理由
- **哈希组合抽成纯函数 `composite_hash_bytes(text, html, image)`**：原 `compute_composite_hash` 直接读真剪贴板、不可单测；抽纯函数后可锚定"html 不同→hash 不同"。`compute_composite_hash(cb)` 改为读三者后委托它。
- **固定拼接顺序 + 独立分隔符**：`text + 0xFF + html + 0xFF + image`，FNV-1a（与 db.rs 同算法，持久化稳定）；UTF-8 串内不含独立 0xFF，防跨段碰撞。
- **`get().html().ok()` 降级**：arboard 无 html 时返 Err，`.ok()`→None 不 panic。

## 改动文件
- `src-tauri/src/pipeline.rs` — 新增 `composite_hash_bytes` 纯函数；`compute_composite_hash` 委托 + 读 html；`read()` 实读 html
- `src-tauri/src/clipboard.rs` — `snapshot_to_clips_for_test`（#[doc(hidden)] 测试导出，沿用 `rgba_to_png_for_test` 约定）
- `src-tauri/tests/richtext_capture.rs`（新增）— 2 个验收测试

## 自测结论（TDD 红-绿-重构）
- 先写 `composite_hash_differs_when_html_differs`、`snapshot_to_clips_propagates_html` 失败测试，实现使其绿。
- 最后一次编辑后实跑全量 `cargo test`：主单测 270 passed、各集成套件全绿、`richtext_capture` 2 passed；除预存在且无关的 `traffic_light_position_returns_centered_coords` 外无新增失败。
- `cargo clippy --all-targets -- -D warnings` exit 0。
- read() 真 arboard GUI 读 html 路径不可自动化，归 manual_confirm（RT1-M01）。
