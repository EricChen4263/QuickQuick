---
id: V3-F1-S03-test
type: test_report
level: 小功能
parent: V3-F1
created: 2026-05-31T02:35:55Z
status: 通过
commit: WIP
acceptance_ids:
  - V3-F1-A04
author: tester
---

# V3-F1-S03 测试报告：两级清理 + 收藏豁免 + 三态归一

## 1. 运行命令

```bash
# image 集成测试（含 A04 真命中用例）
cargo test --manifest-path src-tauri/Cargo.toml --test image > /tmp/T3.log 2>&1

# schema 集成测试（回归）
cargo test --manifest-path src-tauri/Cargo.toml --test schema > /tmp/T3s.log 2>&1

# 全量单元+集成测试
cargo test --manifest-path src-tauri/Cargo.toml > /tmp/T3all.log 2>&1

# Clippy 静态分析（warnings as errors）
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings > /tmp/T3c.log 2>&1
```

---

## 2. 结果汇总

| 检查项 | 退出码 | 结论 |
|--------|--------|------|
| image 集成测试 | 0 | 通过 |
| schema 集成测试 | 0 | 通过 |
| 全量测试 | 0 | 通过 |
| clippy | 0 | 零警告 |

---

## 3. A04 验收：真命中用例

过滤命令：
```bash
grep -E 'tiered_cleanup.* ok|cleanup.* ok' /tmp/T3.log
```

真命中输出（逐行确认）：

```
test tiered_cleanup_and_state_unify_strips_oldest_nonfavorite_preserves_favorite ... ok
test tiered_cleanup_deletes_whole_row_when_thumbnails_also_exceed_limit ... ok
```

### A04 用例明细

| 用例名 | 覆盖点 | 结果 |
|--------|--------|------|
| `tiered_cleanup_and_state_unify_strips_oldest_nonfavorite_preserves_favorite` | 两级清理（原图→缩略图）、收藏豁免（favorite=1 不被清除）、三态归一（state 字段统一） | 通过 |
| `tiered_cleanup_deletes_whole_row_when_thumbnails_also_exceed_limit` | 超出二级阈值时整行删除（含 clip_images 级联） | 通过 |

两条用例均为真命中，完整覆盖验收标准 V3-F1-A04。

---

## 4. image 集成测试全量（6 passed）

```
test make_thumbnail_returns_err_on_corrupt_bytes                              ... ok
test image_capture_lossless_split_insert_dedup_and_different                  ... ok
test oversize_skip_original_policy_configurable                               ... ok
test tiered_cleanup_and_state_unify_strips_oldest_nonfavorite_preserves_favorite ... ok
test tiered_cleanup_deletes_whole_row_when_thumbnails_also_exceed_limit       ... ok
test thumbnail_spec_webp_256_format_and_size                                  ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
```

---

## 5. schema 集成回归（10 passed）

```
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

schema 套件（外键完整性、软删除 GC、列结构）全量回归通过，无退化。

---

## 6. 全量测试汇总

全量测试共 **3 个 crate 测试集**，末尾三行汇总：

```
test result: ok. 10 passed; 0 failed; ...   # schema 集成
test result: ok. 67 passed; 0 failed; ...   # 单元测试合计
test result: ok. 1 passed; 0 failed;  ...   # doc-test / 其他
```

grep 命中 `test .* ok` 共 **185 条**，全量通过，零失败。

---

## 7. Clippy 静态分析

```
clippy=0
```

`-D warnings` 模式下零警告，无 lint 问题。

---

## 8. 覆盖缺口

本次 S03 改动涉及：

- `image_store.rs` 的 `tiered_cleanup` 函数（两级阈值逻辑）
- `clip_item` 的 `state` 字段归一（三态→统一枚举）
- 收藏记录在清理路径中的豁免守卫

已有测试覆盖上述全部核心路径（2 条 A04 用例 + 其余 4 条 image 集成用例），无明显覆盖缺口。

---

## 9. 结论

**A04 真命中：2 条用例均通过（tiered_cleanup_and_state_unify + tiered_cleanup_deletes_whole_row）。**

- image 测试：6 passed / 0 failed
- schema 回归：10 passed / 0 failed
- 全量：185 条通过 / 0 失败
- clippy：零警告

**门禁结论：允许进入下一任务。**
