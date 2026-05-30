---
id: V1-F1-S02-review
type: review
level: 小功能
parent: V1-F1
children: []
created: 2026-05-31T09:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V1-F1-A04, V1-F1-A05]
evidence: []
author: code-reviewer
---

# 审查记录 · V1-F1-S02 内容去重 + 置顶刷新

## 审查范围
- `src-tauri/src/db.rs`：新增 `ingest`/`text_hash`/`bump_to_top`/`IngestOutcome` + `clip_items` 加 `text_hash` 列
- `src-tauri/tests/clipboard.rs`：新增 A04 `dedup_and_bump` / A05 `bump_no_new_record`
参照：code-standards、设计文档§三关键机制、Rust 安全红线。

## Critical / 必修

### C-01 `text_hash` 用 `DefaultHasher`，作为持久化去重键不稳定
- 位置：`src-tauri/src/db.rs:352-356`（`DefaultHasher::new()`）
- **编排器核实校正**：reviewer 初判"每次进程重启即失效"机制不准确——`std::collections::hash_map::DefaultHasher::new()` 用固定种子(0,0)，**同一构建内跨进程是确定的**（随机化的是 HashMap 的 `RandomState`，非 `DefaultHasher::new()`）。但 **C-01 的核心结论仍成立**：std 文档明确 "its hashes should not be relied upon over releases"——`text_hash` 是**持久化进 SQLCipher 库的去重键**，DB 会跨 app 升级（不同 Rust 工具链构建）保留，届时同文本哈希可能改变 → 旧条目判重失效、产生重复。依赖 std 未指定哈希做持久化键属脆弱设计。
- 修复：改用**显式稳定**的确定性哈希。无需新依赖，手写 FNV-1a 64-bit：
  ```rust
  fn text_hash(text: &str) -> String {
      const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
      const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
      let mut hash = FNV_OFFSET;
      for byte in text.as_bytes() { hash ^= *byte as u64; hash = hash.wrapping_mul(FNV_PRIME); }
      format!("{hash:016x}")
  }
  ```
  注释标注"显式稳定哈希、跨 Rust 版本/构建一致、非加密、仅判重"。

## Important

### I-01 `schema.rs` 未断言 `text_hash` 列
- 位置：`src-tauri/tests/schema.rs` `schema_preembed_columns_clip_items`
- text_hash 是 S02 核心新增列，schema 测试应冻结之。补 `assert!(cols.contains(&"text_hash".to_string()), ...)`。

### I-02 A04 测试置顶 Y 依赖 `top_id()` 时序，存非确定性
- 位置：`src-tauri/tests/clipboard.rs` `dedup_and_bump`
- 两次 ingest 若同一毫秒，`ORDER BY last_modified_utc DESC LIMIT 1` 结果不定，`top_id()` 可能返回 X，中间步骤被静默跳过。
- 修复：持有 `ingest Y` 返回的 `id_y`，显式 `bump_to_top(&conn, &id_y)`，不依赖 top_id 时序。

## 通过项（正向确认）
去重仅查 is_deleted=0 ✓；bump_to_top 仅 UPDATE 不 INSERT（A05 行数断言）✓；全 SQL 参数化 ✓；text_hash 注释标注非加密判重用途 ✓；无裸 unwrap/panic（Result 传播）✓；函数 ≤50 行嵌套 ≤2 ✓；schema 加列 IF NOT EXISTS 幂等不破 V0 ✓；A05 测试持 id_x 无时序依赖、AAA、非恒真 ✓。

## 结论
**未过（打回）。** 修复 C-01（text_hash 改显式稳定 FNV-1a）+ I-01（schema 断言 text_hash 列）+ I-02（测试用 id_y 去时序依赖）后复审。注：FNV 改后旧 DefaultHasher 写入的 hash 不兼容——但本阶段库为新建无历史数据，ingest 对 NULL/不匹配 hash 天然按"未命中"处理，无需迁移。

---

## 复审结论（2026-05-31）

**status: 通过**

- **C-01**：`text_hash` 改手写 FNV-1a 64-bit（标准 FNV_PRIME/OFFSET + 字节 XOR + wrapping_mul），移除 DefaultHasher/Hash/Hasher import，注释标注"显式稳定跨版本一致、非加密仅判重"，持久化去重键合格。
- **I-01**：`schema_preembed_columns_clip_items` 已补 text_hash 列断言。
- **I-02**：`dedup_and_bump` 改持 `id_y` 显式 bump，去 top_id 时序依赖。
全量回归 24 passed，clippy --all-targets 零警告，无新引入高危。
