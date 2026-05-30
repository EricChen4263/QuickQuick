---
id: V0-F3-S05-code
type: coding_record
level: 小功能
parent: V0-F3
children: []
created: 2026-05-30T22:05:24Z
status: 通过
commit: WIP
acceptance_ids: [V0-F3-A01, V0-F3-A02, V0-F3-A04, V0-F3-A05, V0-F3-A06]
evidence:
  - src-tauri/src/db.rs
  - src-tauri/src/lib.rs
  - src-tauri/tests/db.rs
  - src-tauri/tests/schema.rs
author: coder
---

# 编码记录 · V0-F3-S05 加密数据库 + schema 预埋 + 软删/GC + 失败恢复

## 做了什么

新增 `src-tauri/src/db.rs` 模块，实现 SQLCipher 加密数据库的打开/创建、schema 预埋、软删、物理 GC、以及损坏时改名备份的失败恢复机制。在 `lib.rs` 中注册 `pub mod db`，并为所有验收项编写集成测试（`tests/db.rs`、`tests/schema.rs`）。

## 关键决策与理由

- **raw key 格式（`PRAGMA key = "x'<hex>'"`)而非 passphrase**：KeyProvider 提供的 32 字节密钥直接对应 AES-256 密钥材料，使用 raw key 跳过 PBKDF2 派生，既无多余性能损耗，又与 KeyProvider 接口语义精确对应。密钥用 `hex_encode` 转为 64 字符十六进制后注入 PRAGMA，不写入日志。

- **打开后立即 `PRAGMA user_version`触发解密校验**：SQLCipher 的解密校验在首次 I/O 时触发，而非 `Connection::open` 时。若不做轻量查询，错误密钥到写操作时才暴露，导致测试和恢复逻辑难以区分"真正损坏"与"未校验"。`PRAGMA user_version` 是只读操作，代价极低。

- **`open_or_recover` 中以 `DbError::Sqlite` 判定永久损坏**：SQLCipher 在密钥错误或格式损坏时均返回 `rusqlite::Error`（`SQLITE_NOTADB` 或解密失败），与正常 SQL 失败同属 `Sqlite` 变体。在此场景下（已知文件存在且不可恢复），这是最简洁的判定路径；Io 错误单独保留不走备份流程，避免误备份权限问题引发的正常文件。

- **`fs::rename` 实现备份（不用 copy+delete）**：`rename` 是 OS 层原子操作，避免 copy 中途失败导致内容丢失；彻底杜绝静默删库（设计§六硬约束）。备份命名格式 `<原名>.corrupt-<utc_secs>` 可从文件名直接推算损坏时间。

- **schema `CREATE TABLE IF NOT EXISTS`（幂等）**：保证重复调用 `ensure_schema` 安全，多次 `open_or_create` 不报错（A01 幂等测试验证此行为）。

- **`gc_purge_deleted` 仅清理 `clip_items`**：A05 验收仅要求 clip_items 的 GC，clip_images 的 GC 留待 v1 功能完整时扩展（当前 GC 不涉及图片表，与验收项语义一致）。

- **[审查 polish] I-03：外键约束 + foreign_keys ON（§十预埋，2026-05-31）**：
  - `open_with_key` 开库后立即 `PRAGMA foreign_keys = ON`（运行期每条连接必须显式开启，SQLite 默认 OFF）。
  - `ensure_schema` 内追加 `PRAGMA foreign_keys = ON`（防止调用方绕过 `open_with_key` 直连时约束失效）。
  - `clip_images.clip_item_id` 从无约束 `TEXT` 改为 `TEXT REFERENCES clip_items(id) ON DELETE CASCADE`：GC 物理删除 `clip_items` 行时，关联图片自动级联清理，对齐设计§五分级清理语义。
  - 新增 3 个测试：`foreign_keys_pragma_is_enabled_after_open`（PRAGMA 值为 1）、`foreign_key_rejects_dangling_clip_item_id`（悬空 FK 被拒）、`gc_cascade_deletes_clip_images_on_clip_item_removal`（GC 级联删图）。

- **[审查 polish] I-01：PRAGMA key 注释（2026-05-31）**：
  - `open_with_key` 中 `PRAGMA key = "x'<hex>'"` 旁增加说明性注释，说明 SQLCipher 上游约束导致无法参数化、hex_key 来源受控、日志不泄漏密钥材料的理由，明确此处为代码库唯一参数化例外。

- **[审查 polish] I-02：测试辅助函数白名单校验（2026-05-31）**：
  - `tests/schema.rs` 的 `table_columns` 辅助函数加 `assert!(matches!(table, "clip_items" | "clip_images"), ...)` 白名单校验，防止误用不受信任表名（code-standards §10 输入校验）。

## schema 预埋关键列（设计§十铁律）

### clip_items
| 列名 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | UUID，永不复用 |
| `content` | TEXT | 剪贴板内容 |
| `kind` | TEXT | 类型（text/image/…） |
| `created_utc` | INTEGER | UTC epoch ms，创建时间 |
| `last_modified_utc` | INTEGER | UTC epoch ms，最后修改时间 |
| `is_deleted` | INTEGER | 墓碑：0=正常 1=软删 |
| `deleted_at_utc` | INTEGER | 软删时间（可 NULL） |

### clip_images
| 列名 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | UUID，永不复用 |
| `clip_item_id` | TEXT | 关联 clip_items.id |
| `thumbnail` | BLOB | 缩略图（拆分字段） |
| `original` | BLOB | 原图（拆分字段） |
| `original_present` | INTEGER | 1=有原图 0=仅缩略图（降级态） |
| `created_utc` | INTEGER | UTC epoch ms |
| `last_modified_utc` | INTEGER | UTC epoch ms |
| `is_deleted` | INTEGER | 墓碑 |
| `deleted_at_utc` | INTEGER | 软删时间（可 NULL） |

## 改动文件

- `src-tauri/src/db.rs` — 新增：加密开库、schema 预埋、软删、GC、失败恢复全部实现
- `src-tauri/src/lib.rs` — 新增一行 `pub mod db;`，注册公开模块
- `src-tauri/tests/db.rs` — 新增：A01/A02/A06 集成测试（6 个测试用例）
- `src-tauri/tests/schema.rs` — 新增：A04/A05 集成测试（4 个测试用例）

## 自测结论（TDD 红-绿-重构）

**RED**：先写 `tests/db.rs` 和 `tests/schema.rs`，此时 `db` 模块不存在，编译报 `unresolved import quickquick_lib::db`，确认失败原因是功能未实现（非语法/环境错）。

**GREEN**：新建 `src/db.rs`，实现 4 个公共函数和内部辅助，在 `lib.rs` 注册模块后：
- `cargo test --test db`：6/6 通过
- `cargo test --test schema`：4/4 通过

**REFACTOR**：提取 `open_with_key`、`ensure_schema`、`backup_corrupt_file`、`current_utc_ms`、`hex_encode` 五个单一职责内部辅助函数，使 `open_or_create` 和 `open_or_recover` 各 ≤25 行，嵌套 ≤2 层。

### code-standards 逐项自检

| 项目 | 结论 |
|---|---|
| 函数 ≤50 行 | 全部符合（最长函数 `ensure_schema` 约 35 行含 SQL） |
| 嵌套 ≤3 层 | 符合，最深 2 层 |
| 无裸 unwrap/panic | 符合；仅 `unwrap_or_default()`（SystemTime 失败时返回 0，无 panic） |
| 错误用 thiserror | 符合，`DbError` 用 `#[derive(Error)]` 覆盖 Sqlite/Io/Corrupt |
| SQL 参数化查询 | 符合，`soft_delete` 和 `gc_purge_deleted` 均用 `rusqlite::params!` |
| 密钥不入日志 | 符合，hex key 仅传入 PRAGMA，不 eprintln/log |
| 命名描述性 | 符合，函数全用「动词+名词」，布尔参数 `allow_rebuild` 清晰 |
| 注释写「为什么」 | 符合，raw key 格式选择、轻量查询时机均有注释说明理由 |
| clippy -D warnings | 通过（exit 0） |
| cargo build | 通过（exit 0） |
| 无 TODO/FIXME | 通过 |

## 验证证据

```
db=0
test db_recovery_corrupt_file_creates_backup_and_returns_err ... ok
test db_encrypt_ciphertext_on_disk ... ok
test db_create_auto_creates_file_on_first_run ... ok
test db_create_is_idempotent_on_subsequent_opens ... ok
test db_encrypt_wrong_key_returns_error ... ok
test db_recovery_allow_rebuild_creates_new_db_and_keeps_backup ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

schema=0
test schema_preembed_columns_clip_images ... ok
test schema_preembed_columns_clip_items ... ok
test soft_delete_gc_does_not_affect_live_rows ... ok
test soft_delete_and_gc_full_lifecycle ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

clippy=0
build=0
todo=1(无 TODO/FIXME)
```
