---
id: RT1-F1-S01-code
type: coding_record
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [RT1-F1-A01]
evidence:
  - src-tauri/src/db.rs
  - src-tauri/tests/richtext_storage.rs
author: coder
---

# 编码记录 · RT1-F1-S01 存储层支持 html

## 做了什么
为剪贴板条目实现 HTML 富文本的存储层：`clip_items` 新增 `html_content` 列（存量库幂等迁移），`ingest` 写入 html 并按 html 有无定 `kind`，去重命中旧行且旧行无 html 时补写并升级为 richtext，查询行读回 html。

## 关键决策与理由
- **存量库迁移用 `PRAGMA table_info` 守卫 + `ALTER TABLE ADD COLUMN`**：SQLite 无 `ADD COLUMN IF NOT EXISTS`，检测列不存在才加，保证幂等、对已有该列的库不重复加。
- **去重仍按 `text_hash`（只对纯文本），不改哈希**：贯彻决策4「纯文本相同即去重」；命中旧行仅在 `旧行 html IS NULL && 新 item.html.is_some()` 时 UPDATE 补写 + 升 kind，**不覆盖已有 html**。
- **同步 4 处内联建表 SQL**：ipc/clipboard.rs、ipc/system.rs、tests/capture_image.rs 各有与 ensure_schema 一致的手搓建表，schema 改后必须同步补列（否则编译过但运行期 `no such column`，已实测会红 7 例）；未触碰业务逻辑。
- **新增真实加密库集成测试**：防"内存库过、真 SQLCipher 库不过"的假绿。

## 改动文件
- `src-tauri/src/db.rs` — schema 加列 + `has_column`/`migrate_html_column` + `ingest` 补写/kind + 行结构与两查询读回 html
- `src-tauri/src/ipc/clipboard.rs`、`src-tauri/src/ipc/system.rs`、`src-tauri/tests/capture_image.rs` — 同步内联建表 SQL 加 `html_content TEXT`（仅测试/夹具）
- `src-tauri/tests/richtext_storage.rs`（新增）— 加密库 roundtrip + 纯文本去重集成测试

## 自测结论（TDD 红-绿-重构）
- 先写 4 个失败单测：`ingest_richtext_roundtrip_persists_html_and_kind`、`html_column_migration_idempotent_on_existing_db`、`dedup_by_plaintext_unchanged`、`ingest_backfills_html_and_upgrades_kind_on_hit`；+ 2 集成测试。实现使其全绿。
- 最后一次编辑后实跑全量 `cd src-tauri && cargo test`：373 passed / 0 failed（lib 270 passed；richtext_storage 2 passed；其余套件全绿）。
- `cargo clippy --all-targets -- -D warnings` exit 0。
- code-standards：函数 ≤50 行、SQL 参数化、注释写"为什么"、无 TODO/FIXME/死代码、断言验具体值。
- 范围外提示：`cleanup_keep_recent` 的 `ORDER BY last_modified_utc DESC` 缺 `rowid DESC` 兜底（本次未改，留后续任务）。
