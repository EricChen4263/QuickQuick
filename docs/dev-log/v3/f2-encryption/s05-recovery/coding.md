---
id: V3-F2-S05-code
type: coding_record
level: 小功能
parent: V3-F2
created: 2026-05-31T02:56:20Z
status: 通过
commit: WIP
acceptance_ids: [V3-F2-A06]
author: coder
---

# 编码记录 · 加密失败分级与恢复（V3-F2-S05）

## 做了什么

在 `src-tauri/src/db.rs` 中补充了加密失败分级与恢复语义，覆盖验收项 V3-F2-A06。具体新增：

- **`DbError::TransientKeychain(String)`**：新增变体，表示钥匙串被拒/系统锁定等瞬时错误。配套构造方法 `transient_keychain_error(msg)`，测试与业务层均可使用。
- **`FailureTier` 枚举**（`Transient` / `Permanent`）：区分可重试的瞬时失败与需要恢复流程的永久失败。
- **`RecoveryAction` 枚举**（`RetryNoTouch` / `BackupAndConfirmRebuild`）：描述分级后应采取的恢复动作，供调用方决策。
- **`classify_failure(err: &DbError) -> FailureTier`**：将 `DbError` 分类——`TransientKeychain` → `Transient`；其余所有变体（`Corrupt` / `Sqlite` / `Io` / `Other`）→ `Permanent`（保守归类）。
- **`recovery_action(tier: FailureTier) -> RecoveryAction`**：纯映射——`Transient` → `RetryNoTouch`；`Permanent` → `BackupAndConfirmRebuild`。

在 `src-tauri/tests/db.rs` 追加 8 个 `enc_failure` 前缀集成测试，全部通过。

## 失败分级设计

### 瞬时（Transient）
- 触发条件：`DbError::TransientKeychain`——对应 `KeyError::Backend`（钥匙串被系统拒绝/锁定），属于外部状态问题，库文件完好无损。
- 恢复动作：`RetryNoTouch`——调用方仅提示用户重试，绝不改名/删除/重建库文件。
- 测试验证：`enc_failure_transient_leaves_db_file_untouched` 写入真实文件后调用 classify+recovery_action，断言原文件存在且内容不变、目录中无备份文件。

### 永久（Permanent）
- 触发条件：`DbError::Corrupt`（库损坏已备份）、`DbError::Sqlite`（解密失败/格式错误）、`DbError::Io`（IO 失败，保守归类）、`DbError::Other`（业务层未知错误）。
- 恢复动作：`BackupAndConfirmRebuild`——旧库 rename 改名备份（绝不删除），`allow_rebuild=false` 时返回 `Err` 不建空库，`allow_rebuild=true` 时才重建。复用 V0 `open_or_recover` 实现。
- 测试验证：`enc_failure_permanent_backup_preserves_corrupt_content` 断言备份文件存在且内容等于原损坏字节（`assert_eq!(backup_content, corrupt_data)`）；`enc_failure_permanent_allow_rebuild_*` 验证两个 allow_rebuild 分支。

## 关键决策与理由

1. **`TransientKeychain` 作为独立变体而非复用 `Other`**：`Other` 是通用桶，若用它表示瞬时失败，`classify_failure` 无法可靠区分"瞬时钥匙串拒绝"与"永久业务层错误"。独立变体使分类规则精确且无歧义，符合单一职责。

2. **`classify_failure` 保守归类 `Io`/`Other` 为 Permanent**：IO 失败（磁盘满/权限拒绝）与未知错误的恢复路径不确定，保守归 Permanent 让用户介入，不擅自重试或无声失败，符合"永不静默删库"硬约束。

3. **复用 V0 `open_or_recover` 实现永久路径**：V0 已经实现了 rename 备份 + allow_rebuild 门控，语义完全对齐设计§六#1。本次只补分级判断层，不重复实现备份逻辑，避免 DRY 违反。

4. **两个纯函数（`classify_failure` / `recovery_action`）不持有状态、无副作用**：便于测试、易于组合，调用方可独立调用任一函数或组合使用，不强耦合。

5. **`transient_keychain_error` 构造方法**：让测试用直观的构造器模拟 KeyProvider 返回 Backend 错误的场景，避免测试代码直接构造内部变体字符串，提升可读性。

## 改动文件

- `src-tauri/src/db.rs` — 在 `DbError` 新增 `TransientKeychain` 变体及 `transient_keychain_error` 构造方法；新增 `FailureTier`、`RecoveryAction` 枚举；新增 `classify_failure`、`recovery_action` 两个公开纯函数（共约 88 行新增代码）
- `src-tauri/tests/db.rs` — 在文件头新增 `use` 导入；追加 8 个 `enc_failure` 前缀集成测试，覆盖瞬时/永久分级、不碰库文件、备份不静默删、两个 allow_rebuild 分支（共约 180 行新增测试代码）

## TDD 过程（红-绿-重构）

1. **RED**：先追加测试至 `tests/db.rs`，导入 `classify_failure`、`recovery_action`、`FailureTier`、`RecoveryAction`、`transient_keychain_error` 等符号。`cargo test` 编译失败，报 4 个 `unresolved import` 和 1 个 `no variant/function`——确认因功能未实现而失败，非语法环境错误。

2. **GREEN**：在 `db.rs` 中添加 `TransientKeychain` 变体、`transient_keychain_error` 构造方法、`FailureTier`、`RecoveryAction`、`classify_failure`、`recovery_action`。`cargo test --test db enc_failure` 结果：8 passed，0 failed。

3. **REFACTOR**：`cargo clippy --all-targets -D warnings` 报两处测试文件中 `&backups[0].path()` 多余借用（`needless_borrows_for_generic_args`），改为 `backups[0].path()`。改后重跑全量测试保持绿，clippy 零警告。实现代码结构已满足单一职责和函数长度要求，无需进一步重构。

## code-standards 自检

| 规范项 | 状态 | 说明 |
|---|---|---|
| 格式 | 通过 | `cargo fmt` 兼容，无多余空行或不一致缩进 |
| 命名 | 通过 | `FailureTier` / `RecoveryAction` 大驼峰枚举；`classify_failure` / `recovery_action` snake_case 动词+名词；无 `tmp`/`flag` |
| 函数长度 ≤ 50 行 | 通过 | `classify_failure` 7 行；`recovery_action` 4 行；`transient_keychain_error` 3 行 |
| 嵌套 ≤ 3 层 | 通过 | 最深 1 层（match 内单行返回） |
| 注释写"为什么" | 通过 | 各类型/函数 doc 说明设计意图与对应设计文档锚点；无装饰性分隔符 |
| 类型安全 | 通过 | 全类型推断，无裸 unwrap/panic（`transient_keychain_error` 用 `impl Into<String>` 泛型避免强制 String 转换） |
| 单一职责 | 通过 | `classify_failure` 仅分类；`recovery_action` 仅映射动作；两者独立可组合 |
| 无裸 unwrap/panic | 通过 | 新增代码无 unwrap/expect/panic（V0 中 `backup_corrupt_file` 的 `unwrap_or_default` 为既有代码，未改动） |
| 安全红线 | 通过 | 无密钥、无用户输入未校验路径、无敏感日志；"永不静默删库"硬约束由 V0 `backup_corrupt_file`（rename 而非 remove）保证 |
| 测试充分性 | 通过 | 8 个集成测试：瞬时分类/映射/不碰文件×3；永久分类（Corrupt/Sqlite）×2；永久映射×1；永久备份不删×1；allow_rebuild 两分支×2；AAA 结构，非恒真 |
| clippy | 通过 | 0 warning（修复了 2 处 needless_borrows_for_generic_args） |
| 无 TODO/FIXME | 通过 | grep 结果为空 |

## 审查修复记录（打回第 1 次）

按 code-reviewer 打回意见（review.md）逐项修复，完成于 2026-05-31：

- **I-1 注释矛盾**：`tests/db.rs` `enc_failure_transient_backend_error_classifies_as_transient` Arrange 注释从"用 DbError::Other 表示瞬时钥匙串拒绝"改为"用 DbError::TransientKeychain 表示上层瞬时钥匙串拒绝"，与实际调用一致。
- **I-2 apply_recovery_action 非恒真**：`src-tauri/src/db.rs` 新增公开函数 `apply_recovery_action(path, action) -> Result<(), DbError>`：`RetryNoTouch` → 不碰文件直接 `Ok`；`BackupAndConfirmRebuild` → 调用 `backup_corrupt_file` 执行改名备份。`tests/db.rs` 中 `enc_failure_transient_leaves_db_file_untouched` 改为写入真实文件后调用 `apply_recovery_action(&db_path, RetryNoTouch)`，断言文件字节未变且无备份（若实现误删/改则失败 = 非恒真）；保留 classify→action 映射断言。
- **I-3 file_name 显式报错**：`backup_corrupt_file` 的 `path.file_name().unwrap_or_default()` 改为 `.ok_or_else(|| std::io::Error::new(InvalidInput, "backup_corrupt_file: 路径无文件名分量"))?`，路径无文件名分量时显式返回 `DbError::Io` 而非静默退化为空名备份。
- **I-4 去装饰注释**：`tests/db.rs` 第 188 行 `// ===== V3-F2-A06 ... =====` 改为普通 `// V3-F2-A06：encryption_failure_tiered 失败分级与恢复`；grep 确认 src/tests 无 `=====`/`──`/`═══`/`━━━` 残留。

回归结果：enc_failure 8/8 通过；全量 all=0；clippy=0；deco=1（无装饰）；todo=1（无 TODO）。

## 测试结果

```
# enc_failure 专项（A06 真命中）
cargo test --manifest-path src-tauri/Cargo.toml --test db enc_failure
test enc_failure_transient_backend_error_classifies_as_transient ... ok
test enc_failure_transient_tier_maps_to_retry_no_touch           ... ok
test enc_failure_transient_leaves_db_file_untouched              ... ok
test enc_failure_corrupt_db_classifies_as_permanent              ... ok
test enc_failure_sqlite_decrypt_failure_classifies_as_permanent  ... ok
test enc_failure_permanent_tier_maps_to_backup_and_confirm_rebuild ... ok
test enc_failure_permanent_backup_preserves_corrupt_content      ... ok
test enc_failure_permanent_allow_rebuild_creates_new_db_keeps_backup ... ok
test result: ok. 8 passed; 0 failed

# db 集成测试回归（含 V0 原有 6 个测试）
test result: ok. 14 passed; 0 failed

# 全量（所有 crate 测试）
test result: ok. 32 passed; 0 failed
test result: ok. 10 passed; 0 failed
test result: ok. 67 passed; 0 failed
test result: ok. 1 passed; 0 failed

# clippy
clippy=0（零 warning）
```
