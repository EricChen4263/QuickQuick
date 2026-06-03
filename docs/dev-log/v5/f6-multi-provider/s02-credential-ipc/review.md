---
id: V5-F6-S02-review
type: review
level: 小功能
parent: V5-F6
children: []
created: 2026-06-03T07:00:00Z
status: 通过
commit: e838919
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 多翻译源批次B 凭据IPC（V5-F6-S02）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src-tauri/src/ipc/settings.rs` | 全文静态读 | 新增 2 DTO、3 纯 impl、3 Tauri 命令（603–753 行），6 条新单元测试（982–1073 行） |
| `src-tauri/src/lib.rs` | diff | invoke_handler 追加 3 条命令注册（+3 行） |
| `src-tauri/src/translate/credential.rs` | 全文静态读（依赖） | CredStore trait / CredError / credential_schema / save_credentials / load_credentials |

参照：Rust 规范（函数≤50行/嵌套≤3层/Result不panic/注释写为什么/禁装饰注释/禁TODO_FIXME/错误处理完整/日志不打印敏感信息）、code-standards、项目规范。

---

## 重点审查结论

### 安全红线：secret 明文不出现在任何返回/日志/错误（明确判定：通过）

逐路径核查：

1. **`get_provider_credentials_impl` 返回路径**：第 676–680 行 `if field.is_secret { CredentialValueDto { key: ..., value: None, is_set: saved_value.is_some() } }` 分支强制 `value: None`，代码不执行 `value: saved_value` 赋值路径。即便 `load_credentials` 已将 secret 明文取入内存局部变量 `saved_value`，该变量也只用于 `is_some()` 判断，不流入返回值。**通过。**

2. **`set_provider_credentials_impl` 错误路径**：`save_credentials(...).map_err(|e| e.to_string())`，`e` 为 `CredError`。`CredError::UnknownField` 的 `#[error("provider '{provider}' 不存在字段 '{field}'")]` 只含 provider_id 和 field_key，不含字段值；`CredError::Keychain(String)` 的 String 来自 keyring crate 自身错误消息（`PlatformFailure`/`NoEntry` 等 OS 级描述），keyring 库不在错误消息中回显写入的 secret 值；`CredError::Db` 来自 rusqlite，也不含业务字段值。**通过。**

3. **日志打印**：`settings.rs` 603–753 行（所有凭据相关函数）无任何 `println!`/`eprintln!`/`log::`/`tracing::`/`dbg!` 调用。全文件唯一一处 `eprintln!`（第 160 行）在排除名单写锁中毒的既有逻辑中，不涉及凭据。`credential.rs` 全文也无日志输出。**通过。**

4. **测试安全断言**：`get_provider_credentials_impl_secret_field_value_is_always_none` 测试明确断言 `assert!(secret_field.value.is_none(), "secret 字段的 value 永远应为 None（不回明文）")`，是有效的非恒真断言（先 `set_provider_credentials_impl` 写入真实值，再 `get` 断言 None）。**通过。**

---

### 正确性（明确判定：通过）

1. **schema 映射完整性**：`credential_schema` 覆盖 4 个 provider（mymemory/baidu/deepl_free/google），`get_provider_credential_schema_impl` 完整映射所有字段（key/label/is_secret/required），字段名逐字对齐（`&'static str` → `String`）。未知 provider 返回空 Vec，与下层 `credential_schema` 的 `_ => vec![]` 语义一致，不 panic。**通过。**

2. **`is_set` 语义正确性**：
   - secret 字段：`is_set = saved_value.is_some()`，其中 `saved_value` 来自 `load_credentials` 对 `store.get_secret` 的调用——secret 存在时返回 `Some`，不存在时 `load_credentials` 不将该字段加入结果集（不是 None 占位），故 `find(|(k,_)| k == field.key).map(...)` 返回 `None`，`is_set = false`。语义正确：有存才 true，没存是 false。**通过。**
   - 非密字段：`value = saved_value`（`Some` 或 `None`），`is_set = value.is_some()`，两者等价，符合 DTO 注释约定。**通过。**

3. **`HashMap→&[(&str,&str)]` 借用正确性**：`values.iter()` 返回 `HashMap` 借用的迭代器，`.map(|(k,v)| (k.as_str(), v.as_str()))` 产生的 `&str` 切片指向 HashMap 内部字符串，`pairs: Vec<(&str, &str)>` 的生命周期绑定到 `values`（本函数参数，在整个函数体内有效），`save_credentials(provider_id, &pairs, ...)` 调用在 `values` 离开作用域之前完成。借用链正确，编译器已通过验证（coding.md 记录 `cargo check` exit 0）。**通过。**

4. **未知 provider/field 的错误透传**：`save_credentials` 对 `schema.is_empty()` 返回 `CredError::UnknownProvider`；对 field 未找到返回 `CredError::UnknownField`；两者均通过 `map_err(|e| e.to_string())` 变为 `Err(String)` 传播，命令层 `with_db` 透传此 Err 给 Tauri，最终返回前端。错误链完整。**通过。**

---

### 一致性（明确判定：通过）

1. **DTO camelCase**：`CredentialFieldDto` 和 `CredentialValueDto` 均标注 `#[serde(rename_all = "camelCase")]`，字段名（`key`/`label`/`isSecret`/`required`/`value`/`isSet`）与 coding.md 签名表一致，与前端 TypeScript Record 对齐。**通过。**

2. **薄命令+纯 impl 模式**：3 个 Tauri 命令均为 2–5 行薄包装（构造 `KeyringCredStore`，调 `with_db` 传入 impl 函数），与 settings.rs 既有命令（`get_hotkeys`/`set_hotkey`/`get_exclude_list` 等）模式完全一致。**通过。**

3. **`with_db` 用法**：`get_provider_credentials` 和 `set_provider_credentials` 均通过 `super::with_db(&state, |conn| { ... })` 调用，与 `translate.rs` 中已有用法一致，Mutex 中毒和 DB 不可用两种错误场景统一处理。**通过。**

---

### 规范合规（明确判定：通过）

1. **函数行数**：`get_provider_credentials_impl` 约 30 行（含注释），`set_provider_credentials_impl` 约 8 行，`get_provider_credential_schema_impl` 约 8 行，全部远低于 50 行上限。**通过。**

2. **嵌套深度**：`get_provider_credentials_impl` 最深处为 `.map()` 闭包内的 `if field.is_secret { ... } else { ... }`，闭包算 1 层，if-else 算 2 层，共 2 层，≤ 3 层。**通过。**

3. **注释质量**：所有函数 doc-comment 均说明"为什么"（安全约定理由、路由规则、语义定义），DTO 结构体字段注释明确了安全约定（`secret 字段永远为 None`）。无装饰性横线分隔，无 TODO/FIXME。**通过。**

4. **测试质量（6 条）**：
   - `get_provider_credential_schema_impl_baidu_returns_two_fields`：断言字段数 + 字段名，非空断言。
   - `get_provider_credential_schema_impl_unknown_returns_empty`：边界路径。
   - `get_provider_credentials_impl_unset_fields_are_not_set`：未存状态下所有字段 is_set=false、value=None。
   - `get_provider_credentials_impl_secret_field_value_is_always_none`：先写后读，断言 secret value=None（安全核心断言），同时断言非密字段 value=Some（互补对比）。**最重要安全断言，非恒真。**
   - `set_provider_credentials_impl_persists_and_loadable`：端到端持久化验证 + secret value=None 双重断言。
   - `set_provider_credentials_impl_unknown_field_returns_err`：错误路径覆盖。
   - 全部 AAA 结构完整（无 Arrange 注释，但结构清晰可辨）。**通过。**

5. **`make_cred_db` 辅助函数**：在 `mod tests` 内建表逻辑（SQL 手动建表以绕开 `db::ensure_schema`）有注释说明原因，与 `credential.rs` 测试中相同模式一致，是 Rust 单元测试的结构性必要做法。**通过。**

---

## 问题清单

审查过程未发现置信度 ≥ 80 的真实问题，所有维度均通过，无必改项。

---

## 低于阈值的观察项（不阻断，备忘）

**`set_provider_credentials_impl` 接收 `HashMap<String, String>` 拥有所有权（置信度约 40%）**

函数签名接 `values: HashMap<String, String>`（按值移入），而非 `&HashMap<String, String>`。在此场景下两者均可编译通过（借用引用同样能 `.iter()`），但 Tauri 命令层传入时 `values` 已被 Tauri 反序列化为新 HashMap，按值传入无额外拷贝开销。现有设计不影响正确性，且与 Tauri 命令签名（需要 `Deserialize`，按值接收更自然）对齐。置信度不足 80%，不阻断。

**`get_provider_credentials_impl` 中 `loaded` 与 `schema` 双重遍历（置信度约 20%）**

先 `load_credentials` 返回 `Vec<(String, String)>`，再 `credential_schema` 再次返回 schema，在 `map` 闭包中用 `loaded.iter().find(...)` 做 O(n*m) 查找。对凭据字段数量（当前最多 2 个字段/provider）不构成性能问题，不超过合理复杂度。置信度不足 80%，不阻断。

---

## 结论

**通过（无必改项）**

安全红线（secret 明文不出现在任何返回/日志/错误）：**通过**。`get` 路径强制 `value: None`，`set` 错误消息只含字段名，全路径无日志输出，测试有效安全断言覆盖核心场景。

正确性：**通过**。schema 映射完整，is_set 语义正确，借用链安全，未知 provider/field 错误透传完整。

一致性与规范：**通过**。DTO camelCase、薄命令+纯 impl 拆分、with_db 用法均与既有约定对齐；函数行数、嵌套、注释、测试质量全部符合项目规范。
