---
id: V0-F3-S05-review
type: review
level: 小功能
parent: V0-F3
children: []
created: 2026-05-31T12:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V0-F3-A01, V0-F3-A02, V0-F3-A04, V0-F3-A05, V0-F3-A06]
evidence: []
author: code-reviewer
---

# 审查记录 · V0-F3-S05 加密数据库 + schema 预埋 + 软删/GC + 失败恢复

## 审查范围

- `src-tauri/src/db.rs`（新增）
- `src-tauri/src/lib.rs`（`pub mod db` 声明）
- `src-tauri/tests/db.rs`（新增集成测试：A01/A02/A06）
- `src-tauri/tests/schema.rs`（新增集成测试：A04/A05）

## 审查维度

code-standards（安全红线、SQL 参数化、Rust 错误处理、函数行数、命名、注释）+ 设计文档§五（图片存储）+ §六（失败/恢复语义）+ §十（schema 预埋铁律）。

## Critical 级别问题

无。

## Important 级别问题（首轮，均非阻塞；已于 polish 全部修复，见文末）

### I-01：`PRAGMA key` 以字符串格式化注入 hex 密钥（置信度 85）
位置 `src-tauri/src/db.rs`。SQLCipher 的 `PRAGMA key` 在 rusqlite 不支持 `?` 占位符（上游约束），字符串格式化为唯一可行路径；hex_key 来自受控 `[u8;32]`、内容固定 64 位十六进制，无注入风险、未被日志打印。建议加注释说明该约束 + 确认无 trace 钩子。

### I-02：测试辅助 `table_columns` 使用字符串格式化构造 PRAGMA 查询（置信度 80）
位置 `src-tauri/tests/schema.rs`。当前调用点均硬编码常量、无注入，但作为可复用辅助函数存在误用路径。建议白名单/enum 约束。

### I-03：`clip_images.clip_item_id` 缺外键约束、引用完整性未强制（置信度 80）
位置 `src-tauri/src/db.rs`。未声明 FOREIGN KEY、未启用 `PRAGMA foreign_keys=ON`。属预埋阶段 schema 遗漏；设计§十"预埋第一天就要对"。建议立即补外键 + foreign_keys ON（不留到 Phase 3）。

## 验收项覆盖核查

| 验收项 | 测试用例 | 结论 |
|---|---|---|
| V0-F3-A01 首次启动自动创建 | `db_create_auto_creates_file_on_first_run` + `db_create_is_idempotent_on_subsequent_opens` | 通过，断言真实 |
| V0-F3-A02 加密/错误密钥失败/密文落盘 | `db_encrypt_wrong_key_returns_error` + `db_encrypt_ciphertext_on_disk`（头部非 SQLite 魔数） | 通过，非恒真 |
| V0-F3-A04 schema 预埋列 | `schema_preembed_columns_clip_items` + `schema_preembed_columns_clip_images` | 通过，逐列断言 |
| V0-F3-A05 软删+GC | `soft_delete_and_gc_full_lifecycle` + `soft_delete_gc_does_not_affect_live_rows` | 通过，全生命周期 |
| V0-F3-A06 失败恢复永不静默删库 | `db_recovery_corrupt_file_creates_backup_and_returns_err` + `db_recovery_allow_rebuild_creates_new_db_and_keeps_backup` | 通过，备份字节级校验 |

## 安全 & 设计§十铁律核查

PRAGMA key raw key 格式正确；密钥不入日志；SQL 参数化（soft_delete params! 绑定、gc 无用户输入，仅 PRAGMA key 为上游约束例外）；密文落盘验证真实（检查头部非 `SQLite format 3\0`）；永不静默删库（fs::rename 原子改名 + allow_rebuild 门控）。UUID TEXT 主键（永不复用）、created/last_modified UTC epoch ms、墓碑 is_deleted/deleted_at、图片表 thumbnail/original 双 BLOB + original_present 降级态——§十铁律全符合。

## 总结论

**通过。** 无 Critical；三条 Important 均非阻塞改进项。其中 I-03 属§十预埋，要求 polish 即修。

---

## Polish 复核结论（2026-05-31）

**status = 通过**

三条 Important 已全部落地并复核：
- **I-01**：`open_with_key` 内 PRAGMA key 行已加完整注释（SQLCipher 上游不可参数化、hex_key 受控不可注入、无 trace 钩子无泄漏）。
- **I-02**：`table_columns` 已加 `matches!(table, "clip_items" | "clip_images")` 白名单断言。
- **I-03**：`open_with_key` + `ensure_schema` 双重 `PRAGMA foreign_keys = ON`；`clip_images.clip_item_id REFERENCES clip_items(id) ON DELETE CASCADE`；新增 3 测试（PRAGMA 值=1、悬空 FK 被拒、GC 级联删图）。
- **级联副作用核查**：`gc_purge_deleted` 仅删 `is_deleted=1` 行，live 行（is_deleted=0）及其图片不受影响，原软删/gc 语义仍正确。无新引入高危。
