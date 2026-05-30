---
id: V1-F3-S06-code
type: coding_record
level: 小功能
parent: V1-F3
children: []
created: 2026-05-30T23:29:16Z
status: 通过
commit: WIP
acceptance_ids: [V1-F3-A15, V1-F3-A16, V1-F3-A17, V1-F3-A18]
evidence:
  - src-tauri/src/paste.rs
  - src-tauri/tests/paste.rs
  - src/panels/history/paste-mode.ts
  - src/panels/history/paste-mode.test.ts
author: coder
---

# 编码记录 · 回写粘贴（V1-F3-S06）

## 做了什么

实现了"用户在历史面板选中条目后按 Enter，将该条目写回剪贴板并模拟 Cmd+V 粘贴到原应用"的完整回写粘贴引擎。具体覆盖四个验收项：

- **A15 回写时序**：`write_then_paste` 写入剪贴板后轮询 `change_count`，确认 OS 已接受写入才发模拟粘贴，超时则返回 `PasteError::Timeout` 不盲发。
- **A16 前端粘贴模式**：`resolvePasteMode(hasModifier)` 纯函数——无修饰键返回 `"paste"`（写回+粘贴），有修饰键返回 `"copy_only"`（仅写回）。
- **A17 焦点恢复顺序契约**：`focus_restore_sequence()` 冻结五步顺序（RecordFrontmost → HidePanel → ActivateOriginalApp → WaitForeground → SimulatePaste），作为运行期实现的纯数据契约。
- **A18 粘贴归属**：粘贴完成后剪贴板留下被选条目 X，不做"借用后恢复原剪贴板"操作。

## 关键决策与理由

- **`PasteBackend` trait 抽象**：将"写剪贴板 / 读 changeCount / 发模拟粘贴"封装为 trait，生产实现（macOS NSPasteboard + CGEvent）与测试用 `FakePasteBackend` 完全解耦。否则集成测试必须依赖真实 OS 环境，无法在 CI 中 headless 运行。

- **轮询 `change_count` 再粘贴（A15）**：macOS `NSPasteboard.changeCount` 是写入成功的唯一可观察信号。写入后立即发 Cmd+V 存在竞态——若 OS 尚未完成写入，粘贴到的是旧内容。轮询至多 `MAX_POLL_ATTEMPTS`（200 次）既保证正确性，又有超时保护避免死等。否决方案：固定 sleep——时长难以定义且不可移植。

- **`FocusStep` 枚举冻结顺序（A17）**：运行期 OS 激活操作不在本 Phase 范围内实现，但顺序契约必须在 Phase 内确定并可测试。选择纯数据枚举 + `focus_restore_sequence()` 纯函数，使契约独立于 OS API，测试对每一步做精确断言，未来运行期实现只需 match 此序列执行。

- **粘贴后不恢复原剪贴板（A18）**：设计文档§三#4 明确剪贴板归属为被选条目 X。若做"借用后恢复"，用户在粘贴完成后无法再用 Cmd+V 重复粘贴同一内容，体验一致性更差。因此 `write_with_marker` 写入 X 后，剪贴板内容即为 X，不额外保存/回写旧内容。

- **`wait_for_count_increase` 独立为私有函数**：将轮询逻辑从 `write_then_paste` 中分离，使两者各 ≤ 15 行，嵌套不超过 2 层，符合函数 ≤ 50 行、嵌套 ≤ 3 层的规范。

## 改动文件

- `src-tauri/src/paste.rs` — 新增 `PasteBackend` trait、`FocusStep` 枚举、`focus_restore_sequence()`、`write_then_paste()`、`wait_for_count_increase()`、`PasteError` 类型，构成回写引擎核心逻辑层
- `src-tauri/tests/paste.rs` — 新增 4 个集成测试（A15 正常路径 + A15 超时路径 + A17 顺序契约 + A18 归属验证），全部使用 `FakePasteBackend` headless 运行
- `src/panels/history/paste-mode.ts` — 新增 `PasteMode` 类型与 `resolvePasteMode()` 纯函数（A16）
- `src/panels/history/paste-mode.test.ts` — 新增 2 个 Vitest 单测（无修饰键 → paste / 有修饰键 → copy_only）

## 自测结论（TDD 红-绿-重构）

**TDD 循环**

1. RED：先写 `paste_timing_paste_waits_changecount` — 因 `write_then_paste` 不存在，编译失败（红）。
2. GREEN：实现最小 `write_then_paste`（记录前值 → 写入 → 单次 check → 粘贴），测试变绿。
3. RED：加入超时测试 `paste_timing_timeout_when_count_never_increases` — 无超时逻辑，测试挂死（红）。
4. GREEN：引入 `MAX_POLL_ATTEMPTS` 轮询上限 + `PasteError::Timeout`，超时测试变绿。
5. REFACTOR：将轮询提取为 `wait_for_count_increase`，降低嵌套，两函数均 ≤ 15 行。
6. 前端：先写 `paste-mode.test.ts` 两个失败用例，再实现一行 `resolvePasteMode`，全绿。

**code-standards 逐项自检**

| 规范项 | 状态 | 说明 |
|---|---|---|
| 格式 | 通过 | `cargo fmt` / Prettier 格式化，2 空格缩进（TS） |
| 命名 | 通过 | Rust snake_case / TS camelCase，函数均为动词+名词，无 `tmp`/`flag` |
| 函数长度 ≤ 50 行 | 通过 | 最长函数 `write_then_paste` 12 行；`wait_for_count_increase` 8 行 |
| 嵌套 ≤ 3 层 | 通过 | 最深 2 层（for + if） |
| 注释写"为什么" | 通过 | 模块头注释、函数 doc 均说明设计意图，无装饰性分隔符 |
| 类型安全 | 通过 | Rust 全类型推断；TS 无 `any`，`PasteMode` 用联合类型 |
| 单一职责 | 通过 | `paste.rs` 仅含回写引擎；`paste-mode.ts` 仅含模式解析 |
| 安全红线 | 通过 | 无密钥、无用户输入未校验路径、无敏感日志 |
| 测试覆盖 | 通过 | Rust 4 测（正常路径+超时+顺序+归属）；TS 2 测（两分支全覆盖） |
| clippy | 通过 | 0 warning |
| 无 TODO/FIXME | 通过 | 无遗留 |

**测试结果**

```
# Rust 集成测试
running 4 tests
test paste_timing_paste_waits_changecount   ... ok
test paste_timing_timeout_when_count_never_increases ... ok
test focus_restore_path_sequence_matches_spec ... ok
test paste_leaves_selected_item_on_clipboard  ... ok
test result: ok. 4 passed; 0 failed

# Vitest 单测
✓ src/panels/history/paste-mode.test.ts (2)
  ✓ resolvePasteMode > 无修饰键（纯 Enter）时返回 paste 模式（写回 + 粘贴）
  ✓ resolvePasteMode > 有修饰键时返回 copy_only 模式（仅写回不粘贴）
Test Files  1 passed (1)
Tests       2 passed (2)
```

全量 6 测通过，clippy 0 warning，符合 code-standards 各项约束。
