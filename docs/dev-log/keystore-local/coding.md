# 编码留痕：本地文件密钥库（去 Keychain，永不弹密码）

> feature-dev Phase 5 实现留痕。权威设计：`docs/design/local-keystore-no-keychain.md`（已批准）。
> 严格 TDD（红-绿-重构）。本文为简留痕（非 goal-dev 三联）。

## 方案要点

把 SQLCipher 主密钥与翻译 secret 从 OS 钥匙串挪到本地加密文件 / 加密库，三平台统一、删除
`#[cfg(debug_assertions)]` 密钥分叉，永不弹密码。无 Apple Developer ID 时钥匙串 ACL 反复失效，
故彻底放弃钥匙串。

- **主密钥**：落 `config_dir/master.key`（0600），格式 `magic(4)+version(1)+salt(32)+nonce(12)+aes-256-gcm(master_key)`。
  KEK = `Argon2id(passphrase = machine_id, salt = 文件内随机盐)`，文件单拷异机 machine_id 不同 → GCM 验签失败解不开。
- **翻译 secret**：进同一 SQLCipher 整库加密的新表 `provider_secret`（不碰钥匙串、连明文 JSON 都不留）。
- **secret_presence 退役**：迁移 `DROP TABLE IF EXISTS secret_presence`，展示路径改 `SELECT 1 FROM provider_secret`。
- **一次性重置**：开库失败（密钥解密失败 / `file is not a database`）→ 备份旧库 + 旧 master.key 后重建空库。

## 关键决策

1. **machine_id 可注入**（`LocalKeyProvider::with_machine_id`）：真实入口 `new()` 用平台函数填充，
   测试注入不同 id 模拟「异机解不开」、注入回退常量验证「降级仍能开库」。
2. **降级回退（硬目标）**：三平台 machine_id 取不到（ioreg / `/etc/machine-id` / reg query 失败）→
   回退固定常量盐 `FALLBACK_MACHINE_ID` 而非 panic，保「永不弹密码、永远能开库」。子进程全程 `Result`，不 panic。
3. **加密原语复用 portable.rs 思路**：Argon2id(m=64MB,t=3,p=4) + AES-256-GCM + Zeroizing 包装 KEK，
   salt/nonce 走 OS CSPRNG（rand），不硬编码、用后 zeroize。
4. **DbCredStore 在 with_db 闭包内构造**：持 `&Connection`，与 `save_credentials(...conn)` 共用同一连接（多 shared 借用）。
5. **Windows 免新依赖**：用 `reg query` 子进程解析 MachineGuid，不引入 winreg。移除 keyring（含 apple-native/windows-native）。

## 改动文件

| 文件 | 改动 |
| --- | --- |
| `src-tauri/src/keyprovider.rs` | 重写：`LocalKeyProvider`（机器绑定 KEK + 0600 master.key + 三平台 machine_id + 降级回退），删 KeychainKeyProvider/FileKeyProvider/KeyAccessibility/KeyStorageAttributes，保留 `KeyProvider` trait + `generate_random_key` |
| `src-tauri/src/translate/credential.rs` | 重写：新 `DbCredStore`（provider_secret 表），删 KeyringCredStore/FileCredStore/default_cred_store/marker 三函数，`load_credentials_for_display` 改查 provider_secret |
| `src-tauri/src/db.rs` | `ensure_schema`：删 secret_presence 建表、加 provider_secret 建表、加 `DROP TABLE IF EXISTS secret_presence`；`backup_corrupt_file` 改 `pub(crate)` |
| `src-tauri/src/lib.rs` | `setup_app_db` 去 cfg 分叉，统一 `LocalKeyProvider::new`；新增 `open_db_with_reset`/`is_resettable_open_error`/`reset_and_reopen` 一次性重置分支 |
| `src-tauri/src/ipc/settings.rs` | `resolve_config_dir` 去 `apply_dev_subdir`（删该函数）；set/delete 命令在 with_db 闭包内构造 DbCredStore；展示注释更新；inline 测试 store 换 DbCredStore |
| `src-tauri/src/ipc/translate.rs` | `translate_text` 在 with_db 闭包内构造 DbCredStore，去 resolve_config_dir/default_cred_store 调用 |
| `src-tauri/src/pipeline.rs` | doc 注释 KeychainKeyProvider→LocalKeyProvider |
| `src-tauri/Cargo.toml` | 移除 keyring 依赖 |
| `src-tauri/tests/keyprovider.rs` | 重写为 LocalKeyProvider 集成测试（去 cfg gate；异机模拟 + 回退 + 损坏） |
| `src-tauri/tests/provider_secret.rs` | 新增（替代 secret_presence.rs）：provider_secret 往返 + 展示 present/absent + secret_presence 已退役断言 |
| `src-tauri/tests/secret_presence.rs` | 删除（被 provider_secret.rs 取代） |
| `src-tauri/tests/config_dir_isolation_test.rs` | 删除（apply_dev_subdir 已移除） |

## 安全声明（已写进 keyprovider.rs 模块注释）

放弃 OS 级访问控制；保住「密钥永不明文落盘 + 密钥/库单独拷到异机解不开」。防设备丢失 / 目录被拷 /
云同步误带 / 二手机未擦盘；不防本机其他进程 / 完整家目录被读。

## Phase 6 复审修复（reviewer I1 + tester 变异 B，只改 keyprovider.rs）

1. **主密钥中间内存 Zeroizing（reviewer I1，安全）**：`decrypt_master_key` 解密出的明文主密钥
   （`cipher.decrypt` 返回的 `Vec<u8>`）改 `Zeroizing::new(...)` 包裹再 `try_into`；`load_or_generate`
   生成路径改 `let key = Zeroizing::new(generate_random_key()); self.write_key_file(&key)?; Ok(*key)`。
   确保解密明文 / 新生成主密钥的中间内存 drop 时清零，对齐设计§四#1「主密钥用 Zeroizing 包装」。
   （trait 返回类型为 `[u8;32]` 不可改，故出口是裸数组——清零覆盖的是函数内中间副本。）

2. **回退分支判别力测试（tester 变异 B）**：把 `machine_id()` 拆出可注入的纯逻辑
   `machine_id_with_reader(reader: impl Fn() -> Option<Vec<u8>>)`，真实入口 `machine_id()` 传
   `read_platform_machine_id`。新增 4 条判别力测试直击回退分支（不再靠 `with_machine_id` 绕过真实读取）：
   - `machine_id_falls_back_to_constant_when_reader_returns_none`（reader=None → 用 FALLBACK_MACHINE_ID）
   - `machine_id_falls_back_to_constant_when_reader_returns_empty`（空 Vec → 回退）
   - `machine_id_uses_reader_value_when_available`（取到值 → 原样采用、不回退，区分两分支）
   - `local_provider_opens_with_fallback_when_reader_returns_none`（回退后 LocalKeyProvider 仍能首启+跨实例开库）
   **变异自检（红→绿证据）**：把回退 `unwrap_or_else(|| FALLBACK.to_vec())` 改成 `panic!` →
   上述 3 条回退测试立即 FAILED（panicked）；还原后全部 ok。证明非恒真、有判别力。

   修复后验证（artifacts/ 下 `*_fix.log`）：`cargo test` debug 31 result 全 ok、`cargo test --release`
   31 result 全 ok、`clippy --all-targets -D warnings` exit 0（0 warning）。
   keyprovider 单测 14 passed（原 10 + 新 4）。

## 自测结论（原始证据见 artifacts/）

- keyprovider 集成测试 5 passed（含异机解密失败 `local_provider_different_machine_fails_decrypt`、
  回退开库、首启）。
- provider_secret 集成测试 7 passed（含 secret_presence 已退役断言、展示不回明文）。
- lib 单元测试 182 passed（含 keyprovider 内联 异机/回退/0600/损坏、credential DbCredStore 往返/路由/删除）。

### 全量验证结果（均 `rtk proxy` 绕代理取原始逐行，artifacts/ 下留底）

- **`cargo test`（debug）全绿**：31 个 test result 全 ok、0 failed（`cargo_test_debug_full.log`）。
- **`cargo test --release` 全绿**：31 个 test result 全 ok、0 failed（`cargo_test_release_full.log`）——
  去 `#[cfg(debug_assertions)]` gate 后 release 测试编译通过，统一前的痛点已消失。
- **`cargo clippy --all-targets -- -D warnings` exit 0**：无 warning（`clippy.log`）。
- 关键测试名逐行 `... ok`（`key_evidence_verbose.log`/`lib_keyprovider_verbose.log`/`lib_credential_verbose.log`）：
  - 首启 `local_provider_new_first_start_succeeds` / `first_call_creates_master_key_file`
  - 幂等+跨实例 `same_instance_is_idempotent` / `new_instance_reads_same_key_with_same_machine_id` /
    `local_provider_persists_and_reads_back_same_machine`
  - 异机解密失败 `different_machine_id_fails_to_decrypt` / `local_provider_different_machine_fails_decrypt`
  - 降级回退仍开库 `fallback_machine_id_still_opens_keystore` / `machine_id_never_empty`
  - 损坏报错不 panic `corrupt_file_reports_error_not_panic`；0600 权限 `master_key_file_has_0600_permissions`
  - provider_secret 往返/路由 `db_cred_store_set_get_roundtrip` / `save_credentials_routes_secret_to_provider_secret_table`
  - display present 不回明文 `save_then_display_reports_secret_present_without_plaintext` /
    `load_for_display_reports_secret_present_without_returning_plaintext`
  - secret_presence 已退役 `ensure_schema_drops_retired_secret_presence_table`

### grep 无残留

仅 `secret_presence` 在 db.rs 退役迁移（`DROP TABLE IF EXISTS`）+ provider_secret.rs「验证已退役」测试中刻意出现；
`keyring`/`KeychainKeyProvider`/`FileCredStore`/`apply_dev_subdir`/`FileKeyProvider`/`KeyringCredStore`/
`default_cred_store`/`KeyAccessibility`/`KeyStorageAttributes` 全部 0 命中。

### 安全红线核对

- 威胁模型安全声明写进 keyprovider.rs 模块注释（放弃 OS 访问控制，保住「不明文落盘 + 异机解不开」）。
- KEK 用 `Zeroizing` 包装、Drop 自动清零；salt/nonce/主密钥走 OS CSPRNG（rand / uuid v4），不硬编码、不入日志。
- 子进程 `ioreg`/`reg query` 全 `.output().ok()?` → 失败回退常量盐，生产路径无 `unwrap/expect/panic`。
