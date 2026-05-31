---
id: V2-F1-S04-code
type: coding_record
level: 小功能
parent: V2-F1
children: []
created: 2026-05-31T00:27:20Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A05]
evidence:
  - src-tauri/src/translate/credential.rs
  - src-tauri/src/translate/mod.rs
  - src-tauri/tests/translate.rs
author: coder
---

# 编码记录 · V2-F1-S04 凭据配置

## 做了什么

实现了翻译 provider 的结构化凭据字段 schema 声明，以及 secret→CredStore（keychain）/非密→加密 DB 的存取路由。框架通过 `credential_schema(provider_id)` 动态获取字段描述符并驱动 save/load，路由正确性由测试负向断言严格验证。

## 关键决策与理由

- **引入 `CredStore` trait 而非直接调用 keyring**：keyring 3 的 mock 使用 `EntryOnly` 持久化语义——数据只在同一 Entry 实例内存活，跨 `Entry::new` 调用即消失。若在 `save_credentials` 内创建一个 Entry 写入，断言时再 `Entry::new` 就必然 NoEntry。通过 `CredStore` trait + `MockCredStore`（内存 HashMap）完全绕开此限制，测试 headless 不弹窗，路由逻辑与真实 keychain 实现解耦。

- **`provider_config` 表 CREATE TABLE IF NOT EXISTS**：新表幂等建表，不影响既有 schema，`db.rs` 的 schema 回归测试（7 个）保持全绿。

- **secret 值不出现在任何错误消息字符串中**：`CredError::Keychain` 只携带 keyring 的错误描述（不含 secret 内容），`write_to_db` 也不接触 secret 路径。

- **`save_credentials` 对未知字段 key 按 `is_secret=false` 路由（走 DB）**：schema 之外的字段不强制报错，而是保守地存 DB，避免因 schema 更新滞后导致调用方数据丢失。

## 改动文件

- `src-tauri/src/translate/credential.rs` — 新建：`CredentialField` 结构体、`CredStore` trait（+ `KeyringCredStore` 生产实现）、`credential_schema` 函数、`save_credentials` / `load_credentials`、`provider_config` 表 DDL、单元测试（含 `MockCredStore`）
- `src-tauri/src/translate/mod.rs` — 追加 `pub mod credential;` 暴露子模块
- `src-tauri/tests/translate.rs` — 追加 A05 集成测试组（6 个测试）：schema 断言 × 4、路由正确性负向断言 × 1、load 读回 × 1；测试内嵌 `MockCredStore` 实现

## 审查修复记录（打回第 1 次，2026-05-31）

**I-1 未知字段报错不降级**：`CredError` 新增 `UnknownField { provider, field }` 变体；`save_credentials` 在 `schema.is_empty()` 时返回 `UnknownProvider`，`schema.iter().find()` 返回 None 时返回 `UnknownField`，彻底移除 `unwrap_or(false)` 静默降级。字段值不携带于任何错误消息（安全约定不变）。

**I-2 provider_config 并入 ensure_schema**：`db.rs::ensure_schema` 新增 `provider_config` 表的 `CREATE TABLE IF NOT EXISTS`，与 `clip_items`/`clip_images` 同批预埋。`credential.rs` 移除 `ensure_provider_config_table` 函数及全部调用点；内部单元测试（绕开 `open_or_create` 用裸内存 DB）保留局部手动建表以维持自包含。`tests/schema.rs` 新增断言 `schema_preembed_provider_config_table_exists_with_required_columns`，验证三列（provider_id / field_key / value）均存在。

**I-3 负向用例**：`tests/translate.rs` A05 区新增两个测试：
- `credential_schema_keychain_unknown_provider_returns_err`：`save_credentials("nonexistent_provider", ...)` 返回 `UnknownProvider` Err。
- `credential_schema_keychain_unknown_field_returns_err_and_does_not_write_db`：百度传 `"secret_keys"`（拼写错误）返回 `UnknownField` Err，并断言 DB 中该 field_key 行数为 0（证明不静默降级）。

**回归结果**：
- credential（含负向）：exit=0，8 passed
- schema（含 provider_config 断言）：exit=0，8 passed
- 全量：exit=0，84 passed（6+3+2+4+5+8+55+1）
- clippy：exit=0，零警告
- 无装饰注释（deco=1）、无 TODO（todo=1）

## 自测结论（TDD 红-绿-重构）

**RED**：先在 `tests/translate.rs` 追加 6 个 credential 测试，引用尚不存在的 `quickquick_lib::translate::credential` 模块，`cargo test` 编译失败（`E0432: unresolved import`）——确认为功能未实现导致的失败，非语法/环境错。

**GREEN**：
1. 创建 `credential.rs`，第一版直接调用 `keyring::Entry::new`。4 个 schema 测试立即通过，2 个运行时测试因 keyring mock `EntryOnly` 语义跨实例不持久而失败。
2. 重构为 `CredStore` trait，集成测试改用 `MockCredStore`，6 个测试全绿。

**REFACTOR**：确认无重复逻辑，`keychain_account` 抽为独立函数，DB 读写各一个私有函数，均 ≤ 15 行。

**code-standards 自检**：
- 格式：4 空格缩进，行宽 ≤ 120，文件末尾换行
- 函数：单一职责，最长函数（`save_credentials`）20 行，嵌套 ≤ 2 层
- 命名：描述性，函数动词+名词（`save_credentials` / `load_credentials` / `write_to_db`），布尔量 `is_secret` / `is_required`
- 注释：写「为什么」（EntryOnly 语义说明、路由规则），无死代码，公共 API 均有文档注释
- 类型：`CredError` 用 `thiserror`，无裸 `unwrap`/`panic`（仅 `MockCredStore` 测试辅助中有 `unwrap`，在 `#[cfg(test)]` 范围内）
- 安全：secret 不入 DB，secret 值不出现在错误消息，DB 操作全部参数化查询，无密钥入库
- 测试：AAA 结构，行为化命名，MockCredStore headless 不弹窗，负向断言验证路由正确性
- 提交：待 commit（WIP）
