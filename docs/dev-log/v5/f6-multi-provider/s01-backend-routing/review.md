---
id: V5-F6-S01-review
type: review
level: 小功能
parent: V5-F6
children: []
created: 2026-06-03T06:00:00Z
status: 通过
commit: e838919
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 多翻译源批次A 后端动态路由（V5-F6-S01）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src-tauri/src/translate/providers.rs` | diff | 新增 `build_provider`（4 provider 动态构造，缺 key→Err）及 8 条单元测试 |
| `src-tauri/src/ipc/translate.rs` | diff | `translate_text_impl` 扩签名（+settings_path +cred_store），动态路由替换硬编码 MyMemory；命令层构造 KeyringCredStore + settings_path；5 条新单元测试 |
| `src-tauri/src/translate/credential.rs` | diff | `MockCredStore` 提升为模块级 `#[cfg(test)] pub`（仅可见性/位置变化） |
| `src-tauri/src/ipc/settings.rs` | diff | `resolve_config_path` 改为 `pub(crate)` |
| `src-tauri/tests/ipc_translate.rs` | diff | 集成测试适配新签名，新增本地 `LocalMockCredStore` |

参照：Rust 规范（函数≤50行/嵌套≤3层/Result不panic/注释写为什么/禁装饰注释/禁TODO_FIXME/错误处理完整/日志不打印敏感信息）、code-standards、项目规范。

---

## 重点审查结论

### 安全红线：secret 不泄漏（明确判定：通过）

逐路径核查：

1. **`build_provider` 错误串**：`"baidu 未配置 AppID，请前往设置填入 API Key"` / `"baidu 未配置 SecretKey，请前往设置填入 API Key"` / `"deepl_free 未配置 auth_key..."` / `"google 未配置 api_key..."`——全部只含字段名（`AppID`/`SecretKey`/`auth_key`/`api_key`），不含字段值。**通过。**

2. **`load_credentials` 错误路径**：`CredError::Keychain(e.to_string())` 的 `e` 来自 keyring crate 自身的错误消息（如 `NoEntry`、`PlatformFailure`），keyring 库不会在错误消息中回显 secret 值，属于 OS 级错误描述。`CredError::UnknownProvider`/`UnknownField` 只携带 provider_id 和 field_key，不携带值。**通过。**

3. **`translate_text_impl` 错误传播**：`load_credentials` 失败 → `map_err(|e| e.to_string())`，CredError Display 实现（`thiserror` derive）不含值——如 `#[error("keychain 操作失败：{0}")]` 的 `{0}` 是 keyring 错误描述字串，非用户 secret 值。**通过。**

4. **HTTP 请求构造后**：`ProviderHttpRequest` 的 `url`/`headers`/`body` 字段内的 secret 值（如 DeepL Authorization 头）只在内存中流转，未见任何日志打印路径（无 `println!`/`eprintln!`/`log::`/`tracing::` 打印 http_req 内容）。**通过。**

### 动态路由正确性（明确判定：通过）

1. **selected_provider 真被读取**：`get_selected_provider_impl(settings_path)?` 在执行器调用前发生，provider_id 驱动 `build_provider`；历史写入用 `&provider_id`（第 213 行）而非硬编码字符串。**通过。**

2. **缺 key 时 Err 不回退**：`find("app_id").ok_or_else(...)` 直接返回 `Err` 并通过 `?` 传播，函数在 `build_request`/`exec.execute` 之前退出；无 fallback 到 mymemory 的分支；FakeExecutor 的 `call_count` 断言也在集成测试中验证（`translate_text_impl_selected_baidu_without_creds_returns_err_with_hint` 断言 `fake.call_count() == 0`）。**通过。**

3. **历史 provider_id 为真实值**：`translate_text_impl_selected_mymemory_writes_provider_id_in_history` 测试直接查询 DB 断言 `rows[0].provider_id == "mymemory"`，非恒真。**通过。**

---

## 问题清单

审查过程未发现置信度 ≥ 80 的真实问题，所有维度均通过，无必改项。

---

## 低于阈值的观察项（不阻断，备忘）

**`LocalMockCredStore` 与 `MockCredStore` 逻辑重复（置信度约 60%）**

`src-tauri/tests/ipc_translate.rs` 中的 `LocalMockCredStore` 与 `src-tauri/src/translate/credential.rs` 中的 `MockCredStore`（`#[cfg(test)] pub`）实现完全相同（相同的 Mutex/HashMap 结构、相同的 key 格式 `{provider_id}.{field_key}`）。集成测试文件位于 `tests/` 目录，Rust 集成测试编译时 `#[cfg(test)]` 的 `pub` 结构体对外部 test crate 是否可见，取决于编译模式——在 `cargo test` 时 `cfg(test)` 在库 crate 的测试 feature 下才展开，集成测试（独立 crate）不能直接 `use quickquick_lib::translate::credential::MockCredStore`（因为集成测试编译时依赖的是库 crate 的 non-test 版本）。因此重复定义在技术上是必要的。该重复属于 Rust 测试体系的结构性约束，不构成 DRY 违规。置信度不足 80%，不阻断。

**`baidu` 错误消息混用中英文标识（置信度约 30%）**

错误串 `"baidu 未配置 AppID，请前往设置填入 API Key"` 中 `AppID` 为英文，与 credential_schema 中 `label: "AppID"` 一致，非随意拼写；`secret_key` 错误串写作 `SecretKey`（驼峰）而非 `secret_key`（蛇形）——仅是展示名差异，不影响功能，schema 的 key 字段仍精确为 `"secret_key"`。用户体验层面可接受，置信度不足 80%，不阻断。

---

## 逐维度核查

### 1. `build_provider` 字段名与 credential_schema 对齐

credential_schema 定义：`mymemory→email`、`baidu→app_id/secret_key`、`deepl_free→auth_key`、`google→api_key`。`build_provider` 中 `find("email")`/`find("app_id")`/`find("secret_key")`/`find("auth_key")`/`find("api_key")` 与 schema 字段 key 逐字一致。`load_credentials` 按 schema 遍历，返回 `(field.key.to_string(), val)` 元组；`build_provider` 的 `find` 闭包按 key 查找，链路无断点。**通过。**

### 2. `MockCredStore` 提升仅为可见性/位置变化

diff 精确确认：`set_secret`/`get_secret` 逻辑（`format!("{provider_id}.{field_key}")`、insert/get HashMap 操作）与原始 `mod tests` 内实现完全相同，无逻辑改动。仅变化：从 `mod tests` 内私有 struct 移出为模块级 `#[cfg(test)] pub struct`，并将 `use std::collections::HashMap; use std::sync::Mutex;` 改为内联路径 `std::sync::Mutex<std::collections::HashMap<...>>`。**通过。**

### 3. 函数行数与嵌套层次

`build_provider`：59 行（含注释），嵌套最深 2 层（match arm 内 find + ok_or_else）。`translate_text_impl`：37 行主体代码，嵌套最深 2 层。全部 ≤ 50 行有效代码、≤ 3 层嵌套。**通过。**

### 4. 注释质量

`build_provider` doc-comment 说明了"为什么字段名必须与 credential_schema 逐字对齐"、"mymemory email 可选"、"不 panic 不回退"的设计约束。`translate_text_impl` doc-comment 更新了编排流程步骤编号，`# Errors` 段补充了必填凭据缺失的错误描述。无装饰性横线分隔注释，无 TODO/FIXME 残留。**通过。**

### 5. 错误处理完整性

命令层 `translate_text` 将所有 `Err` 通过 `with_db` 传播为 `String` 返回前端；`emit` 失败仅 `eprintln` 不影响主路径（s11 既有设计，本次未改动）。`KeyringCredStore` 的 keychain 操作失败表面为 `CredError::Keychain`（不含 secret 值）再 map_err 为 String。无 `unwrap()`/`panic!` 在生产路径。**通过。**

### 6. `TranslateProvider` object-safe 用法

`TranslateProvider: Send + Sync`，三个方法（`capability`/`build_request`/`parse_response`）均接受 `&self`，无泛型方法，object-safe 成立。`Box<dyn TranslateProvider>` 用法正确。**通过。**

### 7. 未知 provider_id 处理

`build_provider` 的 `other => Err(format!("未知翻译 provider：{other}"))` 明确返回 Err，不 panic 不静默回退。`build_provider_unknown_id_returns_err` 单元测试覆盖此路径。**通过。**

### 8. 测试质量

- 新增 8 条 `build_provider_*` 单元测试：覆盖各 provider 成功/缺凭据 Err/未知 id 全路径，断言具体 provider id（`capability().id`）而非空断言。
- 新增 2 条 `translate_text_impl_*` 单元测试：`_writes_provider_id_in_history` 直接查 DB 验证历史字段；`_selected_baidu_without_creds_returns_err_with_hint` 验证错误消息含"未配置"且执行器未被调用，均为非恒真断言。
- 集成测试 6 条全部更新为新签名，断言意图（方向/译文/历史条数/执行器次数）无稀释。
- AAA 结构（Arrange/Act/Assert 注释）完整。
- **通过。**

---

## 结论

**通过（无必改项）**

安全红线（secret 不泄漏）：**通过**。错误消息/传播路径全程只含字段名或 OS 级错误描述，不含字段值。

动态路由正确性：**通过**。selected_provider 真实驱动 provider 构造，缺 key 时明确 Err 且执行器不被调用，历史 provider_id 与实际一致，全部有测试覆盖且断言有效。
