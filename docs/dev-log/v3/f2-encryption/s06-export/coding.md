---
id: V3-F2-S06-code
type: coding_record
level: 小功能
parent: V3-F2
created: 2026-05-31T03:08:36Z
status: 通过
commit: WIP
acceptance_ids: [V3-F2-A07]
author: coder
---

# V3-F2-S06 导出/导入便携文件（口令保护）编码记录

## 验收命中

- **V3-F2-A07** export_import_passphrase：4 个测试全部命中并通过
  - `export_import_passphrase_roundtrip`：往返成功
  - `export_import_passphrase_wrong_passphrase_returns_err`：错口令返回 Err
  - `export_import_passphrase_ciphertext_does_not_contain_plaintext`：密文不含明文
  - `export_import_passphrase_truncated_blob_returns_format_err`：截断格式错误

全量测试 0 failed，clippy 零警告，build 通过，无装饰注释，无 TODO/FIXME。

## 实现摘要

### 口令 KDF（Argon2id）

使用 `argon2` crate（v0.5）的 Argon2id 算法，参数：
- 内存：65536 KiB（64 MB），OWASP 2023 推荐下限
- 迭代：3 次
- 并行度：4 线程
- 输出：32 字节对称密钥

Salt 32 字节，由 `rand::thread_rng()` 从 OS CSPRNG 生成，随 blob 明文存储（salt 非秘密）。

### 便携文件格式

```
offset  len   字段
0       4     magic: 0x51 0x51 0x42 0x50 ("QQBP")
4       1     version: 0x01
5       32    Argon2id salt（随机）
37      12    AES-GCM nonce（随机）
49      N     AES-GCM ciphertext + 16-byte authentication tag
```

总头部 49 字节，后接加密数据。最小有效 blob 长度 = 49 + 16 = 65 字节。

### AES-256-GCM 加解密往返

- 加密：`aes-gcm` crate（v0.10），`Aes256Gcm::encrypt`，输出含 ciphertext + tag
- 解密：`Aes256Gcm::decrypt`，AEAD 自动验签，tag 不匹配返回错误

往返验证：`export_portable(data, pass)` → `import_portable(blob, pass)` == data

### 错口令验签失败

错误口令经 KDF 派生出不同密钥 → AES-GCM `decrypt` 返回 `Err` → 映射为 `PortableError::WrongPassphrase`。不 panic，不返回 Ok，不泄露任何口令信息。

### 口令不入日志

`PortableError` 所有变体的错误消息均不含口令明文。`derive_key` 内部的 KDF 失败映射为 `PortableError::Kdf`，消息为"密钥派生失败"，无口令内容。

## 关键决策

| 决策 | 选项 | 选择理由 |
|---|---|---|
| KDF | argon2id vs pbkdf2+sha2 | Argon2id 内存硬化，抗 GPU/ASIC 暴力破解；OWASP 2023 首选 |
| AEAD | aes-gcm vs chacha20poly1305 | AES-GCM 更普遍，硬件加速支持广，已有 AES-NI |
| salt 长度 | 16 vs 32 字节 | 32 字节（256 bit）熵更充裕，碰撞概率可忽略 |
| Argon2 参数 | m=64MB,t=3,p=4 | OWASP 2023 推荐下限，在测试环境约 2 秒内完成 |
| rand 版本 | 0.8 | 项目已有 uuid（依赖 getrandom），rand 0.8 兼容同一 getrandom 版本族 |

## 改动文件

| 文件 | 变更类型 | 说明 |
|---|---|---|
| `src-tauri/Cargo.toml` | 新增依赖 | aes-gcm 0.10、argon2 0.5、rand 0.8 |
| `src-tauri/src/portable.rs` | 新建 | 核心实现：KDF、格式定义、export/import 函数、PortableError |
| `src-tauri/src/lib.rs` | 新增 pub mod | 注册 `pub mod portable;` |
| `src-tauri/tests/portable.rs` | 新建 | 集成测试：4 个 export_import 测试用例 |

## TDD 流程记录

1. RED：先写 `tests/portable.rs`，引用不存在的 `quickquick_lib::portable`，编译报 `unresolved import`，确认红。
2. GREEN：实现 `Cargo.toml` 依赖 → `src/portable.rs` → `lib.rs` 注册，4 测试全绿。
3. REFACTOR：函数均在 50 行以内，嵌套不超过 3 层，无需额外重构。

## code-standards 自检

- 函数长度：`derive_key`（11 行）、`export_portable`（30 行）、`import_portable`（35 行），均 ≤ 50 行
- 嵌套深度：最深 2 层（`map_err` 链式调用），≤ 3 层
- 命名：函数用动词+名词（`derive_key`、`export_portable`、`import_portable`），常量全大写
- 注释：注释写"为什么"（KDF 参数引用 OWASP、salt 非秘密原因），无装饰分隔符
- 安全：口令及派生密钥绝不入日志或错误消息，错误消息仅说"口令错误"不含口令值
- 无裸 unwrap/panic：所有错误路径用 `?` 或 `map_err` 传播
- 无 TODO/FIXME：已确认
- clippy -D warnings：通过（exit=0）

## 审查回归（第 1 次打回修复）

按 code-reviewer 审查意见修复两项 Important：

### I-1：导出随机性测试

在 `src-tauri/tests/portable.rs` 补充 `export_produces_distinct_blobs_each_call`：相同明文+相同口令连续两次调用 `export_portable`，断言 `blob1 != blob2`，证明每次导出的 salt/nonce 均由 CSPRNG 独立生成（碰撞概率约 2^-192）。测试为非恒真 AAA 结构，若随机性退化（如固定 seed）测试必然失败。

### I-2：派生密钥 Zeroizing 内存清零

- `src-tauri/Cargo.toml` 直接声明 `zeroize = { version = "1" }`（纵深防御依赖显式化）。
- `src-tauri/src/portable.rs` 的 `derive_key` 返回类型由 `[u8; KEY_LEN]` 改为 `Zeroizing<[u8; KEY_LEN]>`：内部用 `Zeroizing::new([0u8; KEY_LEN])` 初始化，`hash_password_into` 写入其中，Drop 时自动 `memset(0)` 清零派生密钥。
- `export_portable` 和 `import_portable` 调用方改为 `Key::from_slice(key_bytes.as_ref())` 透明解包，加解密逻辑不变。

回归结果：portable 5 passed（含新随机性测试）、全量 0 failed、clippy 零警告、无 TODO/FIXME。
