---
id: V1-F1-S02-code
type: coding_record
level: 小功能
parent: V1-F1
children: []
created: 2026-05-30T22:34:37Z
status: 通过
commit: WIP
acceptance_ids: [V1-F1-A04, V1-F1-A05]
evidence:
  - src-tauri/src/db.rs
  - src-tauri/tests/clipboard.rs
author: coder
---

# 编码记录 · 去重入库 + 置顶刷新（V1-F1-S02）

## 做了什么

在 `src-tauri/src/db.rs` 追加去重/置顶相关公共 API，实现设计文档§三「去重=内容去重+置顶刷新」机制。新增：

- `IngestOutcome` 枚举（`Inserted(id)` / `Bumped(id)`）
- `ingest(conn, item)` — 去重入库主入口
- `bump_to_top(conn, id)` — 显式置顶（仅改 `last_modified_utc`）
- `count_live(conn)` — 未软删行数查询
- `top_id(conn)` — 最前条目 id 查询
- `text_hash(text)` — 内部判重哈希函数
- `clip_items` 建表语句新增 `text_hash TEXT` 列

`src-tauri/tests/clipboard.rs` 追加两个验收测试：`dedup_and_bump`（A04）、`bump_no_new_record`（A05）。

## 关键决策与理由

### 哈希算法：`DefaultHasher` 而非 SHA-256

使用 `std::collections::hash_map::DefaultHasher` 而非加密哈希（如 SHA-256）。理由：
- 不需要新增依赖（`sha2` crate）；
- 判重场景不要求抗碰撞，只需同文本→同哈希；
- `DefaultHasher` 在单进程内确定性一致（同版本 Rust）；
- 代码注释明确标注「非加密，判重用途」，防止误用。

### schema 迁移策略：直接改建表语句

`text_hash TEXT` 直接写入 `CREATE TABLE IF NOT EXISTS` 语句（非 `ALTER TABLE`）。理由：
- V1 阶段库为空，不存在既有数据迁移问题；
- `IF NOT EXISTS` 保证幂等：已有库不受影响（schema 测试回归确认）；
- 避免引入 `ALTER TABLE` 迁移逻辑，保持简洁；
- schema 回归测试 7 passed，确认 V0 约定的列（`id`/`created_utc`/`last_modified_utc`/`is_deleted`/`deleted_at_utc`）仍全部存在。

### `ingest` 的去重查询：按 `text_hash + is_deleted=0`

查询条件为 `text_hash = ? AND is_deleted = 0`，软删行不参与去重比对。理由：
- 软删行已从用户视角消失，不应影响新内容的入库；
- 与设计文档「未软删」语义对齐。

### `bump_to_top` 语义：仅改 `last_modified_utc`

不触碰 `created_utc`、`content`、`is_deleted` 等字段。`last_modified_utc DESC` 作为列表排序依据，只需更新此字段即可实现「移到最前」。这保证了「置顶刷新由业务逻辑显式改库，绝不靠重新捕获」的设计约束（A05）。

### `OptionalExtension` trait 引入

`rusqlite::query_row` 在查无结果时返回 `Err(QueryReturnedNoRows)`，需通过 `OptionalExtension::optional()` 将其转换为 `Ok(None)`。这是 rusqlite 的标准用法，显式引入 trait 比手动 match 更简洁。

## 改动文件

- `src-tauri/src/db.rs`
  - 文件头注释更新（增加 S02 相关函数说明）
  - `use` 追加：`DefaultHasher`、`Hash`/`Hasher`、`OptionalExtension`、`Uuid`、`CapturedItem`
  - `clip_items` 建表语句新增 `text_hash TEXT` 列
  - 新增公共 API：`IngestOutcome`、`ingest`、`bump_to_top`、`count_live`、`top_id`
  - 新增内部函数：`text_hash`

- `src-tauri/tests/clipboard.rs`
  - 文件头注释更新（增加 S02 验收项）
  - `use` 追加：`CapturedItem`、`db`、`tempfile::tempdir`
  - 新增测试：`dedup_and_bump`（A04）、`bump_no_new_record`（A05）

## 自测结论（TDD 红-绿-重构）

**RED**：先写 A04/A05 测试，`cargo test --test clipboard` 编译失败：

```
error[E0432]: unresolved imports `quickquick_lib::db::IngestOutcome`,
              `bump_to_top`, `count_live`, `ingest`, `top_id`
```

确认失败原因为功能未实现（非环境/语法问题）。

**GREEN**：实现 `db.rs` 新增函数 + schema 加列后：

```
test bump_no_new_record ... ok
test dedup_and_bump ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

**REFACTOR**：实现已是最小实现，无重复逻辑，无需进一步重构。

**验证指标**：

| 检查项 | 结果 |
|--------|------|
| `cargo test --test clipboard` | 6 passed, 0 failed（含 A04/A05）|
| `cargo test --test schema`（回归）| 7 passed, 0 failed |
| `cargo clippy --all-targets -- -D warnings` | clippy=0，零 warning |
| `cargo build` | build=0 |
| `grep -rn TODO\|FIXME src-tauri/src/` | 无（exit=1）|

**code-standards 逐项自检**：

- 格式：4 空格缩进，行宽 ≤ 100，符合 §2
- 函数：`ingest` 30 行，`bump_to_top` 7 行，`count_live` 9 行，`top_id` 10 行，`text_hash` 8 行；嵌套最深 2 层；单一职责，符合 §3
- 命名：`IngestOutcome`/`Inserted`/`Bumped` PascalCase；`ingest`/`bump_to_top`/`count_live`/`top_id`/`text_hash` snake_case 动词+名词，符合 §4
- 注释：所有公共类型/函数有 `///` 文档注释，内部函数 `text_hash` 注释说明显式稳定 FNV-1a、跨版本一致、非加密用途，符合 §5
- 类型：无裸 `unwrap`/`panic`；全部返回 `Result<_, DbError>`；`OptionalExtension` 处理 QueryReturnedNoRows，符合 §6
- 性能：去重查询走索引（`text_hash` 等值查询）；`bump_to_top` 单条 UPDATE；无多余查询，符合 §7
- 测试：AAA 结构；`dedup_and_bump` 断言去重（count 不增）+ 置顶（top_id == id_x）；`bump_no_new_record` 断言无新行 + 置顶；非恒真，符合 §8
- 安全：无密钥泄露；SQL 全部参数化（`rusqlite::params!`）；无用户输入直接拼接，符合 §10

---

## 按审查修复（打回第 1 次 → 第 2 次提交）

**修复项**：C-01（text_hash 改 FNV-1a 显式稳定）+ I-01（schema 断言 text_hash 列）+ I-02（测试去时序依赖）

### C-01：text_hash 改 FNV-1a 显式稳定

原因：`std::collections::hash_map::DefaultHasher` 的哈希值在跨 Rust 工具链构建时无标准保证（std 文档："its hashes should not be relied upon over releases"）。`text_hash` 作为持久化进 SQLCipher 的去重键，须在跨 app 升级（不同构建）后保持一致，否则旧条目判重失效产生重复。

改法：手写 FNV-1a 64-bit（`FNV_PRIME = 0x0000_0100_0000_01B3`，`FNV_OFFSET = 0xcbf2_9ce4_8422_2325`），无新依赖。同时移除不再需要的 `use std::collections::hash_map::DefaultHasher` 和 `use std::hash::{Hash, Hasher}` import，避免 unused warning。

FNV-1a 确定性：算法完全由常量和字节运算决定，与 Rust 版本/构建无关，是显式稳定哈希。

### I-01：schema 断言 text_hash 列

在 `schema_preembed_columns_clip_items` 末尾追加：

```rust
assert!(cols.contains(&"text_hash".to_string()), "clip_items 应含 text_hash 列（S02 去重字段）；实际: {:?}", cols);
```

冻结 S02 核心新增列，防止后续 schema 变动静默移除。

### I-02：dedup_and_bump 去时序依赖

原代码：`bump_to_top(&conn, &top_id(&conn).expect(...).expect(...))` — 若两次 ingest 同一毫秒，`ORDER BY last_modified_utc DESC LIMIT 1` 返回值不确定，可能返回 X 而非 Y，导致置顶 Y 的步骤被静默跳过。

改法：持有 `ingest Y` 返回的 `id_y`（`IngestOutcome::Inserted(id)`），直接 `bump_to_top(&conn, &id_y)`，不依赖时序查询。断言意图不变（Y 置顶后 X 再 bump 成为最前、count 不变）。

### 回归结论

| 检查项 | 结果 |
|--------|------|
| `cargo test --test clipboard` | clip=0，6 passed（dedup_and_bump + bump_no_new ok）|
| `cargo test --test schema` | schema=0，7 passed（含新 text_hash 断言）|
| `cargo test`（全量）| all=0，24 passed 总计 |
| `cargo clippy --all-targets -- -D warnings` | clippy=0，零 warning |
| `grep -rn TODO\|FIXME src-tauri/src/` | 无（exit=1）|
