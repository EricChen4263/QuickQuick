---
id: V1-F2-S05-code
type: coding_record
level: 小功能
parent: V1-F2
children: []
created: 2026-05-30T23:12:21Z
status: 通过
commit: WIP
acceptance_ids: [V1-F2-A11]
evidence:
  - src-tauri/src/db.rs
  - src-tauri/tests/clipboard.rs
author: coder
---

# 编码记录 · V1-F2-S05 收藏（★置顶 + 豁免清理）

## 做了什么

在 `db.rs` 增加收藏功能：`is_favorite` schema 列、`set_favorite` API、`list_ordered` 置顶排序查询、`cleanup_keep_recent` 豁免清理裁剪。在 `tests/clipboard.rs` 追加两条 A11 验收测试（置顶 + 豁免）。

## 关键决策与理由

- **is_favorite 列加在建表语句内（非 ALTER TABLE）**：新库直接建出完整 schema，避免迁移复杂度；`CREATE TABLE IF NOT EXISTS` 幂等，与设计§十预埋铁律一致。
- **置顶排序 SQL**：`ORDER BY is_favorite DESC, last_modified_utc DESC`——收藏项（1）整体排在非收藏（0）之前，组内仍按最近修改降序，语义清晰，单次查询无子查询。
- **清理豁免用软删（非物理删）**：`cleanup_keep_recent` 对超出 keep_count 的非收藏旧项执行软删（is_deleted=1），与 `soft_delete` 语义一致（设计§六），GC 负责后续物理清理；收藏项完全不进入候选集（WHERE is_favorite=0），绝不触碰。
- **keep_count 语义针对非收藏项**：只计非收藏条目，收藏项不占容量配额，与设计§五"收藏永远豁免"对齐。
- **set_favorite 同时刷新 last_modified_utc**：收藏操作反映到时间线，与 bump_to_top 惯例一致；WHERE 限定 is_deleted=0，不意外操作已软删行。
- **ClipRow 公开结构体**：list_ordered 返回类型，含 id/is_favorite/last_modified_utc 三字段，业务层排序/显示所需最小集，避免过度暴露。

## 改动文件

- `src-tauri/src/db.rs` — 新增 `is_favorite` 列至建表语句；新增 `ClipRow` 结构体；新增 `set_favorite`、`list_ordered`、`cleanup_keep_recent` 三个公开函数；更新模块文档注释
- `src-tauri/tests/clipboard.rs` — 追加 `favorite_pin_sorted_first`（A11 置顶断言）和 `favorite_exempt_from_cleanup`（A11 豁免断言）两个测试

## 自测结论（TDD 红-绿-重构）

- RED：先在 clipboard.rs 写两个测试（引用 `set_favorite`、`list_ordered`、`cleanup_keep_recent`），运行确认因符号不存在编译失败（exit=101，unresolved imports）。
- GREEN：在 db.rs 实现三函数 + schema 列 + ClipRow，两个 favorite 测试全绿（clip=0，10/10 pass）。schema 回归 7/7 pass，全量 all=0（37 tests）。
- REFACTOR：clippy 发现 `assert_eq!(..., true)` 可改为 `assert!(...)`，已修正；重跑 clippy=0、全量绿。

code-standards 逐项自检：
- 格式：4 空格缩进，Rust 惯例，行宽合规。
- 函数：三个新函数均 ≤ 50 行，参数 ≤ 3 个，嵌套 ≤ 2 层（early return 降嵌套），单一职责。
- 命名：`set_favorite`（动词+名词）、`list_ordered`（动词+形容词）、`cleanup_keep_recent`（动词+形容词）；`ClipRow` PascalCase；`is_favorite` 布尔量带 `is_` 前缀。
- 注释：每个函数含 doc-comment 说明用途、参数语义、`# Errors` 节；注释解释"为什么"（设计锚点）。
- 类型：无裸 unwrap（query_map 结果用 `?` + filter_map ok()），返回 `Result<_, DbError>`，无 panic。
- 安全：SQL 全参数化（rusqlite::params!），无字符串拼接，无裸 unwrap/panic。
- 测试：AAA 结构，断言含描述性 message，非恒真（清理数 >= 1、收藏项 count=1 是实际状态断言，非常量）。
- 装饰注释：本次新增代码无 `──/═══/━━━` 装饰分隔符（grep 无新增命中）。
- TODO/FIXME：无遗留（grep 返回 1）。

---

## 审查修复（打回第 1 次，2026-05-31）

按 code-reviewer 打回意见修复两项 Important：

**I-01 修复**（`db.rs` `cleanup_keep_recent` 错误 `?` 传播）
- 原实现：`.filter_map(|r| r.ok()).collect()` 静默吞掉迭代中的 rusqlite::Error，导致 all_ids 偏少、裁剪偏差。
- 改为两阶段写法，与 `list_ordered` 惯例一致：
  ```rust
  let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
  let mut all_ids: Vec<String> = Vec::new();
  for r in rows { all_ids.push(r?); }
  ```
  任意行读取失败均通过 `?` 上抛 DbError，函数返回 Result 语义不变。

**I-02 修复**（`tests/clipboard.rs` `favorite_pin_sorted_first` 强制 is_favorite 排序生效）
- 原实现：`set_favorite(A)` 同时刷新 A 的 last_modified_utc，使 A 的时间戳比 B 更新；即便 SQL 删掉 `is_favorite DESC` 仅按时间戳排序，A 仍排第一——测试对错误实现无排除能力。
- 修复：在 `set_favorite(A, true)` 之后、`list_ordered` 之前插入 `bump_to_top(&conn, &id_b)`，把非收藏 B 的 last_modified_utc 刷到最新，确保 B 的时间戳晚于 A；此后只有 `ORDER BY is_favorite DESC, last_modified_utc DESC` 中的 is_favorite DESC 才能使 A 排第一。

回归结论（2026-05-31）：
- clip=0（10/10，favorite 两测试均 ok）
- all=0（全量 24 tests，6 个 test result 均 ok）
- clippy=0（零 warning/error）
- TODO/FIXME grep exit=1（无遗留）
