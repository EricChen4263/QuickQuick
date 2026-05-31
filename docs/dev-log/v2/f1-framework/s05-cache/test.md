---
id: V2-F1-S05-test
type: test_report
level: 小功能
parent: V2-F1
created: 2026-05-31T00:50:23Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A06]
author: tester
---

# 测试报告：V2-F1-S05 翻译缓存（I-1/I-2 修复后验证）

## 1. 执行命令与结果

| # | 命令 | exit | 通过数 | 结论 |
|---|------|------|--------|------|
| 1 | `cargo test --manifest-path src-tauri/Cargo.toml cache` | **0** | 6（translate）+ 1（schema）= 7 | 通过 |
| 2 | `cargo test --manifest-path src-tauri/Cargo.toml --test schema` | **0** | 9 | 通过 |
| 3 | `cargo test --manifest-path src-tauri/Cargo.toml --test translate` | **0** | 61 | 通过 |
| 4 | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | **0** | — | 零警告 |

## 2. 验收用例映射表（V2-F1-A06）

A06 验收标准：cache key 包含 provider（换源导致 miss）、LRU 淘汰最久未使用条目、`cache_get_at` 命中时刷新 `last_used_utc` 时间戳、空段分隔符与非空段产生不同 key。

### 集成测试（`tests/translate.rs` 过滤 `cache`，本次新增 I-1/I-2 修复用例）

| 测试用例 | 验证内容 | 结果 |
|---------|---------|------|
| `cache_key_includes_provider_lru_put_then_get_hits_with_correct_value` | put 后 get 命中并返回正确译文 | **通过** |
| `cache_key_includes_provider_lru_different_providers_produce_different_keys` | 同原文不同 provider → 产生不同 key | **通过** |
| `cache_key_includes_provider_lru_cross_provider_cache_get_is_miss` | provider A 写入、provider B 读取 → miss | **通过** |
| `cache_key_includes_provider_lru_evicts_least_recently_used_on_overflow` | 超出容量时淘汰最久未访问条目 | **通过** |
| `cache_key_separator_empty_segment_differs_from_nonempty`（I-1 修复） | 空段与非空段产生不同 key，防止 key 碰撞 | **通过** |
| `cache_get_at_hit_refreshes_last_used_utc_to_injected_timestamp`（I-2 修复） | `cache_get_at` 命中时将 `last_used_utc` 刷新为注入时间戳 | **通过** |

### schema 集成（`tests/schema.rs` 过滤 `cache`）

| 测试用例 | 验证内容 | 结果 |
|---------|---------|------|
| `schema_preembed_translation_cache_table_exists_with_required_columns` | `translation_cache` 表存在且含全部必需列 | **通过** |

**A06 合计：7 / 7 通过（含 I-1/I-2 修复后新增的 2 条用例）。**

## 3. I-1 / I-2 修复验证

| 缺陷编号 | 缺陷描述 | 修复验证用例 | 结果 |
|---------|---------|------------|------|
| I-1 | key 分隔符使用 `\0`，与空字符串段拼接时产生碰撞风险 | `cache_key_separator_empty_segment_differs_from_nonempty` | **通过** |
| I-2 | `cache_get_at` 命中后未更新 `last_used_utc`，导致 LRU 判断使用陈旧时间戳 | `cache_get_at_hit_refreshes_last_used_utc_to_injected_timestamp` | **通过** |

两条缺陷均由专项回归用例覆盖，行为符合预期，缺陷闭环。

## 4. 负向路径覆盖

| 负向场景 | 对应测试用例 | 预期行为 | 实际结果 |
|---------|------------|---------|---------|
| 换源后旧缓存不命中 | `cross_provider_cache_get_is_miss` | get 返回 `None` | 通过 |
| 容量溢出时 LRU 淘汰 | `evicts_least_recently_used_on_overflow` | 最久未访问被驱逐，最近留存 | 通过 |
| 空段 key 碰撞防护 | `cache_key_separator_empty_segment_differs_from_nonempty` | 空段与非空段 key 不同 | 通过 |

## 5. 回归套件全量

| 套件 | 通过 | 失败 | 跳过 |
|------|------|------|------|
| `cargo test cache`（A06 核心 7 条） | 7 | 0 | 0 |
| `cargo test --test schema`（全量，含 translation_cache 表断言） | 9 | 0 | 0 |
| `cargo test --test translate`（全量，含 S01–S05 全部用例） | 61 | 0 | 0 |
| `clippy --all-targets -D warnings` | — | 0 警告 | — |

**总计：61 个 translate 用例全绿，9 个 schema 用例全绿，clippy 零警告。**

相比 I-1/I-2 修复前（translate × 59，schema × 9），本轮 translate 新增 2 条（59 → 61），对应 I-1 空段 key 碰撞用例与 I-2 时间戳刷新用例，无任何回归破坏。

## 6. 数据隔离确认

全部 cache LRU 用例在进程内存中构建 `TranslateCache` 实例，不写磁盘 SQLite、不访问网络、不依赖系统 keychain，测试结果确定性强，无环境噪声。

## 7. 覆盖缺口

无缺口。

- A06 四项语义（put/get 命中、provider 参与 key、LRU 淘汰、get_at 刷新时间戳）各有专项用例。
- I-1 空段碰撞防护通过 `cache_key_separator_empty_segment_differs_from_nonempty` 断言。
- I-2 时间戳刷新通过 `cache_get_at_hit_refreshes_last_used_utc_to_injected_timestamp` 断言。
- DB 表结构通过 `schema_preembed_translation_cache_table_exists_with_required_columns` 覆盖。
- 全量回归（translate × 61 + schema × 9）确认 S01–S04 已有功能无副作用破坏。

## 8. 结论

**门禁：放行。**

A06 通过（7 / 7，含 I-1/I-2 修复新增用例），translate 全量回归通过（61 / 61），schema 全量回归通过（9 / 9），clippy 零警告。V2-F1-S05 翻译缓存缺陷修复验收完毕，可进入下一阶段。
