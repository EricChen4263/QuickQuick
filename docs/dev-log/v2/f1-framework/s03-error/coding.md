---
id: V2-F1-S03-code
type: coding_record
level: 小功能
parent: V2-F1
children: []
created: 2026-05-31T00:11:13Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A03, V2-F1-A04, V2-F1-A07]
evidence:
  - src-tauri/src/translate/error.rs
  - src-tauri/src/translate/retry.rs
  - src-tauri/src/translate/cancel.rs
  - src-tauri/src/translate/mod.rs
  - src-tauri/tests/translate.rs
author: coder
---

# 编码记录 · V2-F1-S03 错误枚举 + 错误降级 + 超时/取消框架

## 做了什么

扩展了 `TranslateError` 统一错误枚举，新增三个子模块：`error`（归一映射）、`retry`（同源退避策略）、`cancel`（在途请求追踪），实现了 A03/A04/A07 三项验收标准，全程 headless 可测，无真实网络请求。

## 关键决策与理由

- **保留 `ParseError` 变体名**：s01/s02 既有 provider 实现（`providers.rs`）均使用 `ParseError`，重命名会破坏现有 45 个测试。新变体统一用语义化短名（`Network`/`Auth`/`RateLimit` 等），与旧 `ParseError` 并存，清晰区分解析错误与业务错误。

- **`map_provider_error` 优先级：provider_code > HTTP 状态码**：provider_code 是精确业务语义，比 HTTP 状态码更可靠（例如 200 状态码但返回 quota 超限的 provider）。

- **`classify_timeout` → `Network` 而非独立 `Timeout` 变体**：设计文档 §4.1 明确"超时驱动 network 错误枚举"，且超时与连接失败在重试策略上一致（均瞬时可重试），合并为 `Network` 减少变体数量，符合 KISS 原则。

- **`retry_with_backoff` 不真正 sleep**：框架只计算退避毫秒数（`next_backoff_ms`）但不调用 `std::thread::sleep`，保持 headless 可测。生产调用方按需在 op 闭包或外层加真实 sleep。

- **`InflightTracker` 用 `AtomicU64`**：线程安全且无锁，`begin()` 原子自增返回新 generation，`is_current()` 原子读取比较。初始值 0 表示"未发起任何请求"，第一次 `begin()` 返回 1，语义清晰。

- **绝不自动 failover 跨源**：`retry.rs` 中无任何 provider 切换逻辑；provider_id 完全由调用方持有，框架不感知，从结构上保证了"同源重试、显式跨源"的约束。

## 改动文件

- `src-tauri/src/translate/mod.rs` — 扩展 `TranslateError` 为 8 个具名变体（保留 `ParseError`），新增 `pub mod error/retry/cancel`
- `src-tauri/src/translate/error.rs` — 新建：`map_provider_error`（按 provider_code/HTTP 状态码归一）、`classify_timeout`（→ Network）
- `src-tauri/src/translate/retry.rs` — 新建：`is_transient`（瞬时/永久分类）、`retry_with_backoff`（同源退避执行器）、`next_backoff_ms`（指数退避计算）
- `src-tauri/src/translate/cancel.rs` — 新建：`InflightTracker`（AtomicU64 generation 计数器，begin/is_current）
- `src-tauri/tests/translate.rs` — 追加 A03/A04/A07 三组共 18 个集成测试（保留 s01/s02 既有 27 个测试）

## 审查修复（打回第 1 次，2026-05-31）

按 code-reviewer 打回意见修复三项：

**P1 退避 sleep_fn 真实生效**：`retry_with_backoff` 新增 `sleep_fn: impl Fn(u64)` 参数，在每次瞬时失败后调用 `sleep_fn(next_backoff_ms(attempt))`，退避不再装饰化。生产传 `|ms| std::thread::sleep(Duration::from_millis(ms))`，测试传记录调用序列的闭包。

**P2 provider_id 不变真实断言**：`retry_policy_same_source_retry_no_cross_failover_succeeds_on_third_attempt` 测试中，op 闭包用 `RefCell<Vec<&str>>` 记录每次调用使用的 `provider_id`，断言全程 3 次均为同一值（`"mymemory"`）；同时断言 sleep_fn 被调用 2 次，退避值为 500ms/1000ms，验证退避真实生效。删除原占位 `let _ = provider_id`。

**P3 错误码精确匹配**：`map_by_provider_code` 改用 `const` 数组精确集合匹配（`QUOTA_CODES`/`TOO_LONG_CODES`/`UNSUPPORTED_CODES`），避免 `quota_remaining`/`unsupported_format` 等含相同子串但语义不同的 code 误命中。新增两个边界测试：`quota_remaining` 不误判为 Quota、`unsupported_format` 不误判为 Unsupported。

回归结果：47 tests passed / 0 failed，clippy 零警告，无装饰注释，无 TODO/FIXME。

## 自测结论（TDD 红-绿-重构）

**RED**：先追加 18 个测试（import 三个不存在的模块 + 使用不存在的枚举变体），`cargo test` 报 `unresolved import` + `no variant found`，确认失败原因是功能未实现而非语法错误。

**GREEN**：依序实现 `TranslateError` 扩展 → `error.rs` → `retry.rs` → `cancel.rs` → `mod.rs` 暴露，`cargo test` 45 passed / 0 failed。

**REFACTOR**：代码审查无需改动——所有函数 ≤ 50 行，嵌套 ≤ 3 层，命名符合 snake_case，无裸 unwrap/panic。

**code-standards 逐项**：
- 格式：4 空格缩进，行宽 ≤ 120，文件末单换行
- 函数：单一职责，最长函数 `retry_with_backoff` 约 20 行
- 命名：`is_transient`（布尔 is 前缀）、`map_provider_error`（动词+名词）、`classify_timeout`（动词+名词）
- 注释：解释"为什么"（如超时归 Network 的理由、不 sleep 的原因），无装饰分隔符
- 类型：无魔术数字（退避上限 `8_000` 有注释说明），公共 API 显式类型
- 测试：AAA 结构，描述行为命名，非恒真断言，headless
- 安全：无密钥入库，无用户输入直接拼 SQL
- 提交：待 commit（WIP）
