---
id: V1-F1-S02-test
type: test_report
level: 小功能
parent: V1-F1
created: 2026-05-30T22:36:20Z
status: 通过
commit: WIP
acceptance_ids: [V1-F1-A04, V1-F1-A05]
author: tester
---

# 测试报告 · 内容去重+置顶刷新（V1-F1-S02）

## 运行命令

```bash
# 1. clipboard 集成测试（含 A04/A05 + 回归 A01-A03/I-01）
cargo test --manifest-path src-tauri/Cargo.toml --test clipboard > /tmp/Tv1s2.log 2>&1

# 2. schema 回归
cargo test --manifest-path src-tauri/Cargo.toml --test schema > /tmp/Tv1s2s.log 2>&1

# 3. db 回归
cargo test --manifest-path src-tauri/Cargo.toml --test db > /tmp/Tv1s2d.log 2>&1

# 4. Clippy 静态检查（--all-targets）
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings > /tmp/Tv1s2c.log 2>&1

# 5. 无 TODO/FIXME
grep -rn 'TODO\|FIXME' src-tauri/src/
```

## 执行结果汇总

| 检查项 | 退出码 | 结论 |
|--------|--------|------|
| `cargo test --test clipboard` | 0 | 6 passed, 0 failed |
| `cargo test --test schema`（回归） | 0 | 7 passed, 0 failed |
| `cargo test --test db`（回归） | 0 | 6 passed, 0 failed |
| `cargo clippy --all-targets -- -D warnings` | 0 | 无 warning，无 error |
| `grep TODO\|FIXME src-tauri/src/` | 1（无匹配） | 无遗留标记 |

### clipboard 测试输出原文

```
test bump_no_new_record ... ok
test dedup_and_bump ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### schema 回归输出原文

```
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

### db 回归输出原文

```
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

## 用例与验收映射表

| 用例函数名 | 对应验收 ID | 验证行为 | 结果 |
|---|---|---|---|
| `dedup_and_bump` | V1-F1-A04 | 相同文本第二次 `ingest` 返回 `Bumped`（不新建行）；原条目成为最前（`top_id == 原 id`）；总行数仍为 2 | **pass** |
| `bump_no_new_record` | V1-F1-A05 | 显式调 `bump_to_top` 后行数仍为 2（无新记录产生）；目标条目成为最前 | **pass** |

## 回归确认

| 测试套件 | 覆盖内容 | 本次状态 |
|---|---|---|
| `clipboard`（A01/A02/A03/I-01） | S01 捕获核心逻辑 + OS 计数重置防御 | 全部 pass（6/6） |
| `schema` | `clip_items` 表结构与 V0 兼容性 | 全部 pass（7/7） |
| `db` | 基础 DB 操作与加密完整性 | 全部 pass（6/6） |

`clip_items` 表新增 `last_modified_utc` 列后，schema/db 套件全部通过，确认 V0 基线未被破坏。

## 覆盖缺口

| 缺口 | 说明 | 风险等级 |
|---|---|---|
| 并发写入去重 | 多线程同时 `ingest` 相同内容的竞态行为无专用用例 | 低（当前架构单线程写入，OS 层序列化保证） |
| `last_modified_utc` 精度 | 毫秒级时间戳在同一事务内双写的顺序性无专用断言 | 低（`bump_to_top` 已隐式验证排序正确性） |

## 结论

**门禁判定：放行**

V1-F1-A04（`dedup_and_bump`）和 V1-F1-A05（`bump_no_new_record`）全部 pass；S01 + schema + db 回归 19 用例全部通过；clippy（`--all-targets`）0 warning 0 error；无 TODO/FIXME 遗留。
