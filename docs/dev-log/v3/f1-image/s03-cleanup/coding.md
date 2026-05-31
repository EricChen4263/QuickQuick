---
id: V3-F1-S03-code
type: coding_record
level: 小功能
parent: V3-F1
children: []
created: 2026-05-31T02:34:16Z
status: 通过
commit: WIP
acceptance_ids: [V3-F1-A04]
evidence:
  - src-tauri/src/image.rs
  - src-tauri/src/db.rs
  - src-tauri/tests/image.rs
author: coder
---

# 编码记录 · V3-F1-S03 分级清理

## 做了什么

在 `src-tauri/src/image.rs` 实现了图片存储的两级分级清理机制，并在 `src-tauri/src/db.rs` 的 `clip_images` 表预埋了 `is_favorite` 列，使收藏豁免逻辑可在 SQL 层直接过滤。

具体行为：

- **第一级（strip original）**：当图片总字节数超过 `CleanupPolicy::max_total_bytes`（默认 500 MiB），按 `created_utc` 最旧优先、仅对 `is_favorite=0` 且 `original_present=1` 的行执行 `strip_original`——清空原图 BLOB、置 `original_present=0`、保留缩略图——直到总量降至阈值以下或无可腾行。
- **第二级（整条软删）**：若 strip 全部非收藏原图后总量仍超限（缩略图累计也超），再按最旧优先对非收藏行执行软删（`is_deleted=1`），直到达标或仅剩收藏行。
- **收藏永久豁免**：两级均通过 SQL `WHERE is_favorite = 0` 过滤，收藏行不参与任何清理。
- **三态归一**：超大图未存（`ingest_image_with_policy` 超阈）、清理 strip、v2 同步未拉三种情形，统一表示为 `original_present=0`，由 `is_degraded()` 函数统一判断，调用方无需区分来源。
- **总量计算**：`total_image_bytes()` 对 `original` 和 `thumbnail` 两列均使用 `COALESCE(length(...), 0)` 求和，NULL BLOB 计为 0，避免 SQLite `SUM` 对 NULL 的静默忽略。

## 关键决策与理由

- **`clip_images` 加 `is_favorite` 列**：图片表独立维护收藏标记（不依赖 `clip_items.is_favorite`），原因是分级清理逻辑完全在 `clip_images` 层操作，若跨表 JOIN 则查询复杂且性能差；与 `clip_items.is_favorite` 语义对称，保持"每层自包含"的设计原则。实际实现中 `clip_images` schema 已加 `is_favorite INTEGER NOT NULL DEFAULT 0`，并在 S03 测试辅助函数 `insert_image_with_ts` 中直接写入该列。
- **三态归一为 `original_present=0`**：拒绝了"用多个字段区分超大/strip/未拉"的方案。三种状态对 UI 和上层业务均等价（均无可用原图），归一后 `is_degraded()` 一个断言即可覆盖所有情形，消除了条件分支爆炸风险。
- **软删而非物理删（第二级）**：第二级删行使用 `is_deleted=1` 墓碑，与 `db::soft_delete` 语义一致，GC 负责后续物理清理；物理删会破坏外键级联语义并使 GC 逻辑失去幂等点。
- **策略结构体 `CleanupPolicy`**：将 `max_total_bytes` 封装为 struct，测试中可传小值（如 `0`）验证可配性，生产默认值为 `500 * 1024 * 1024`（常量 `DEFAULT_MAX_TOTAL`），不硬编码在函数签名里。
- **逐行查询 + 循环 strip**：每次 strip 后立即重新调用 `total_image_bytes()` 而非预先计算，确保多并发场景下不过度清理。代价是多次 SQL 往返，当前数据规模下可接受。

## 改动文件

- `src-tauri/src/image.rs` — 新增 `CleanupPolicy`、`CleanupReport`、`DEFAULT_MAX_TOTAL`、`total_image_bytes`、`is_degraded`、`strip_original`、`tiered_cleanup` 公开 API，以及内部私有函数 `strip_oldest_originals`、`delete_oldest_nonfavorite_rows`、`current_utc_ms`
- `src-tauri/src/db.rs` — `clip_images` 表 schema 新增 `is_favorite INTEGER NOT NULL DEFAULT 0` 列（schema 版本 10）
- `src-tauri/tests/image.rs` — 新增三个 V3-F1-A04 集成测试：`tiered_cleanup_and_state_unify_strips_oldest_nonfavorite_preserves_favorite`、`tiered_cleanup_deletes_whole_row_when_thumbnails_also_exceed_limit`，以及辅助函数 `insert_image_with_ts`

## 审查修复（code-reviewer 打回第 1 次）

按审查意见修复三项：

- **I-1 软删守卫**：`strip_original` 的 UPDATE SQL 加 `AND is_deleted = 0`，防止对已软删行静默 strip 造成语义不一致。
- **I-2 累计释放避免 O(N²)**：`strip_oldest_originals` 候选查询改为同时 `SELECT id, COALESCE(length(original), 0)`，strip 前已知该行原图大小；strip 后用本地变量 `estimated_total -= orig_len` 累计估算，仅在初始时调用一次 `total_image_bytes` 全表扫，消除原来每条循环都调 `total_image_bytes` 导致的 O(N²) 全表扫描。
- **I-3 测试从 DB 查 length**：`tiered_cleanup_and_state_unify_strips_oldest_nonfavorite_preserves_favorite` 测试中的 `oldest_orig_size` 由 `oldest_png.len() as i64` 改为执行 `SELECT COALESCE(length(original), 0) FROM clip_images WHERE id = ?1`，固化"DB 实际存储大小"而非依赖内存推算，消除存储转码假设。

回归结果：image 6/6 通过（tiered_cleanup 两测试均真命中 ok），全量 78 测试绿，clippy 零警告，无装饰注释，无 TODO。

## 自测结论（TDD 红-绿-重构）

**TDD 循环：**

1. RED：先在 `tests/image.rs` 写 `tiered_cleanup_and_state_unify_strips_oldest_nonfavorite_preserves_favorite`，引用尚不存在的 `tiered_cleanup`、`is_degraded` — 编译失败确认红态。
2. GREEN：依次实现 `total_image_bytes` → `strip_original` → `is_degraded` → `strip_oldest_originals` → `tiered_cleanup`，使第一个测试编译并通过。
3. RED-2：追加 `tiered_cleanup_deletes_whole_row_when_thumbnails_also_exceed_limit`，`max_total_bytes=0` 路径进第二级，首次运行失败（第二级未实现）。
4. GREEN-2：实现 `delete_oldest_nonfavorite_rows`，测试通过。
5. REFACTOR：将私有辅助函数 `strip_oldest_originals` / `delete_oldest_nonfavorite_rows` 从 `tiered_cleanup` 内联拆出，各自 ≤ 30 行，消除嵌套超 2 层的 for 循环体。

**code-standards 自检：**

- 格式/命名：函数名全部「动词+名词」（`strip_original`、`total_image_bytes`、`is_degraded`），布尔返回量前缀 `is_`；`cargo fmt` 通过。
- 函数长度：最长函数 `tiered_cleanup` 12 行，`strip_oldest_originals` / `delete_oldest_nonfavorite_rows` 各约 20 行，均 ≤ 50 行。
- 嵌套：最深两层（for 循环 + if），符合 ≤ 3 层要求。
- 注释：doc comment 写「为什么」与调用约定，无装饰性分隔线，无死代码注释。
- 安全：SQL 均参数化（`rusqlite::params![]`），无字符串拼接，无裸 unwrap（业务路径全走 `?`）。
- 类型/错误：返回 `Result<_, DbError>`，与模块约定一致；`CleanupReport` 实现 `Default`/`PartialEq` 便于测试断言。
- 无 TODO / FIXME 遗留。
- clippy：0 警告（`cargo clippy -- -D warnings` 通过）。
- 测试：3 个集成测试全绿，覆盖第一级 strip、第二级整条删、收藏豁免、三态归一四条验收路径。
