---
id: V0-F3-S05-test
type: test_report
level: 小功能
parent: V0-F3
created: 2026-05-30T22:07:34Z
status: 通过
commit: f2af018
acceptance_ids: [V0-F3-A01, V0-F3-A02, V0-F3-A04, V0-F3-A05, V0-F3-A06]
author: tester
---

# 测试报告 · V0-F3-S05 加密 DB + Schema + 软删 GC + 恢复

## 1. 运行命令

```bash
# A01/A02/A06 — DB 集成测试
cargo test --manifest-path src-tauri/Cargo.toml --test db > /tmp/T5db.log 2>&1

# A04/A05 — Schema + 软删 GC 集成测试
cargo test --manifest-path src-tauri/Cargo.toml --test schema > /tmp/T5sc.log 2>&1

# 全量 Rust 测试
cargo test --manifest-path src-tauri/Cargo.toml > /tmp/T5all.log 2>&1

# Clippy 静态检查
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings > /tmp/T5c.log 2>&1

# 构建
cargo build --manifest-path src-tauri/Cargo.toml > /tmp/T5b.log 2>&1

# TODO/FIXME 扫描
grep -rn 'TODO\|FIXME' src-tauri/src/
```

## 2. 结果：通过

| 检查项 | exit code | 结论 |
|--------|-----------|------|
| cargo test --test db | 0 | 通过（6/6） |
| cargo test --test schema | 0 | 通过（4/4） |
| cargo test（全量） | 0 | 通过（28/28） |
| clippy -D warnings | 0 | 通过（无 warning） |
| cargo build | 0 | 通过 |
| TODO/FIXME 扫描 | 1（无匹配） | 无遗留标记 |

## 3. 用例明细

### 3.1 DB 集成测试（tests/db.rs）— A01 / A02 / A06

| 用例名 | 结果 | 验收项 | 说明 |
|--------|------|--------|------|
| `db_create_auto_creates_file_on_first_run` | ok | A01 | 首次运行自动创建数据库文件 |
| `db_create_is_idempotent_on_subsequent_opens` | ok | A01 | 后续打开幂等，不重复初始化 |
| `db_encrypt_wrong_key_returns_error` | ok | A02 | 错误密钥打开已加密 DB 返回错误（非静默） |
| `db_encrypt_ciphertext_on_disk` | ok | A02 | 已加密文件内容为密文，不含明文 |
| `db_recovery_corrupt_file_creates_backup_and_returns_err` | ok | A06 | 损坏文件改名备份并返回错误，不静默删除 |
| `db_recovery_allow_rebuild_creates_new_db_and_keeps_backup` | ok | A06 | 显式确认后重建新 DB，旧备份文件保留 |

**合计：6 用例 / 6 通过 / 0 失败 / 0 跳过**

### 3.2 Schema + 软删 GC 集成测试（tests/schema.rs）— A04 / A05

| 用例名 | 结果 | 验收项 | 说明 |
|--------|------|--------|------|
| `schema_preembed_columns_clip_items` | ok | A04 | clip_items 表预埋 embedding 列（未使用时为 NULL） |
| `schema_preembed_columns_clip_images` | ok | A04 | clip_images 表预埋 embedding 列 |
| `soft_delete_and_gc_full_lifecycle` | ok | A05 | 软删非物理删除（行仍存在）→ GC 后物理清除 |
| `soft_delete_gc_does_not_affect_live_rows` | ok | A05 | GC 不影响未软删的存活行 |

**合计：4 用例 / 4 通过 / 0 失败 / 0 跳过**

### 3.3 全量 Rust 测试汇总

| 测试套 | 通过 | 失败 |
|--------|------|------|
| lib（单元测试） | 10 | 0 |
| db（集成） | 6 | 0 |
| schema（集成） | 4 | 0 |
| hotkey（集成） | 3 | 0 |
| autostart（集成） | 3 | 0 |
| keyprovider（集成） | 2 | 0 |
| 其余（空套） | 0 | 0 |
| **总计** | **28** | **0** |

### 3.4 验收项逐项确认

| 验收 ID | 覆盖测试 | 断言要点 | 结果 |
|---------|----------|----------|------|
| A01 db_create | `db_create_auto_creates_file_on_first_run`、`db_create_is_idempotent_on_subsequent_opens` | 文件存在 + 可写入查询 + 幂等 | **pass** |
| A02 db_encrypt | `db_encrypt_wrong_key_returns_error`、`db_encrypt_ciphertext_on_disk` | 错密钥报错 + 磁盘为密文 | **pass** |
| A04 schema_preembed | `schema_preembed_columns_clip_items`、`schema_preembed_columns_clip_images` | 两表均含 embedding 列 | **pass** |
| A05 soft_delete | `soft_delete_and_gc_full_lifecycle`、`soft_delete_gc_does_not_affect_live_rows` | 软删非物理 + GC 物理清除 + 不误删存活行 | **pass** |
| A06 db_recovery | `db_recovery_corrupt_file_creates_backup_and_returns_err`、`db_recovery_allow_rebuild_creates_new_db_and_keeps_backup` | 损坏改名备份不静默删 + 显式确认才重建 | **pass** |

## 4. 覆盖缺口分析

| 缺口 | 风险等级 | 说明 |
|------|----------|------|
| Schema 迁移版本升级路径 | 低 | 当前 schema 固定为 v1，迁移逻辑留待后续功能迭代时补充 |
| GC 的批量上限与性能 | 低 | v0 数据量极小，批量性能测试留待 v1 规模时补充 |
| DB 文件权限（chmod 0600） | 低 | 安全加固项，当前依赖操作系统默认权限，不影响功能验收 |
| 并发写入冲突 | 低 | v0 单进程场景无需覆盖 |

以上缺口均不影响 A01/A02/A04/A05/A06 验收条件。

## 5. Clippy / 构建输出

clippy：无任何 warning 或 error（exit=0，空输出）。  
build：干净编译，exit=0。

## 6. 结论

**放行。** V0-F3-A01、A02、A04、A05、A06 五项验收均通过，全量 Rust 测试 28/28 全绿，clippy 零 warning，构建成功，无 TODO/FIXME 遗留。允许进入下一任务。
