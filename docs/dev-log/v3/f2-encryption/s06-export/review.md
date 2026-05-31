---
id: V3-F2-S06-review
type: review
level: 小功能
parent: V3-F2
children: []
created: 2026-05-31T05:30:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F2-A07]
evidence: []
author: code-reviewer
---

# 代码审查 · V3-F2-S06 导出/导入便携文件（口令保护）

## 审查范围
- `src-tauri/src/portable.rs`（export_portable/import_portable/derive_key/PortableError）+ Cargo.toml(aes-gcm 0.10/argon2 0.5/rand 0.8) + `tests/portable.rs`（4 用例）
依据：code-standards 安全红线 + 设计§六 + 密码学正确性。

## Critical：无（三条安全红线通过）
- **nonce 不复用**：每次 export 独立 `rng.fill_bytes(nonce)` OS CSPRNG。
- **口令不入日志**：无 println/log 调用；PortableError 消息不含口令/密钥。
- **错口令必验签失败**：import KDF 后 AES-256-GCM decrypt，错口令 tag 验证失败→WrongPassphrase，不 panic 不返回 Ok。

## Important 问题
**[I-1] 缺"两次导出 salt/nonce 不同"测试（置信度 82）**
- 位置：`tests/portable.rs`（4 用例无"相同明文+口令连续两次 export → blob 不同"断言；salt/nonce 随机性脱离测试保护）。
- 修复：补 `export_produces_distinct_blobs_each_call`：两次 export 同输入 `assert_ne!(blob1, blob2)`。

**[I-2] 派生密钥未 zeroize（置信度 80）**
- 位置：`portable.rs`（derive_key 返回 `[u8;32]`，函数退出不保证清零；crash dump/swap 可能留明文）。zeroize 已是 aes-gcm/argon2 间接依赖。
- 修复：derive_key 返回 `zeroize::Zeroizing<[u8;32]>`（Drop 自动清零），Cargo.toml 直接声明 zeroize；调用方用包装类型。

## 其余维度核查（通过）
Argon2id m=65536/t=3/p=4（OWASP 2023）；salt 32B/nonce 12B 每次随机 OS CSPRNG；AES-256-GCM 256-bit key；格式解析 magic/version/截断均 Format Err 无 panic；无裸 unwrap/panic（map_err/?）；无装饰注释/TODO；函数 ≤35 行嵌套 ≤2。

## 结论
**未过（打回）。** 修 I-1（导出随机性测试）+ I-2（派生密钥 Zeroizing）后复审。安全红线已全通过，核心密码学正确。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-1 已解决**：tests/portable.rs 新增 `export_produces_distinct_blobs_each_call`（同明文+口令两次 export assert_ne! blob 不同），真随机性断言非恒真。
- **I-2 已解决**：Cargo.toml 顶层声明 zeroize；derive_key 返回 `Zeroizing<[u8;KEY_LEN]>`（Drop 自动清零派生密钥），调用方 `.as_ref()` 适配，加解密逻辑不变。
安全红线（nonce 不复用/口令不入日志/错口令验签失败）未破坏；无新增高危；portable 5 测试全过。
