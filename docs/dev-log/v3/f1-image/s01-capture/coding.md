---
id: V3-F1-S01-code
type: coding_record
level: 小功能
parent: V3-F1
children: []
created: 2026-05-31T02:02:33Z
status: 通过
commit: WIP
acceptance_ids: [V3-F1-A01]
evidence:
  - src-tauri/src/image.rs
  - src-tauri/tests/image.rs
  - src-tauri/src/db.rs
  - src-tauri/src/lib.rs
  - src-tauri/tests/schema.rs
author: coder
---

# 编码记录 · 图片捕获入库（V3-F1-S01）

## 做了什么

新增 `src-tauri/src/image.rs`，实现图片捕获入库的核心逻辑：
FNV-1a 字节哈希判重、原图无损 BLOB 存储、缩略图/原图拆分两字段。
同时给 `clip_images` 表补加 `image_hash TEXT` 列，并在 schema 回归测试中追加该列的断言。

## 关键决策与理由

- **字节哈希用 FNV-1a 64-bit 对 `&[u8]` 逐字节操作**：与 `db::text_hash`（对 `&str`）区分，
  两者哈希空间独立互不干扰。FNV-1a 无外部依赖、确定性稳定、跨版本一致，满足判重需求。
  非加密哈希，碰撞概率业务可接受。

- **原图无损**：`original` 字段以参数化 BLOB 写入（`rusqlite::params!` 传 `&[u8]`），
  SQLCipher 不对 BLOB 内容做任何变换，取回时与输入逐字节相同。
  测试中用 `assert_eq!(fetched_original, original_bytes)` 直接验证。

- **缩略图/原图拆分两 BLOB 字段**：`thumbnail` 和 `original` 是 `clip_images` 表的两个独立
  BLOB 列，`get_image_thumbnail`/`get_image_original` 分别 SELECT 对应列，测试分别断言两者
  内容与输入相同，验证拆分存储成立。

- **`clip_images` 无外键约束 clip_item_id（S01 不绑父行）**：S01 范围内 `ingest_image` 不
  传 `clip_item_id`，插入时该列为 NULL。外键约束仅在非 NULL 时触发（REFERENCES 语义），
  现有外键测试（`foreign_key_rejects_dangling_clip_item_id`）针对显式传入非法 id 的场景，
  NULL 值绕过约束是 SQLite 标准行为，两者不冲突。

- **`image_count` 过滤软删行**：`WHERE is_deleted = 0`，与 `count_live` 语义一致，
  避免判重后计数含软删行导致断言错误。

- **不引入新 crate**：FNV-1a 手写实现（与 `db::text_hash` 模式一致），保持零新依赖。

## 改动文件

- `src-tauri/src/image.rs` — 新建；含 `image_hash`、`IngestImageOutcome`、`ingest_image`、
  `get_image_original`、`get_image_thumbnail`、`image_count` 及单元测试
- `src-tauri/tests/image.rs` — 新建集成测试；含 `image_capture_lossless_split_insert_dedup_and_different`（A01）
- `src-tauri/src/db.rs` — `ensure_schema` 中 `clip_images` 建表语句追加 `image_hash TEXT` 列
- `src-tauri/src/lib.rs` — 追加 `pub mod image;` 一行
- `src-tauri/tests/schema.rs` — `schema_preembed_columns_clip_images` 追加 `image_hash` 列断言

## 自测结论（TDD 红-绿-重构）

**RED**：先写 `tests/image.rs`（含 A01 测试），运行后因 `quickquick_lib::image` 不存在，
编译报 `error[E0432]: unresolved import`，确认测试感知到实现缺失。

**GREEN**：
1. 新建 `src/image.rs` 实现全部函数
2. `db.rs` 追加 `image_hash` 列
3. `lib.rs` 注册 `pub mod image;`

```
cargo test --test image
test image_capture_lossless_split_insert_dedup_and_different ... ok
test result: ok. 1 passed; 0 failed; finished in 0.66s
```

**REFACTOR**：实现已是最小实现，函数职责单一，无重复逻辑，无需进一步重构。

**验证指标**：

| 检查项 | 结果 |
|--------|------|
| `cargo test --test image image_capture` | 1 passed, 0 failed |
| `cargo test --test schema` | 10 passed, 0 failed（含 image_hash 列断言） |
| `cargo test`（全量） | 全绿（最终汇总 1+2+4+5+32+10+67+1 passed） |
| `cargo clippy --all-targets -- -D warnings` | clippy=0，零 warning |
| 装饰注释检查（`─── / ═══ / ━━━`） | 无（deco=1） |
| `grep TODO\|FIXME src-tauri/src/` | 无（todo=1） |

## 审查回归记录（打回第 1 次 → 已修复）

**I-01（clip_item_id=NULL 缺口）**：在 `src-tauri/src/image.rs` `ingest_image` 的文档注释中
补充 `# clip_item_id 缺口声明` 小节，明确声明：当前阶段 `clip_item_id` 不写入（NULL），
GC 级联（`clip_items ON DELETE CASCADE`）对本函数写入路径不生效，绑定及 GC 路径留待
V3-F1-A04 补全。无需改实现逻辑，仅补注释避免后续误判。

**I-02（测试判别力弱）**：删除旧的"倒序序列"用例 `image_hash_different_bytes_produce_different_hash`，
改为：
- `image_hash_differs_on_last_byte_only`：`[1,2,3]` vs `[1,2,4]`，末位仅差一字节，验证边界灵敏度；
- `image_hash_empty_and_single_byte_boundary`：空序列 vs `[0x00]`，边界不 panic 且两者哈希不同。
两个新用例均为 AAA 结构、非恒真断言。

**回归结论**：

| 检查项 | exit | 结果 |
|--------|------|------|
| `cargo test image_capture` | 0 | 1 ok |
| `cargo test image_hash` | 0 | 4 ok（新用例全部真命中） |
| `cargo test --test image` | 0 | 1 passed |
| `cargo test`（全量） | 0 | 全绿（1+2+4+5+32+10+67+1） |
| `cargo clippy --all-targets -D warnings` | 0 | 零 warning |
| 装饰注释检查 | deco=1 | 无 |
| TODO/FIXME 检查 | todo=1 | 无 |

**code-standards 逐项自检**：

- 格式：4 空格缩进，行宽 ≤ 100，文件末尾换行，符合规范
- 函数：最长函数 `ingest_image` 约 25 行，嵌套最深 2 层，单一职责，符合 ≤50 行 / ≤3 层约束
- 命名：`IngestImageOutcome`/`image_hash`/`ingest_image`/`get_image_original` 描述性命名；无 `tmp`/`x`/`flag`
- 注释：公共类型/函数均有 `///` 文档注释，注释写「为什么」（哈希算法选型理由、无损保证），无装饰性分隔符，无死代码
- 类型：无魔术数字（FNV 常量具名），公共接口全部显式类型，无裸 `unwrap`/`panic`（`unwrap_or_default` 仅用于时钟回退兜底，与 db.rs 一致）
- SQL：所有查询参数化（`rusqlite::params!`），无字符串拼接 SQL
- 测试：AAA 结构，行为化命名（`image_capture_lossless_split_insert_dedup_and_different`），非恒真断言，固定 key + tempfile，符合规范
- 安全：无密钥入库，SQL 参数化，无敏感信息日志，符合安全红线
