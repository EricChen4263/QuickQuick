---
id: keystore-local-review
type: review
level: 小功能
parent: keystore-local
children: []
created: 2026-06-06T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 本地文件密钥库（keystore-local）

## 审查范围

| 文件 | 说明 |
|---|---|
| `src-tauri/src/keyprovider.rs` | 新 `LocalKeyProvider`（机器绑定 KEK + 0600 文件，去 Keychain） |
| `src-tauri/src/translate/credential.rs` | 新 `DbCredStore`，删 `KeyringCredStore`/`FileCredStore`/标记函数 |
| `src-tauri/src/db.rs` | `provider_secret` 建表、`DROP TABLE IF EXISTS secret_presence`、`backup_corrupt_file` 提 `pub(crate)` |
| `src-tauri/src/lib.rs` | `setup_app_db` 去 cfg 分叉 + `open_db_with_reset`/`is_resettable_open_error`/`reset_and_reopen` |
| `src-tauri/src/ipc/settings.rs` | 去 `apply_dev_subdir`，构造 `DbCredStore` |
| `src-tauri/src/ipc/translate.rs` | 构造 `DbCredStore` |
| `src-tauri/Cargo.toml` | 移除 `keyring` |

参照标准：设计文档 `docs/design/local-keystore-no-keychain.md`（§二安全声明、§四详细设计、§九风险）、Rust code-standards（安全红线 / Zeroize / 函数≤50行 / 嵌套≤3层）、项目规范。

---

## 安全关键检查

### Crypto 正确性

**Argon2id 参数：通过。**  
`ARGON2_MEM_KIB=65536`（64MB）、`ARGON2_ITERATIONS=3`、`ARGON2_PARALLELISM=4`，对齐 OWASP 2023 推荐下限（注释亦标注出处）。salt 为 32B CSPRNG，满足推荐下限 16B。passphrase 为 machine_id（UUID 约 36B 或回退常量 48B），均满足 Argon2 最小 passphrase 要求。

**AES-256-GCM nonce 唯一性：通过。**  
`write_key_file` 每次生成新随机 12B nonce，且密钥文件生命周期内只被写一次（幂等路径只读不重加密），因此每个密钥文件只有一个 (key, nonce) 对，无 nonce 复用风险。

**GCM tag 校验：通过。**  
`cipher.decrypt` 在 tag 验签失败时返回 `Err`，映射为 `KeyError::Decrypt`——解密失败即错误，无绕过。

**KEK zeroize：通过。**  
`derive_kek` 返回 `Zeroizing<[u8; 32]>`，KEK 在 `write_key_file`/`decrypt_master_key` 作用域 drop 时自动清零。`Aes256Gcm::new` 接收 `Key::from_slice(kek.as_ref())`，kek 的 Zeroizing 守卫持续到两处调用结束。

### 机器绑定 + 降级硬目标

**异机解密失败：通过。**  
异机 machine_id 不同 → 复算 KEK 不同 → AES-GCM 验签失败 → `KeyError::Decrypt`。测试 `different_machine_id_fails_to_decrypt` 以注入不同 machine_id 覆盖此路径，tester 动态验证通过。

**machine_id 取不到 → 回退常量盐，不 panic：通过。**  
三平台 `read_platform_machine_id` 全程 `Result`/`Option` 链，无 `unwrap`/`expect`/`panic`；子进程（ioreg / reg query）用 `.ok()?` 处理。`machine_id()` 兜底 `FALLBACK_MACHINE_ID`，`filter(|id| !id.is_empty())` 过滤空串回退。测试 `fallback_machine_id_still_opens_keystore` 覆盖，tester 通过。

### 不泄密

**展示路径 SELECT 1 不回明文：通过。**  
`load_credentials_for_display` 对 secret 字段走 `secret_exists`（`SELECT 1 FROM provider_secret`），返回空串不回值。`get_provider_credentials_impl` 注释已更新，无残留旧 keychain 路径。

**日志/错误消息不含密钥或 secret 值：通过。**  
`KeyError` 所有 variant 的 `#[error]` 消息仅含类型描述，不含密钥材料。`CredError::UnknownField` 只携带 `provider_id` 和 `field_key`，不携带字段值。credential.rs 全文无 `eprintln!`/`println!` 打印 secret。`hex_key`（db.rs L703）存在但注释说明了 SQLite trace 钩子未启用故不入日志，且该变量未传给 eprintln，日志安全成立。

**secret 在整库加密 SQLCipher DB：通过。**  
`provider_secret` 表与 `provider_config` 同在 SQLCipher 加密库，secret 不再落明文文件或 Keychain。

### 重置分支安全

**先备份再重建：通过。**  
`reset_and_reopen`：先 `backup_corrupt_file(db_path)`（`fs::rename` OS 原子操作），再 `backup_corrupt_file(key_path)`，最后 `LocalKeyProvider::new` 重建。备份失败则放弃重置，不无声毁数据。重置后旧 `master.key` 已备份重命名，`load_or_generate` 遇 `NotFound` 自然生成新密钥。

**触发条件精准：通过。**  
`is_resettable_open_error` 匹配 `"密钥解密失败"`（`KeyError::Decrypt` display）和 `"file is not a database"`/`"not a database"`（SQLCipher 错误文本）。`pipeline::open_app_db` 将 `KeyError::Decrypt` 包装为 `"密钥获取失败：密钥解密失败…"` — 包含子串 `"密钥解密失败"`，字符串匹配成立。

**不从 Keychain 静默读迁移：通过。**  
`ipc/settings.rs`、`ipc/translate.rs`、`translate/credential.rs` 全文无 `keyring` 引用（grep 已确认），迁移路径不触发 Keychain 弹窗。

### 退役完整性

**secret_presence 退役：通过。**  
`db.rs` `ensure_schema` 删 `secret_presence` 建表语句，新增 `DROP TABLE IF EXISTS secret_presence`（幂等迁移）。全仓 grep 确认 `secret_presence` 仅在 db.rs 迁移注释和 drop 语句中出现，无残留路由。

**keyring 依赖移除：通过。**  
`Cargo.toml` 无 `keyring`；全仓 `src/` grep 无 `keyring` 引用（仅 keyprovider.rs 模块注释历史说明，非代码引用）。

---

## Important 级问题

### I1 · decrypt_master_key 中 `plaintext Vec<u8>` 未 zeroize

**severity: Important · confidence: 82**  
**文件：`src-tauri/src/keyprovider.rs:287-291`**

```rust
let plaintext = cipher
    .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
    .map_err(|_| KeyError::Decrypt)?;
plaintext.as_slice().try_into().map_err(|_| KeyError::Decrypt)
```

`aes_gcm::Aead::decrypt` 返回 `Vec<u8>`，持有 32 字节明文主密钥。`Vec<u8>` drop 时不自动 zeroize，密钥材料短时驻留堆，进程 core dump 或堆扫描可读到。设计文档 §四#1 明确要求「主密钥用 Zeroizing 包装」，此处未落实。

同一问题也出现在 `load_or_generate`（L240）中 `let key = generate_random_key()`：`[u8; 32]` 返回栈值，经 `Ok(key)` 传递给调用方，调用方（`pipeline.rs:177`）也以 `let key` 接收，均非 `Zeroizing`。

影响评估：攻击者需在密钥 drop 后读取已释放堆内存，实际利用难度高；主要风险是进程崩溃时的 core dump。定级 Important（非 Critical）。

**建议修复：**

```rust
// decrypt_master_key 修复
let plaintext = Zeroizing::new(
    cipher.decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| KeyError::Decrypt)?
);
let key: [u8; KEY_LEN] = plaintext.as_slice().try_into().map_err(|_| KeyError::Decrypt)?;
// 返回前 plaintext Zeroizing 守卫 drop 时清零堆内存
```

`load_or_generate` 中：
```rust
let key = Zeroizing::new(generate_random_key());
self.write_key_file(&key)?;
Ok(*key)
```

注：`pipeline.rs` 的 `key` 传入 `db::open_or_create` 用于 SQLCipher `PRAGMA key`，生命周期极短，优先级低于 `decrypt_master_key` 中的堆分配。

---

## 无置信度 ≥80 的观察

- **`generate_random_key` 用 uuid v4 拼接而非直接 `rand::fill_bytes`**：uuid::new_v4 内部调用 getrandom（OS CSPRNG），熵源等价，功能无误。仅风格偏间接，不报。
- **`HEADER_LEN + 16` 最小长度检查偏宽**：合法文件 97B，检查下限 65B，65-96B 的损坏文件会通过 Format 检查但 decrypt 失败（`KeyError::Decrypt`），功能等价，不是安全问题。
- **Windows `line.split_whitespace().last()` MachineGuid 解析**：若行格式异常取到错误 token，machine_id 变化触发重置或回退常量盐，不扩大攻击面，不报。
- **`MockCredStore::store.lock().unwrap()`**：测试专用，Mutex poison 导致测试 panic 合理，不报。
- **`backup_corrupt_file` 中 `.unwrap_or_else(|| Path::new("."))`**：当路径无父目录时备份落当前工作目录，理论上不应发生（文件路径来自 app_config_dir），低风险，不报。

---

## 测试覆盖评估

tester 已动态证伪通过（命中 + 变异 A/C/D/E 红 + 不泄密/不 panic/grep 无残留 + debug+release 双绿）。

变异 B 覆盖缺口（`machine_id()` 真实平台取失败 → 回退路径注入测试）已知非阻塞。现有 `fallback_machine_id_still_opens_keystore` 通过注入 `FALLBACK_MACHINE_ID` 覆盖了回退逻辑的正确性；缺失的是「注入 `machine_id()` 返回 `FALLBACK_MACHINE_ID`」的完整端到端路径（即真实平台函数失败 → 回退）。由于 `machine_id()` 是私有函数且无注入接口，此缺口难以在不改接口的情况下弥补；结合降级设计本身的保守性（失败只退化安全级，不 panic），维持非阻塞评估。

---

## 审查结论

无 Critical 问题。存在 1 个 Important 级问题（I1：`plaintext Vec<u8>` 和 `key [u8;32]` 未 Zeroizing，偏离设计文档 §四#1 明确要求），建议修复但不阻塞合并。

所有安全核心路径（Argon2id 参数、GCM nonce 唯一性与 tag 校验、KEK zeroize、异机解密失败、降级不 panic、不泄密、重置先备份、退役完整性）均通过核查。

**VERDICT: WARNING**

`severity(Important) · confidence(82) · src-tauri/src/keyprovider.rs:287-291,240 · decrypt_master_key 中 plaintext = Vec<u8> 及 load_or_generate 中 key = [u8;32] 未用 Zeroizing 包裹，drop 时不自动清零，偏离设计文档 §四#1"主密钥用 Zeroizing 包装"明确要求；进程 core dump 可暴露密钥材料 · 将 plaintext 改为 Zeroizing::new(cipher.decrypt(...)?)，load_or_generate 中 key 改为 Zeroizing::new(generate_random_key())`
