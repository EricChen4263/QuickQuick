---
id: V1-F1-S03-code
type: coding_record
level: 小功能
parent: V1-F1
children: []
created: 2026-05-30T22:50:23Z
status: 通过
commit: WIP
acceptance_ids: [V1-F1-A06, V1-F1-A07, V1-F1-A08]
evidence:
  - src-tauri/src/privacy.rs
  - src-tauri/src/clipboard.rs
  - src-tauri/tests/privacy.rs
  - src-tauri/tests/clipboard.rs
author: coder
---

# 编码记录 · 隐私门控（V1-F1-S03）

## 做了什么

新增 `src-tauri/src/privacy.rs`，实现隐私门控模块：包含 `ExcludeList`（App 排除名单）、
`CapturePolicy`（暂停开关 + 名单引用）和 `should_skip` 纯函数（判定快照是否跳过）。
在 `clipboard.rs` 新增 `poll_once_with_policy` 函数，在 `poll_once` 的变化检测基础上
叠加隐私门控，并为 `ClipboardSnapshot` 补充 `is_concealed` 与 `source_app` 两个字段。
三个验收项（A06/A07/A08）全部通过自动化测试，24 个测试用例全绿。

## should_skip 判定序

判定优先级由高到低，命中即返回，后续规则不再执行：

1. `policy.paused == true` → `Some(SkipReason::Paused)`
2. `snapshot.is_concealed == true` → `Some(SkipReason::Concealed)`
3. `snapshot.source_app` 在 `policy.exclude` 名单内 → `Some(SkipReason::Excluded)`
4. `snapshot.has_self_marker == true` → `Some(SkipReason::SelfMark)`
5. 否则 → `None`（可捕获）

**重要约束**：本函数不分析剪贴板内容，不做任何启发式识别（不猜密码、不检测 token）。
敏感判定仅依赖平台标记（`is_concealed`）和用户显式配置（排除名单）。
A06 中 `concealed_no_heuristic` 用例专门验证此约束：内容含密码特征字符串但 `is_concealed=false` 时，`should_skip` 返回 `None`，不跳过。

## 关键决策与理由

- **不做内容启发式（A06 含 `concealed_no_heuristic` 用例）**：设计文档§三#6 明确"敏感判定仅依赖平台标记"。内容启发式（正则匹配密码/token）误报率高且难以维护，用户不可预期；反之平台标记由密码管理器等应用主动写入，语义明确。专用反例测试 `concealed_no_heuristic` 将此约束固化为可执行规范。

- **`ExcludeList` 用 `HashSet<String>`**：排除名单的核心操作是 `contains`，`HashSet` 提供 O(1) 平均查找，优于 `Vec` 的 O(n)。名单规模小（通常 < 10 条）时差异不大，但接口语义上 HashSet 更能表达"集合成员检测"。

- **`CapturePolicy` 借用 `ExcludeList` 而非持有**：轮询循环持有 policy 的短暂引用，ExcludeList 生命周期更长（由运行时维护）。借用避免每次轮询克隆名单，也让所有权关系更清晰。`CapturePolicy<'a>` 的生命周期参数在编译期保证 ExcludeList 的有效性。

- **`ClipboardSnapshot` 新增字段与 S01/S02 兼容**：`is_concealed` 默认 `false`、`source_app` 默认 `None`，S01/S02 的 `FakeBackend::new` 构造器补填这两个字段的零值，既有测试行为完全不变，无破坏性改动。

- **`poll_once_with_policy` 独立函数而非修改 `poll_once`**：保持 `poll_once` 接口稳定，S01/S02 的调用方无需迁移。隐私门控是可选叠加层，分离函数使职责边界清晰。

## 改动文件

- `src-tauri/src/privacy.rs` — 新建：`SkipReason`、`ExcludeList`、`CapturePolicy`、`should_skip` 纯函数，含模块文档注释（判定序与不做启发式约束）
- `src-tauri/src/clipboard.rs` — 扩展：`ClipboardSnapshot` 新增 `is_concealed`/`source_app` 字段；新增 `poll_once_with_policy` 函数；模块文档注释补充 S03 相关说明
- `src-tauri/src/lib.rs` — 新增 `pub mod privacy` 导出
- `src-tauri/tests/privacy.rs` — 新建：A06/A07 集成测试（5 个用例：concealed_skipped、concealed_no_heuristic、app_exclude_list、app_not_in_exclude_list、app_exclude_none_source）
- `src-tauri/tests/clipboard.rs` — 扩展：A08 集成测试（2 个用例：pause_stops_capture、pause_false_captures_normally）；导入 `poll_once_with_policy`、`CapturePolicy`、`ExcludeList`；`FakeBackend::new` 补填 `is_concealed=false`/`source_app=None`

## 自测结论（TDD 红-绿-重构）

**RED 阶段**：
- 先写 `concealed_skipped` 测试，引用尚不存在的 `privacy` 模块 → 编译失败（模块未定义）
- 先写 `app_exclude_list` 测试 → 编译失败
- 先写 `pause_stops_capture` 测试，引用尚不存在的 `poll_once_with_policy` → 编译失败

**GREEN 阶段**：
- 实现 `SkipReason`、`ExcludeList::contains/new_with_apps`、`CapturePolicy`、`should_skip` → A06/A07 通过
- 在 `ClipboardSnapshot` 补充两字段，实现 `poll_once_with_policy` → A08 通过

**REFACTOR 阶段**：
- `should_skip` 中 `source_app` 判定用 `if let` + early return，保持嵌套 ≤ 2 层
- 模块文档注释补全判定序与约束说明，使阅读代码无需查阅设计文档

**code-standards 逐项自检**：
- 格式：`cargo fmt` 通过，无格式问题
- 命名：函数用动词+名词（`should_skip`、`new_with_apps`），布尔用 `is_`/`has_` 前缀（`is_concealed`、`has_self_marker`），无 `tmp`/`flag` 等禁用名
- 函数长度：`should_skip` 20 行、`poll_once_with_policy` 26 行，均 ≤ 50 行
- 嵌套深度：最深 2 层（`if let Some(app) = ... { if ... }`），≤ 3 层
- 注释：写"为什么"（为何不做启发式、为何借用而非持有），无装饰性分隔注释
- 类型：使用 `Option<SkipReason>` 而非 bool，携带语义；`HashSet` 匹配集合查找语义
- 性能：`should_skip` 纯函数无堆分配，仅做引用比较；`ExcludeList::contains` O(1)
- 测试：TDD 红-绿-重构，7 个测试用例覆盖正反例，全绿
- 安全：不分析剪贴板内容，日志不打印内容，符合隐私约束
- clippy：`cargo clippy --all-targets -- -D warnings` 零警告

## 审查清理（打回第 1 次，2026-05-31）

按 code-reviewer 审查意见清理 `tests/clipboard.rs`：

- **I-1**：删除全部 7 处装饰性分隔注释（`// ── ... ──` 形式，约第 15/50/74/102/121/155/207 行）；以空行分段，测试函数 doc 注释已描述意图，无需额外装饰标题。`grep -nE '──|═══|━━━'` 确认清零（exit=1）。
- **I-2**：修正节标题与函数错位——原文件 A08 的 `pause_stops_capture` / `pause_false_captures_normally` 两个函数错位于 A05 `bump_no_new_record` 之前，现移至 A05 之后，顺序调整为 A01→A02→A03→reset→A04→A05→A08，与验收项排列一致。不改测试逻辑与断言。

回归结论：clipboard 8 passed、全量全绿、clippy=0、deco=1（清零）、todo=1（无残留）。
