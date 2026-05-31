---
id: V2-F1-S05-code
type: coding_record
level: 小功能
parent: V2-F1
children: []
created: 2026-05-31T00:43:35Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A06]
evidence:
  - src-tauri/src/translate/cache.rs
  - src-tauri/src/db.rs
  - src-tauri/tests/translate.rs
  - src-tauri/tests/schema.rs
author: coder
---

# 编码记录 · 翻译缓存（S05-cache）

## 做了什么

实现了基于 SQLite 持久化的翻译结果缓存，以 `(source_text, source_lang, target_lang, provider_id)` 四元组 FNV-1a 哈希为键，命中时刷新 `last_used_utc` 实现 LRU 语义，写入后自动淘汰超出容量的最旧条目；`provider_id` 是键的组成部分，换源后键必然不同，天然隔离不同翻译源的缓存。

## 关键决策与理由

- **cache_key 组成**：将 `provider_id` 纳入四元组键，而非仅用文本三元组。理由：同一原文在 DeepL 与 MyMemory 下的译文可能不同；若共用一个键，换源后会命中另一家的旧缓存，导致翻译结果错乱。四元组保证换源必 miss，各 provider 缓存完全隔离。

- **FNV-1a 哈希 + 段间 `\0` 分隔符**：采用 FNV-1a 64-bit 算法而非 `std::hash`，原因是 `std::hash` 受 `RUSTFLAGS` 随机化影响，跨进程重启后同一输入可能得到不同哈希，不适合持久化键。段间插入 `\0` 防止前缀碰撞（如 `("ab","c")` 与 `("a","bc")` 拼接后字节序列相同）。

- **`last_used_utc` 刷新作为 LRU 依据**：`cache_get` 命中时立即 UPDATE `last_used_utc`；`cache_put_at` 写入后调用 `cache_evict_lru`，按 `last_used_utc ASC` 物理删除超出容量的最旧条目。没有引入 in-process LRU 结构，全部依赖 DB，重启后访问历史自然保留。

- **`cache_put_at` 注入时间戳**：为便于测试控制 LRU 顺序，拆出 `cache_put_at(conn, entry, capacity, now_ms)` 供测试注入确定性时间戳，`cache_put` 内部调用它并传入 `current_utc_ms()`。同毫秒写入多条时若不注入，LRU 顺序不确定，测试会随机失败。

- **`translation_cache` 表进 `ensure_schema` 预埋**：在 `db::ensure_schema` 中一并建表，而非在首次缓存写入时懒建表。理由：schema 迁移统一管控，避免各功能模块各自偷偷建表导致版本追踪混乱；此外 schema 回归测试可在不触碰翻译流程的情况下独立验证表结构。

## 改动文件

- `src-tauri/src/translate/cache.rs` — 新增文件：实现 `cache_key`、`cache_get`、`cache_put`、`cache_put_at`、`cache_evict_lru` 五个函数及 `CacheEntry` 结构体。
- `src-tauri/src/db.rs` — 在 `ensure_schema` 中追加 `translation_cache` 建表 DDL（8 列：`cache_key` PRIMARY KEY、`source_text`、`source_lang`、`target_lang`、`provider_id`、`translated`、`created_utc`、`last_used_utc`）。
- `src-tauri/tests/translate.rs` — 新增 4 个 cache 测试（A06-a 至 A06-d）：provider 不同键不同、换源必 miss、put 后 get 命中返回正确值、容量溢出后 LRU 淘汰最旧条目。
- `src-tauri/tests/schema.rs` — 新增 1 个 schema 回归测试（A06）：验证 `translation_cache` 表由 `ensure_schema` 预埋，且含全部 8 个必要列。

## 审查修复（打回第 1 次，2026-05-31）

### I-1：非零分隔符修复

- **位置**：`cache.rs` `cache_key` 函数段间分隔逻辑。
- **修改**：将 `hash ^= 0u8 as u64`（XOR 0 不改变哈希状态，分隔形同虚设）改为 `hash ^= 0x01_u64;`，保留后续 `hash = hash.wrapping_mul(FNV_PRIME);`。非零字节 XOR 后再乘以素数，段边界真正产生状态分叉，空段与非空段路径不再碰撞。
- **新增测试**：`cache_key_separator_empty_segment_differs_from_nonempty`——`("a","","c","d")` 与 `("a","x","c","d")` 必须产生不同键，证明分隔生效。TDD 红（编译通过但逻辑错时失败）→ 绿（改为非零字节后通过）。

### I-2：cache_get_at 可注入时间戳变体

- **位置**：`cache.rs`，新增公开函数 `cache_get_at(conn, key, now_ms)`。
- **修改**：将原 `cache_get` 内联的查询 + UPDATE 逻辑提取为 `cache_get_at(conn, key, now_ms)`，命中时用传入的 `now_ms` 刷新 `last_used_utc`；`cache_get` 改为委托调用 `cache_get_at(conn, key, current_utc_ms())`，对外行为不变。
- **新增测试**：`cache_get_at_hit_refreshes_last_used_utc_to_injected_timestamp`——put(t=100) → `cache_get_at(t=500)` 命中 → 直查 `last_used_utc==500`，AAA 结构，非恒真（写入时是 100，断言 500 只有刷新路径真实执行才成立）。TDD 红（`cache_get_at` 不存在，编译失败）→ 绿（实现后通过）。

## 自测结论（TDD 红-绿-重构）

**TDD 循环**：先写 4 个测试（provider 键隔离、换源 miss、命中返回值、LRU 淘汰），此时 `cache.rs` 不存在，编译即红；随后逐步实现 `cache_key` → `cache_get` → `cache_put_at` → `cache_evict_lru`，每步只写刚好令对应测试变绿的代码；重构阶段提取 `CacheEntry` 结构体以消除 `cache_put_at` 参数过长问题，测试始终保持全绿。schema 测试在 `ensure_schema` 加入建表 DDL 后同步变绿。

**code-standards 自检**：

- 格式：`cargo fmt` 通过，无手动格式修改。
- 命名：函数使用「动词+名词」（`cache_get` / `cache_put` / `cache_evict_lru`），结构体 `CacheEntry` 清晰描述用途；无 `tmp` / `flag` 等模糊命名。
- 函数长度：最长函数 `cache_put_at` 约 30 行，`cache_evict_lru` 约 25 行，均在 50 行以内；嵌套不超过 2 层。
- 注释：模块级文档说明职责与 LRU 语义，函数级 `///` 注释说明行为与 Errors；段间 `\0` 分隔符处注释写明「防前缀碰撞」原因，均为「为什么」而非「是什么」；无装饰性注释，无注释掉的死代码。
- 类型/安全：SQL 全部参数化查询（`rusqlite::params!`），无字符串拼接 SQL；`cache_key` 显式使用 FNV-1a 常量，不依赖随机化 hasher。
- 性能：LRU 淘汰使用单条 `DELETE ... WHERE cache_key IN (SELECT ... LIMIT n)`，一次 SQL 完成，不做 N 次循环删除。
- 测试：cache 4 个测试全部通过；schema 9 个回归测试（含新增 A06）全部通过；`cargo clippy` 0 警告。
- 无 `TODO` / `FIXME` 遗留。
