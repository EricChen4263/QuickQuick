---
id: V1-F1-S01-review
type: review
level: 小功能
parent: V1-F1
children: []
created: 2026-05-31T05:10:00Z
status: 通过
commit: WIP
acceptance_ids: [V1-F1-A01, V1-F1-A02, V1-F1-A03]
evidence: []
author: code-reviewer
---

# 审查记录 · 剪贴板捕获核心（V1-F1-S01）

## 审查范围
- `src-tauri/src/clipboard.rs`（新增核心实现）
- `src-tauri/src/lib.rs`（`pub mod clipboard;`）
- `src-tauri/tests/clipboard.rs`（新增集成测试 A01/A02/A03）
参照：设计文档§三#2/#3/#5、acceptance V1-F1-A01~A03、code-standards。

## 问题清单

### Critical
无。

### Important
**[I-01] `poll_once` 计数比较用 `==` 而非 `>`，回绕/OS重置/降序场景偏离"一递增即捕获"（置信度 82）**
- 位置：`src-tauri/src/clipboard.rs`（`if current == *last_seen_count { return None; }`）
- 问题：当 `current < last_seen_count`（OS 进程重启致 Windows `GetClipboardSequenceNumber` 重置、或测试构造降序）会穿透判断、把 `last_seen_count` 下调为更小值 → 下次计数恢复原值时重复捕获；当前测试未覆盖该边界。
- 修复（推荐方案 A）：
  ```rust
  if current <= *last_seen_count {
      if current < *last_seen_count { *last_seen_count = current; } // OS 计数重置，更新基线
      return None;
  }
  ```
  并补一条降序/重置的防御性测试。

### 附（tester 提示，随手清理）
`src-tauri/tests/clipboard.rs` 有 `unused_import: CapturedItem` 的 rustc warning，按 code-standards "无死代码" 移除多余 import。

## 逐维度核查（通过项）
双字段同存（A01）✓；一递增只捕一次（A02 主路径）✓；防自污染 has_self_marker（A03）✓；ClipboardBackend 抽象清晰、OS 细节隔离 ✓；无裸 unwrap/panic（`?` 处理 Option）✓；POLL_INTERVAL_MS 具名常量非魔数 ✓；测试 AAA、无恒真、FakeBackend 可编程 ✓；text 为 None 跳过 ✓；无越界（无去重/隐私名单/UI/回写）✓；无 TODO/FIXME ✓。change_count 回绕边界 = I-01。

## 总结论
**未过（打回）。** 修复 I-01（计数比较改 `>`/`<=` + 重置处理 + 防御测试）+ 清理 unused import 后复查。主路径实现正确，预期一次复查通过。

---

## 复审结论（2026-05-31）

**status = 通过**

I-01 与 unused import 清理均已落实：`poll_once` 改为 `current <= *last_seen_count`，降序时下调基线、严格递增才捕获；新增 `poll_count_reset_defense` 测试覆盖 降序→None+基线更新 / 随后递增→正常捕获一次 / 同值再调→None 三段（非恒真）。tests import 仅保留实际使用三项（`CapturedItem` 已移除），`cargo clippy --all-targets -- -D warnings` 零警告。A01/A02/A03 主路径未受影响，无新增高危。
