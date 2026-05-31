---
id: V3-F2-S05-review
type: review
level: 小功能
parent: V3-F2
children: []
created: 2026-05-31T10:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F2-A06]
evidence: []
author: code-reviewer
---

# 代码审查 · V3-F2-S05 加密失败分级与恢复

## 审查范围
- `src-tauri/src/db.rs`（FailureTier/classify_failure/RecoveryAction/recovery_action/backup_corrupt_file/open_or_recover）+ `tests/db.rs`（enc_failure 8 用例）
依据：code-standards + 设计§六#1。

## 核心安全约束验证（通过）
- **瞬时不碰库**：open_or_recover 仅 `Err(DbError::Sqlite(_))` 触发备份；TransientKeychain 走 `Err(other)=>return` 直接返回，不 rename/不重建。✓
- **永久备份不静默删**：backup_corrupt_file 用 `fs::rename` 改名保留绝不删；allow_rebuild=false 不建库、=true 才建；测试字节级断言备份保留损坏内容。✓
- **失败分级**：classify_failure 覆盖全变体（TransientKeychain→Transient；Corrupt/Sqlite/Io/Other→Permanent），与§六#1 对齐。✓

## 问题清单（Important，无 Critical）
**[I-1] 测试注释与实际变体矛盾（置信度 90）**
- 位置：`tests/db.rs`（注释称 `DbError::Other` 表瞬时钥匙串拒绝，实际用 `DbError::TransientKeychain`；Other 在 classify_failure 归 Permanent，与意图相反，误导）。
- 修复：注释改"用 DbError::TransientKeychain 表示上层瞬时钥匙串拒绝"。

**[I-2] transient_leaves_db_file_untouched 文件断言恒真（置信度 88）**
- 位置：`tests/db.rs`（Act 仅调 classify_failure/recovery_action 纯函数不碰 FS，"文件存在/内容不变/无备份"三断言恒真，不能证明 db 层 Transient 路径不碰文件）。
- 修复（二选一）：① 补真实集成——写库文件→走会触发 Transient 的恢复调度（如 apply_recovery(RetryNoTouch) 为 no-op）→断言文件字节未变（buggy 实现会失败=非恒真）；② 明确注释为纯函数语义测试 + 文件断言仅辅助。优先 ①。

**[I-3] backup_corrupt_file file_name().unwrap_or_default() 静默掩盖逻辑错误（置信度 82）**
- 位置：`db.rs`（裸根/分隔符结尾路径 file_name()=None→空 OsStr→备份名退化 `.corrupt-<ts>`）。
- 修复：`.ok_or_else(|| io::Error::new(InvalidInput, "路径无文件名分量"))?` 显式报错。

**[I-4] tests/db.rs 装饰分隔注释（置信度 80）**
- 位置：`tests/db.rs`（`// ===== V3-F2-A06 ... =====`，`=====` 属装饰禁令）。
- 修复：改普通 `// V3-F2-A06：...`。

## 结论
**未过（打回）。** 修 I-1（注释）+ I-2（transient 测试非恒真）+ I-3（file_name 显式报错）+ I-4（去装饰注释）后复审。核心安全约束已验证通过。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-1 已解决**：transient 分类测试注释改为 DbError::TransientKeychain，与实现一致。
- **I-2 已解决**：transient_leaves_db_file_untouched 改为写真实文件→`apply_recovery_action(RetryNoTouch)`→断言字节未变（非恒真，误删会失败）；db.rs apply_recovery_action（RetryNoTouch=no-op/BackupAndConfirmRebuild=backup）。
- **I-3 已解决**：backup_corrupt_file file_name 改 `.ok_or_else(io::Error InvalidInput)?` 显式报错。
- **I-4 已解决**：`// =====` 装饰注释清除（`={5,}` 零匹配）。
核心安全（瞬时不碰库 RetryNoTouch no-op / 永久备份 rename 不删）双侧验证未破坏；无新增高危。
