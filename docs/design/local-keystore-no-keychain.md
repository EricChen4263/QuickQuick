# 本地文件密钥库改造方案：保留加密、永不弹密码

> 适用范围：QuickQuick 桌面端（Tauri2 + Rust）· 制定 2026-06-06 · 状态：已批准待实现

## 一、背景与动机

起点是一个对照问题：**为什么 Maccy 等剪贴板工具不填密码也能一直保存数据？** 由此引出对 QuickQuick 现状的评估。

- **Maccy 零密码的本质**：它根本不做应用级加密，明文存本地 SQLite，靠 OS 文件权限 + FileVault 兜底，因而完全不碰 Keychain、永不弹密码。代价是已登录会话下剪贴板历史明文可读。
- **QuickQuick 现状**：剪贴板历史走 SQLCipher 整库 AES-256 加密；主密钥在 release 存 macOS Keychain，在 dev 存本地文件（`FileKeyProvider`）。翻译 API secret 同样 release 走 Keychain、dev 走本地 JSON。`secret_presence` 标记表用于让设置页不读 Keychain、避免打开即弹一排授权框。
- **弹密码的真因**：Keychain item 的 ACL 绑定 App 签名主体；签名主体不稳定就会反复弹授权。**本项目没有 Apple Developer ID**，正式版也是 ad-hoc / 无稳定签名，所以 release 也会反复弹——"靠稳定签名让 Keychain 静默放行"这条路被现实否决。

**目标**：在「无 Developer ID + 保留加密 + 零密码」三约束下，把密钥从 Keychain 挪到本地文件密钥库（推广 dev 已有的 `FileKeyProvider` 思路），三平台统一，密钥用**机器绑定派生**加固。

### 方案选型对照

| 方案 | 加密 | 弹密码 | 可行性 | 结论 |
| --- | --- | --- | --- | --- |
| A 学 Maccy 去加密 | ✗ 明文落盘 | 永不弹 | 可行 | 安全降级，剪贴板含密码/token 不接受 |
| B1 修签名链让 Keychain 静默 | ✓ 保留 | 不弹 | 需 Developer ID | 无证书，否决 |
| B2 机器标识派生不落盘 | ✓ 保留 | 不弹 | 可行但≈混淆 | 安全约等明文，否决 |
| B3 用户口令派生 | ✓ 保留 | 每次输口令 | 可行 | 是加密码，与目标相反，否决 |
| **本方案 本地文件密钥库 + 机器绑定** | ✓ 保留 | 永不弹 | 可行 | **采纳** |

## 二、安全声明（诚实交代）

须同时写进代码注释与 release note。

> 相比 Keychain 方案，本地密钥库放弃了「OS 级访问控制 + 锁屏后台保护」——任何能读到你登录用户家目录的进程 / 人都能解开剪贴板库。**保住的是**：数据永不明文落盘 + 密钥文件 / 数据库被单独拷到别的机器也解不开。**防的是**设备丢失、磁盘 / 目录被单独拷走、云同步盘 / Time Machine 误带走、二手机未擦盘；**不防**本机其他进程、完整家目录被读。

注：Keychain 那层 ACL 在本项目无稳定签名下本就反复失效、形同虚设，所以这层「损失」在真实威胁模型下损失有限。

## 三、总体方案

删除 `#[cfg(debug_assertions)]` 的密钥 / 凭据分叉，三平台统一走一个 `LocalKeyProvider`（机器绑定 KEK + 0600 文件）；翻译 secret 改存进 SQLCipher 加密库内一张新表（不再碰 Keychain，连明文 JSON 都不留）；`secret_presence` 标记表退役。预发布单用户，采取**一次性重置**迁移（备份旧库重建 + 提示重填翻译 key），不做无损迁移。

## 四、详细设计

### 1. `LocalKeyProvider`（src-tauri/src/keyprovider.rs）

替换 `KeychainKeyProvider` + cfg 版 `FileKeyProvider` 为单一 `LocalKeyProvider`。保留 `KeyProvider` trait 与 `generate_random_key()`（测试与 `setup_app_db` 依赖）。

密钥文件落正式 `config_dir` 根（不再用 `dev` 子目录），文件名 `master.key`，权限 0600。文件内容复用 `portable.rs` 的头部格式思路：`magic + version + salt(32) + nonce(12) + aes-256-gcm(master_key)`。

- **KEK 派生**：`KEK = Argon2id(passphrase = machine_id, salt = 文件内随机盐)`。盐随密钥文件落盘，但 KEK 复算还需机器标识——文件单拷到异机 machine_id 不同 → GCM 解密失败。复用已有 `argon2` / `aes-gcm` / `zeroize` / `rand` 依赖，主密钥用 `Zeroizing` 包装。
- **`get_or_create_key()`**：文件不存在 → 生成 32 字节主密钥 → 取 machine_id + 新随机盐 → Argon2id 得 KEK → GCM 封装 → 0600 落盘 → 返回；文件存在 → 读盐 + 取 machine_id → 复算 KEK → GCM 解密 → 校验 32 字节 → 返回；解密失败 → 返回明确 `KeyError`，由 lib.rs 走重置分支。
- **`machine_id()` 三平台分支**：macOS = `IOPlatformUUID`（`ioreg -rd1 -c IOPlatformExpertDevice` 子进程解析，免新依赖）；Linux = `/etc/machine-id`（回退 `/var/lib/dbus/machine-id`）；Windows = 注册表 `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`（`reg query` 子进程免依赖，或加 windows-only `winreg`）。
- **降级回退（硬目标）**：任一平台取不到机器标识 → 回退固定常量盐（退化为纯 0600 安全级）而非 panic。「永不弹密码、永远能开库」是硬目标，机器绑定是增益不是阻塞。子进程调用（ioreg / reg）必须 `Result` 处理 + 回退，不 panic（遵项目「第三方 / 外部调用先确认错误表面」规约）。
- **可测性**：把 machine_id 设计成可注入（构造器或内部接受 `machine_id: &[u8]`），真实入口用平台函数填充，便于异机模拟单测。

### 2. 翻译 secret 入加密库（src-tauri/src/translate/credential.rs）

新增 `DbCredStore` 实现现有 `CredStore` trait，删除 `KeyringCredStore` / `FileCredStore` / cfg 版 `default_cred_store`。

- 新表 `provider_secret(provider_id TEXT, field_key TEXT, value TEXT, PRIMARY KEY(provider_id, field_key))`，**普通 TEXT 列存进 SQLCipher 加密库**（库已整库加密，最小改动；额外 GCM 层列为后续可选加固）。
- `DbCredStore` 在命令层 `with_db` 闭包内拿 `&Connection` 构造（与现有 `save_credentials(...conn)` 签名契合）。
- 路由规则不变：非密字段仍走 `provider_config`，secret 走 `provider_secret`，由 `credential_schema` 的 `is_secret` 判定。

### 3. `secret_presence` 退役

secret 进同一加密库后读取无弹窗成本，标记表失去存在理由。

- 删 `write_secret_marker` / `delete_secret_marker` / `secret_marker_exists` 及调用。
- `load_credentials_for_display` 改为 `SELECT 1 FROM provider_secret WHERE ...` 判断 is_set（语义不变：secret 已配置则回空串、不回明文）。
- db.rs `ensure_schema`：删 `secret_presence` 建表、加 `provider_secret` 建表；迁移时 `DROP TABLE IF EXISTS secret_presence`。

### 4. 接线与配置目录

- **lib.rs `setup_app_db`**：去 cfg 分叉，统一 `LocalKeyProvider::new(&dir)`；新增「开库失败 → 一次性重置」分支——密钥解密失败或 `file is not a database` 时，调用已有 `backup_corrupt_file`(db.rs) 备份旧库 + 重命名旧 `master.key`，再重建。覆盖老 dev（密钥在 `dev/`）与老 release（密钥在 Keychain）两种迁移场景，统一走备份重建。
- **ipc/settings.rs**：`resolve_config_dir` 去掉 `apply_dev_subdir`，删该函数；`set/delete_provider_credentials` 在 `with_db` 闭包内构造 `DbCredStore`；`get_provider_credentials_impl` 注释更新（数据源换表）。
- **ipc/translate.rs**：`default_cred_store(&config_dir)` 调用点改在拿到 conn 处构造 `DbCredStore`。

### 5. 依赖清理（src-tauri/Cargo.toml）

全仓 grep 确认无残留引用后移除 `keyring`（含 apple-native / windows-native feature）。Windows 若选 `winreg` 方案则加 windows-only 依赖；否则用 `reg query` 子进程免依赖。

## 五、迁移与重置范围

预发布期、单用户，须写进 release note。**不做无损迁移。** 改造后首启：

- 旧库无法用新密钥解密 → 备份重建 → **剪贴板历史重置**。
- 翻译 secret 在旧 Keychain / 旧 JSON，新库无 → **需用户到设置页重填一次**（≤3 个 provider，成本极低）。
- 不尝试从旧 Keychain 静默读迁移 secret——那一次读取在 ad-hoc 签名下可能弹窗，与「永不弹密码」冲突。

## 六、关键文件

| 文件 | 改动 |
| --- | --- |
| `src-tauri/src/keyprovider.rs` | 核心：新 `LocalKeyProvider` + `machine_id()` + 降级回退 |
| `src-tauri/src/translate/credential.rs` | 新 `DbCredStore`、删 marker 三函数、改 display |
| `src-tauri/src/db.rs` | `ensure_schema`：删 `secret_presence`、加 `provider_secret` |
| `src-tauri/src/lib.rs` | `setup_app_db`：去分叉 + 解密失败重置分支 |
| `src-tauri/src/ipc/settings.rs` | 去 dev 隔离、构造 `DbCredStore` |
| `src-tauri/src/ipc/translate.rs` | 构造 `DbCredStore` |
| `src-tauri/Cargo.toml` | 移除 keyring |

## 七、测试策略

- `tests/keyprovider.rs`：删 Keychain mock / accessibility / device_only 断言；新增 `LocalKeyProvider` 首启生成 + 幂等 + 跨实例读、异机模拟（注入不同 machine_id → 解密失败）、降级回退（machine_id 不可用仍能开库）、损坏文件报错。keyprovider.rs 内联 `file_provider_tests` 迁移为 `LocalKeyProvider` 测试并去掉 `#[cfg(debug_assertions)]` gate。
- `tests/secret_presence.rs`：整体改写为 `provider_secret` 等价测试（保存后 display 报 present 不回明文、删除后 absent、display 不弹窗），可重命名为 `provider_secret.rs`；store 换 `DbCredStore`。
- `tests/config_dir_isolation_test.rs`：`apply_dev_subdir` 删后失效 → 删除该文件或改断言 release/debug 同路径。
- `tests/ipc_settings.rs` / `tests/ipc_translate.rs` / db schema 测试：store 换 `DbCredStore`，grep 改所有断言 `secret_presence` 的用例为 `provider_secret`。
- credential.rs 内联：保留 `MockCredStore` 证明 trait 抽象；新增 `DbCredStore` 往返 / 覆盖 / 删除测试（in-memory 加密库 + 固定 KEY，参考 secret_presence.rs 的 `open_tmp_db`）。

## 八、流程与验证

- **走 feature-dev 七阶段**（单功能改造、一个会话内可交互完成）。Phase 5 用 `coder`（Opus 4.8）严格 TDD；Phase 6 用 `tester`（Sonnet）动态证伪 + `code-reviewer` 规范审查。主 agent 只协调 + 只读核对。
- **验证命令**（证据先于断言）：`cargo test` + `cargo test --release`（确认去 cfg gate 后 release 测试编译通过——这正是现状特意 gate 的痛点，统一后消失）+ `cargo clippy`，全绿。
- **手动验证**：删 `<config>/master.key` + 库，首启自动生成 → 能写读剪贴板 → 设置页填翻译 key 后翻译可用 → 重启后 key 仍在 → 全程无任何密码弹窗。异机模拟：拷 `master.key` + 库到不同 machine_id 上下文，断言开库失败并触发备份重建。

## 九、风险点

1. 机器标识在 VM 克隆 / `/etc/machine-id` 被清 / ioreg 格式变动时变化 → 触发重置。缓解：降级回退 + release note 告知。
2. 一次性重置丢历史 + secret 需重填 → release note 显式告知（预发布可接受）。
3. 子进程读机器标识在受限 / 沙箱环境失败 → 必须 Result + 回退，不 panic。
4. 移除 keyring 前全仓 grep 确认无残留引用（含 tests）。
