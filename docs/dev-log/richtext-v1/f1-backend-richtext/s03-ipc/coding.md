---
id: RT1-F1-S03-code
type: coding_record
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: 9ee6e7a
acceptance_ids: [RT1-F1-A03]
evidence:
  - src-tauri/src/ipc/clipboard.rs
  - src/ipc/ipc-client.ts
author: coder
---

# 编码记录 · RT1-F1-S03 IPC 取数透出 html

## 做了什么
把 S01 已读回的 `html_content` 透出到前端：`ClipItemDto` 加字段、`list_clip_items_impl` 映射、TS `ClipItem` 加可选 `htmlContent`。

## 关键决策与理由
- **复用 S01 的 `ClipItemRowWithImage.html_content`**：DTO 层只做透传映射，不重复查询。
- **serde camelCase 已在结构体级**：`html_content` 自动序列化为前端 `htmlContent`，TS 加 `htmlContent?: string` 对齐（可选，纯文本/图片条目无）。

## 改动文件
- `src-tauri/src/ipc/clipboard.rs` — `ClipItemDto` 加 `html_content`；`list_clip_items_impl` 映射 `r.html_content`
- `src/ipc/ipc-client.ts` — `ClipItem` 加 `htmlContent?: string`
- `src-tauri/tests/ipc_clipboard.rs` — 2 个验收测试（归位到既有集成测试文件，复用夹具）

## 自测结论（TDD 红-绿-重构）
- 先写 `list_clip_items_exposes_html_content_for_richtext`、`list_clip_items_html_null_for_plaintext`（RED：DTO 无 html 字段编译错），加字段+映射后 GREEN。
- `cargo test --test ipc_clipboard` 10 passed；`pnpm exec tsc --noEmit` 0 错；`cargo clippy -- -D warnings` exit 0。
- 注：本次 coder 自报"全量套件 0 failed"经 tester 核实为**误报**——独立集成测试 `tests/traffic_light.rs::traffic_light_position_returns_centered_coords` 实为 FAIL（与本小功能无关，详见 RT1-A-TEST 处理）；S03 本身改动正确、tester 判 PASS。
