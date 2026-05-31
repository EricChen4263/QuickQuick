---
id: V2-F1-S05-review
type: review
level: 小功能
parent: V2-F1
children: []
created: 2026-05-31T09:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A06]
evidence: []
author: code-reviewer
---

# V2-F1-S05 审查报告 · 翻译缓存（键含 provider + 落 DB + LRU）

## 审查范围
- `src-tauri/src/translate/cache.rs`、`db.rs`(ensure_schema translation_cache)、`mod.rs`、`tests/translate.rs`(A06)、`tests/schema.rs`(断言)
依据：code-standards + 设计§4.1#5。

## 问题清单（Important，无 Critical）
**[I-1] cache_key 分隔符 XOR 语义失效（置信度 88）**
- 位置：`cache.rs`（`hash ^= 0u8 as u64`）。XOR 0 不改 hash，注释声称的 `\0` 段间分隔实际仅靠 mul；空段（source_lang=""）时前缀碰撞防护弱化。
- 修复：分隔符改非零字节 `hash ^= 0x01_u64; hash = hash.wrapping_mul(FNV_PRIME);`。

**[I-2] cache_get 的 LRU 刷新 UPDATE 路径未被测试覆盖（置信度 85）**
- 位置：`tests/translate.rs` A06-d 用 cache_put_at 绕过 cache_get；cache_get 内 `UPDATE last_used_utc` 从未实际执行，若条件写反测试不捕获。
- 修复：加可注入时间戳的 `cache_get_at(conn,key,now_ms)` 变体并测之（命中后查 last_used_utc 被刷新）；或 A06 中调 cache_get 后直查 last_used_utc 验证更新。

## 合规确认项
四元组键含 provider（换源必 miss，A06-a/b 覆盖）✓；哈希确定性 FNV-1a 非 std 随机 ✓；SQL 全参数化 ✓；无裸 unwrap/panic ✓；translation_cache 进 ensure_schema 幂等预埋 ✓；UPSERT ON CONFLICT 正确 ✓；LRU `ORDER BY last_used_utc ASC LIMIT` 删最旧 ✓；schema.rs 断言 8 列 ✓；无装饰注释/TODO；函数 ≤50 行；AAA 非恒真。

## 结论
**未过（打回）。** 修 I-1（非零分隔符）+ I-2（cache_get 刷新路径可测+覆盖）后复审。无 Critical。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-1 已解决**：cache.rs 分隔符改 `hash ^= 0x01_u64`（非零）+ mul，段边界真正改变哈希；测试 `cache_key_separator_empty_segment_differs_from_nonempty`（tests/translate.rs:932）assert_ne 空段 vs 非空段 key 不同，非恒真。
- **I-2 已解决**：新增 `cache_get_at(conn,key,now_ms)` 变体、cache_get 委托之；测试 `cache_get_at_hit_refreshes_last_used_utc_to_injected_timestamp`（:947）put@100→get_at@500→直查 DB last_used_utc==500，命中+LRU 刷新双断言、非恒真。
- 注：首轮复审误判"测试缺失"系仅在 cache.rs 找单测块；实际 cache 测试在集成文件 tests/translate.rs（acceptance verify path 一致），经独立实跑确认两测试各 1 passed。
无新引入≥80 高危；缓存主路径未破坏；cache 6 + 全量 91 绿。
