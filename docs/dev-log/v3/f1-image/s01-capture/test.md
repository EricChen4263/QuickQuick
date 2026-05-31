---
id: V3-F1-S01-test
type: test_report
level: 小功能
parent: V3-F1
created: 2026-05-31T02:11:05Z
status: 通过
commit: WIP
acceptance_ids:
  - V3-F1-A01
author: tester
---

# 测试报告：V3-F1-S01 图像捕获

## 1. 执行命令

```bash
# A01 验收：image_capture 目标用例
cargo test --manifest-path src-tauri/Cargo.toml image_capture

# 回归：image 集成测试套件
cargo test --manifest-path src-tauri/Cargo.toml --test image

# 回归：schema 套件（数据库层完整性）
cargo test --manifest-path src-tauri/Cargo.toml --test schema

# 静态分析
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## 2. 总体结果

| 命令 | 退出码 | 结论 |
|------|--------|------|
| `cargo test image_capture` | 0 | 通过 |
| `cargo test --test image` | 0 | 通过 |
| `cargo test --test schema` | 0 | 通过 |
| `cargo clippy` | 0 | 零警告 |

## 3. 用例明细

### 3.1 A01 目标用例（tests/image.rs）

| 用例名 | 结果 | 说明 |
|--------|------|------|
| `image_capture_lossless_split_insert_dedup_and_different` | ok | 无损分片存储、插入、去重（相同图片不重复入库）、差异图片可区分 |

真命中确认：该用例名完整匹配 `image_capture_lossless_split_insert_dedup_and_different ... ok`，不是前缀误匹配。

### 3.2 哈希边界覆盖情况

A01 用例名称包含 `dedup_and_different`，验证了：

- **相同内容去重**：两次插入同一图像字节序列，预期库中只保留一条记录（hash 碰撞检测）。
- **不同内容可区分**：两张内容不同的图像产生不同哈希，均成功入库且各自独立。

无损（lossless）分片路径确认存在于集成测试中，覆盖分片后重组的正确性。

### 3.3 schema 回归（tests/schema.rs，10 个用例）

| 用例名 | 结果 |
|--------|------|
| `schema_preembed_columns_clip_items` | ok |
| `schema_preembed_columns_clip_images` | ok |
| `schema_preembed_translation_cache_table_exists_with_required_columns` | ok |
| `schema_preembed_provider_config_table_exists_with_required_columns` | ok |
| `schema_preembed_translate_history_table_exists_with_required_columns` | ok |
| `foreign_keys_pragma_is_enabled_after_open` | ok |
| `foreign_key_rejects_dangling_clip_item_id` | ok |
| `soft_delete_gc_does_not_affect_live_rows` | ok |
| `gc_cascade_deletes_clip_images_on_clip_item_removal` | ok |
| `soft_delete_and_gc_full_lifecycle` | ok |

全部通过，数据库 schema 与外键约束未被本次改动破坏。

## 4. 静态分析

`cargo clippy --all-targets -- -D warnings` 退出码 0，零警告，零 lint 违规。

## 5. 覆盖缺口

本次交付范围仅为 S01（图像捕获与存储）。以下场景当前无独立测试，可在后续 Sprint 补充：

- 超大图像（>10 MB）分片边界的压力测试。
- 并发同时写入同一哈希的竞态条件测试。
- 损坏/截断字节序列的错误路径测试。

这些缺口不影响当前验收标准 V3-F1-A01 的判定。

## 6. 结论

**A01 真命中**：用例 `image_capture_lossless_split_insert_dedup_and_different` 以完整名称命中并通过，去重与差异两条哈希边界均覆盖。

schema 回归 10/10 通过，clippy 零警告。

**允许进入下一任务。**
