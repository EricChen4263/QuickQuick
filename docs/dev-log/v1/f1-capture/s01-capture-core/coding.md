---
id: V1-F1-S01-code
type: coding_record
level: 小功能
parent: V1-F1
children: []
created: 2026-05-30T22:23:22Z
status: 通过
commit: WIP
acceptance_ids: [V1-F1-A01, V1-F1-A02, V1-F1-A03]
evidence:
  - src-tauri/src/clipboard.rs
  - src-tauri/tests/clipboard.rs
  - src-tauri/src/lib.rs
author: coder
---

# 编码记录 · 剪贴板捕获核心（V1-F1-S01）

## 做了什么

新增 `src-tauri/src/clipboard.rs`，实现剪贴板捕获引擎的纯逻辑核心：
抽象 OS 剪贴板为 `ClipboardBackend` trait，通过 `poll_once` 函数完成变化检测、
防自污染过滤和双字段（纯文本 + HTML）捕获，三个验收项全部通过自动化测试。

## 关键决策与理由

- **trait 抽象而非直接调用 OS API**：`ClipboardBackend` trait 使捕获引擎逻辑层
  完全脱离 OS 依赖，测试文件中用 `FakeBackend` 构造计数序列驱动，无需 mock 框架、
  无 OS 调用，测试速度极快（0.00s）。否决了直接在 `poll_once` 内调用
  `NSPasteboard` 的方案，那样会使逻辑不可单测。

- **`last_seen_count` 在跳过时也推进**：A03 防自污染的关键——本工具自写剪贴板后
  changeCount 递增，若跳过时不推进 `last_seen_count`，下次轮询（count 未再变）
  仍会命中 `==` 分支返回 None，行为正确但语义含混；若 count 再次递增（用户紧接复制），
  将重读同一 count，反而可能漏捕。先推进 count 再 return None 是最清晰的语义。

- **`text` 为必填键，`html` 为 Option**：对齐设计文档§三#5「显示/搜索/判重走
  纯文本键」；无纯文本的快照（如纯图片）当前静默跳过，留给后续 s 实现处理。

- **`poll_once` 接收 `&dyn ClipboardBackend`**：使用动态分发而非泛型，避免
  在公共 API 中暴露类型参数，调用侧更简洁；性能不敏感（500ms 轮询一次）。

- **不实现 write**：S01 范围严格限于捕获核心，回写相关（私有 UTI 写入 OS）留给 s06。
  `has_self_marker` 由 OS 后端实现，逻辑层只消费布尔值，保持单一职责。

## 改动文件

- `src-tauri/src/clipboard.rs` — 新建；含 `ClipboardBackend` trait、
  `ClipboardSnapshot`、`CapturedItem`、`poll_once`、`POLL_INTERVAL_MS` 常量
- `src-tauri/tests/clipboard.rs` — 新建集成测试；含 `FakeBackend` 及 A01/A02/A03 三用例
- `src-tauri/src/lib.rs` — 追加 `pub mod clipboard;` 一行

## 自测结论（TDD 红-绿-重构）

**RED**：先写 `tests/clipboard.rs`（三个测试），确认因 `clipboard` 模块不存在
而编译失败（`E0432 unresolved import`），验证测试本身可感知实现缺失。

**GREEN**：实现 `src/clipboard.rs` + 注册 `pub mod clipboard`，
`cargo test --test clipboard` 输出：

```
test capture_dual_field ... ok
test poll_changecount_triggers_capture ... ok
test self_write_marker_skipped ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**REFACTOR**：实现本身已是最小实现，无重复逻辑，无需进一步重构。

**验证指标**：

| 检查项 | 结果 |
|--------|------|
| `cargo test --test clipboard` | 3 passed, 0 failed |
| `cargo clippy -- -D warnings` | clippy=0，无 warning |
| `cargo build` | build=0 |
| `grep -rn TODO\|FIXME src-tauri/src/` | 无（todo_exit=1） |

**code-standards 逐项自检**：

- 格式：4 空格缩进（Rust 惯例），行宽 ≤ 100，文件末尾换行，符合 §2
- 函数：`poll_once` 共 20 行，嵌套最深 2 层，单一职责，符合 §3
- 命名：`ClipboardBackend`/`ClipboardSnapshot`/`CapturedItem` PascalCase；
  `poll_once`/`change_count`/`has_self_marker` snake_case；布尔量用 `has_` 前缀，符合 §4
- 注释：每个公共类型/函数都有 `///` 文档注释，注释写「为什么」（私有 UTI 标记机制说明），无死代码，符合 §5
- 类型：无魔术数字（`POLL_INTERVAL_MS` 具名常量），公共接口全部显式类型，无 `unwrap`/`panic`（thiserror 用于 db 层，本模块无错误路径），符合 §6
- 性能：`poll_once` 仅在 count 变化时调用 `read()`，无多余 OS 调用，符合 §7
- 测试：AAA 结构、行为化命名、`FakeBackend` 可编程、无恒真断言，符合 §8
- 安全：无密钥、无 SQL、无用户输入、无日志泄露，符合 §10

---

## 审查修复（打回第 1 次 → 回归）

按 code-reviewer 审查意见完成以下两项修复：

**I-01 修复**：`poll_once` 计数比较由 `== *last_seen_count` 改为严格递增判定（`current <= *last_seen_count`）。降序分支（OS 计数重置场景，如 Windows `GetClipboardSequenceNumber` 进程重启归零）额外将基线下调为 `current`，避免计数恢复后重复捕获。

新增防御测试 `poll_count_reset_defense`（AAA 结构，非恒真）：
- 构造 `last_seen=6`，OS 重置后 `count=5`（降序）→ 返回 None 且 `last_seen` 下调为 5；
- 随后 `count=6`（递增）→ 正常捕获一次；
- 再次调用同 count → None（不重复捕获）。

**清理**：移除 `src-tauri/tests/clipboard.rs` 中未使用的 `CapturedItem` import，消除 rustc `unused_import` warning。

**回归结论**：

| 检查项 | 结果 |
|--------|------|
| `cargo test --test clipboard` | 4 passed, 0 failed（含新增 `poll_count_reset_defense`）|
| `cargo clippy --all-targets -- -D warnings` | clippy=0，零 warning |
| `cargo build` | build=0 |
| `grep -rn TODO\|FIXME src-tauri/src/` | 无（todo_exit=1）|

全修复 + 回归全绿（含 `--all-targets` clippy）。
