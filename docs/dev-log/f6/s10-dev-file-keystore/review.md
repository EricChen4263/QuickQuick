---
id: F6-S10-review
type: review
level: 小功能
parent: F6
children: []
created: 2026-06-04T00:00:00Z
status: 未过
commit: WIP
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · dev 文件密钥库（F6-S10）

## 审查范围

| 文件 | 说明 |
|---|---|
| `src-tauri/src/keyprovider.rs` | 新增 `FileKeyProvider`（debug_assertions 门控，文件存主密钥） |
| `src-tauri/src/translate/credential.rs` | 新增 `FileCredStore` + `default_cred_store` cfg 工厂 |
| `src-tauri/src/lib.rs` | `setup_app_db` 中 cfg 选 KeyProvider |
| `src-tauri/src/ipc/settings.rs` | 新增 `resolve_config_dir` + 三处凭据命令改用 `default_cred_store` |
| `src-tauri/src/ipc/translate.rs` | `translate_text` 改用 `default_cred_store` |

参照标准：Rust code-standards（安全红线 / 函数≤50行 / 嵌套≤3层）、项目规范。

---

## 安全关键检查（cfg 隔离）——通过

- `FileKeyProvider` 结构体 / `impl FileKeyProvider` / `impl KeyProvider for FileKeyProvider` 三处均以 `#[cfg(debug_assertions)]` 守卫，release 二进制不含任何相关代码，安全隔离成立。
- `FileCredStore` 同理：L130 / L136 / L177 三处 `#[cfg(debug_assertions)]` 完整。
- `default_cred_store` 两个版本互斥完整：`cfg(debug_assertions)` 返回 `FileCredStore`，`cfg(not(debug_assertions))` 返回 `KeyringCredStore`，无漏洞。
- `setup_app_db`（lib.rs L199-202）两分支完整。

## 文件安全约束——通过

- `write_key_file` 和 `write_all` 均正确：`std::fs::write` 后接 `#[cfg(unix)]` 的 `set_permissions(0o600)`，非 unix 不 panic 不报错。
- `app_config_dir()` 解析到 `~/Library/Application Support/com.quickquick.app/`，不在 git 工作树，无误提交风险。

## 错误传播——通过

所有 IO 错误均通过 `map_err` 映射为 `KeyError::Backend` / `CredError::Keychain`，无静默吞错。文件长度非 32 字节通过 `KeyError::InvalidKeyLength` 上报，不 panic。

## 日志安全——通过

credential.rs 全文无打印 secret 值，错误消息仅含类型描述，符合安全红线。

## 函数长度——通过

所有新增函数均在 50 行内，嵌套不超 3 层。`resolve_config_dir` / `resolve_config_path` 重构干净无重复。

---

## 必改项

### C1 · cargo test --release 编译失败——测试模块缺 debug_assertions 双重门控

**severity: Critical · confidence: 90**

`file_cred_store_tests`（credential.rs:543）和 `file_provider_tests`（keyprovider.rs:326）只有 `#[cfg(test)]` 而无 `#[cfg(debug_assertions)]`。`FileCredStore`/`FileKeyProvider` 在 `#[cfg(debug_assertions)]` 下编译，`cargo test --release` 开启 `cfg(test)` 但关闭 `cfg(debug_assertions)`，导致两个测试模块内对这两个类型的引用编译失败。

**必须修复：**

```rust
// credential.rs L543
#[cfg(all(test, debug_assertions))]
mod file_cred_store_tests { ... }

// keyprovider.rs L326
#[cfg(all(test, debug_assertions))]
mod file_provider_tests { ... }
```

---

## 非阻塞建议

### I1 · FileCredStore::delete_secret 文件不存在时静默创建空 JSON（副作用与注释矛盾）

**severity: Important · confidence: 82**

`credential.rs:190-195`：`delete_secret` 调 `read_all`（文件不存在返回空 map）→ `remove` 无效 → `write_all` 把 `{}` 写盘，**在文件不存在时创建了空文件**。注释说"幂等，不报错"，但文件系统视角"不存在→建立空文件"并非严格 no-op。测试 `file_store_delete_missing_is_ok` 只断言 `result.is_ok()`，未验证文件未被创建。

建议：key 不存在时跳过 write_all：
```rust
let removed = map.remove(&keychain_account(provider_id, field_key));
if removed.is_some() {
    self.write_all(&map)?;
}
Ok(())
```

### I2 · MockCredStore key 格式与 FileCredStore/KeyringCredStore 不一致

**severity: Important · confidence: 80**

`credential.rs:526-537`：`MockCredStore` 用 `"{provider_id}.{field_key}"` 格式，生产实现用 `keychain_account()` → `"cred.{provider_id}.{field_key}"`。跨实现测试时若期望 key 一致性，静默不同。建议 `MockCredStore` 也复用 `keychain_account()`。

---

## 无置信度 ≥80 的其他问题

- `CredError::Keychain` 用于文件 IO 错误（语义略混淆）：trait 无"文件错误"专用 variant，用 `Keychain` 作通用后端错误容器是合理妥协，不报。
- write→set_permissions 微秒级 TOCTOU 窗口：dev only 场景可接受，不报。

---

## 审查结论

存在 1 个 Critical 问题（C1：`cargo test --release` 编译失败），需修复后重审。

**VERDICT: BLOCK**

`severity(Critical) · confidence(90) · src-tauri/src/translate/credential.rs:543 + src-tauri/src/keyprovider.rs:326 · file_cred_store_tests / file_provider_tests 仅有 #[cfg(test)] 而无 #[cfg(debug_assertions)]，cargo test --release 下 FileCredStore/FileKeyProvider 不编译但测试模块引用它们，编译失败 · 改为 #[cfg(all(test, debug_assertions))]`

`severity(Important) · confidence(82) · src-tauri/src/translate/credential.rs:190-195 · FileCredStore::delete_secret 在文件不存在时 write_all 空 map 会静默创建 dev-credentials.json，与"幂等 no-op"注释矛盾 · 检查 removed.is_some() 再决定是否 write_all`

`severity(Important) · confidence(80) · src-tauri/src/translate/credential.rs:526-537 · MockCredStore key 格式 "{pid}.{key}" 与生产 "cred.{pid}.{key}" 不一致，跨实现测试有隐性差异 · MockCredStore 改用 keychain_account()`
