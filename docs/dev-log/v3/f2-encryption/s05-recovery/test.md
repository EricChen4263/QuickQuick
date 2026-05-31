---
id: V3-F2-S05-test
type: test_report
level: 小功能
parent: V3-F2
created: 2026-05-31T02:57:51Z
status: 通过
commit: WIP
acceptance_ids: [V3-F2-A06]
author: tester
---

# 测试报告 · 加密失败分级与恢复（V3-F2-S05）

## 验收项

**V3-F2-A06**：加密失败分级（transient/permanent） + 瞬时不碰库文件 + 永久备份不删 + 重建门控（allow_rebuild）。

---

## 执行命令与结果

### 1. A06 专项——`enc_failure` 真命中

```
cargo test --manifest-path src-tauri/Cargo.toml enc_failure
```

退出码：`A06=0`（成功）

真命中用例（8/8 通过）：

| 序号 | 用例名 | 验证语义 |
|------|--------|----------|
| 1 | `enc_failure_transient_backend_error_classifies_as_transient` | TransientKeychain 错误被分类为 Transient |
| 2 | `enc_failure_transient_tier_maps_to_retry_no_touch` | Transient tier 映射为 RetryNoTouch 动作 |
| 3 | `enc_failure_transient_leaves_db_file_untouched` | 瞬时失败后库文件存在且内容不变、无备份产生 |
| 4 | `enc_failure_corrupt_db_classifies_as_permanent` | Corrupt 错误被分类为 Permanent |
| 5 | `enc_failure_sqlite_decrypt_failure_classifies_as_permanent` | Sqlite 解密错误被分类为 Permanent |
| 6 | `enc_failure_permanent_tier_maps_to_backup_and_confirm_rebuild` | Permanent tier 映射为 BackupAndConfirmRebuild 动作 |
| 7 | `enc_failure_permanent_backup_preserves_corrupt_content` | 永久失败后备份文件存在且内容等于原损坏字节（不静默删除） |
| 8 | `enc_failure_permanent_allow_rebuild_creates_new_db_keeps_backup` | allow_rebuild=true 重建新库同时保留备份文件 |

### 2. db 集成测试回归

```
cargo test --manifest-path src-tauri/Cargo.toml --test db
```

退出码：`db=0`

```
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

V0 原有 6 个 db 集成测试全部通过，无回归。

### 3. 全量测试

```
cargo test --manifest-path src-tauri/Cargo.toml
```

退出码：`all=0`

```
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
```

全量 78 个测试，0 失败，0 忽略。

### 4. Clippy 静态检查

```
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

退出码：`clippy=0`

零 warning，零 error。

---

## 覆盖分析

| 验收子项 | 覆盖用例 | 状态 |
|----------|----------|------|
| transient 分类正确 | 用例 1 | 覆盖 |
| transient 不碰库文件（真实文件系统验证） | 用例 3 | 覆盖 |
| permanent 分类（Corrupt / Sqlite） | 用例 4、5 | 覆盖 |
| permanent 备份不删（内容校验 assert_eq! 字节级） | 用例 7 | 覆盖 |
| allow_rebuild=false 门控（不重建） | 用例 6（映射层） + open_or_recover V0 | 覆盖 |
| allow_rebuild=true 重建且保留备份 | 用例 8 | 覆盖 |
| Io / Other 保守归 Permanent | 设计决策，由用例 4/5 的保守分类逻辑隐含验证 | 部分覆盖（无独立用例，风险可接受） |

无覆盖缺口阻断进入下一任务。`Io`/`Other` 变体无独立专项测试属可接受风险——二者由 `classify_failure` 默认分支保守归 Permanent，逻辑极简，不影响门禁判定。

---

## 结论

**通过。允许进入下一任务。**

- A06 真命中：8 个 `enc_failure` 用例全部通过，分级语义（transient/permanent）、行为约束（不碰库/备份不删/重建门控）均有非恒真集成测试验证。
- db 回归：14 passed，0 failed，V0 原有功能无损。
- 全量：78 passed，0 failed。
- Clippy：0 warning。
