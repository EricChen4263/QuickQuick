//! 密钥层抽象模块：`KeyProvider` trait + 本地文件密钥库 `LocalKeyProvider`
//!
//! 设计要点（对齐 docs/design/local-keystore-no-keychain.md）：
//! - `KeyProvider` trait 是开库唯一依赖的抽象：调用方只需通过接口取 256-bit 密钥。
//! - `LocalKeyProvider` 三平台统一实现：SQLCipher 主密钥落本地 `master.key` 文件（0600），
//!   再用「机器绑定」KEK 二次加密，文件单拷到异机解不开（machine_id 不同 → GCM 验签失败）。
//! - 不再依赖 OS 钥匙串：无 Apple Developer ID 时钥匙串 ACL 反复失效、反复弹密码，
//!   本方案以「永不弹密码、永远能开库」为硬目标。
//!
//! ## 安全声明（诚实交代威胁模型）
//! 相比 Keychain 方案，本地密钥库放弃了「OS 级访问控制 + 锁屏后台保护」——任何能读到
//! 当前登录用户家目录的进程/人都能解开剪贴板库。**保住的是**：密钥永不明文落盘 + 密钥文件
//! 单独被拷到别的机器也解不开。**防的是**设备丢失、目录被单独拷走、云同步盘/Time Machine
//! 误带走、二手机未擦盘；**不防**本机其他进程、完整家目录被读。
//! 注：钥匙串那层 ACL 在本项目无稳定签名下本就形同虚设，这层「损失」在真实威胁模型下有限。

use std::path::{Path, PathBuf};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use thiserror::Error;
use zeroize::Zeroizing;

/// 密钥文件名（落在 app_config_dir 根，非 dev 子目录）
const KEY_FILE_NAME: &str = "master.key";

/// 文件 magic 头（"QQMK" = QuickQuick Master Key，Little-Endian 排布）
const MAGIC: [u8; 4] = [0x51, 0x51, 0x4D, 0x4B];

/// 密钥文件格式版本
const FORMAT_VERSION: u8 = 0x01;

/// Argon2id KEK 派生盐长度（字节）
const SALT_LEN: usize = 32;

/// AES-GCM nonce 长度（字节）
const NONCE_LEN: usize = 12;

/// 主密钥长度（字节，AES-256 raw key）
const KEY_LEN: usize = 32;

/// 头部固定长度：magic(4) + version(1) + salt(32) + nonce(12)
const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN;

/// Argon2id 内存开销（KiB）：64 MB，符合 OWASP 2023 推荐下限（与 portable.rs 一致）
const ARGON2_MEM_KIB: u32 = 65536;

/// Argon2id 迭代次数：3 次
const ARGON2_ITERATIONS: u32 = 3;

/// Argon2id 并行度：4 线程
const ARGON2_PARALLELISM: u32 = 4;

/// 机器标识不可用时的降级回退常量盐。
///
/// 为什么是固定常量：当三平台 machine_id 子进程/文件全部取不到时，机器绑定增益失效，
/// 退化为「纯 0600 文件」安全级——这是为保住「永不弹密码、永远能开库」硬目标刻意做的降级。
/// 此值仅作 KEK 派生的 passphrase 占位，绝非密钥本身（真正机密是文件内随机盐 + 随机主密钥）。
const FALLBACK_MACHINE_ID: &[u8] = b"quickquick-local-keystore-fallback-machine-id-v1";

/// KeyProvider 操作错误枚举
#[derive(Debug, Error)]
pub enum KeyError {
    /// 文件 I/O 失败（读写密钥文件、设权限等）
    #[error("密钥文件 I/O 失败：{0}")]
    Io(String),

    /// 随机密钥生成/加密失败（理论上不应发生）
    #[error("随机密钥生成失败")]
    Generation,

    /// 密钥文件格式非法（magic/version 不符或长度不足）
    #[error("密钥文件格式无效")]
    Format,

    /// KEK 解密主密钥失败——文件被拷到异机（machine_id 不同）或文件损坏/被篡改。
    ///
    /// lib.rs 据此判定「需一次性重置」：备份旧 master.key 与旧库后重建。
    #[error("密钥解密失败（文件损坏或来自其他机器）")]
    Decrypt,

    /// Argon2 KEK 派生失败（参数构建或 hash 失败）
    #[error("密钥派生失败")]
    Kdf,
}

/// 密钥层抽象接口——开库唯一依赖的密钥获取方式。
///
/// # Contract
/// - 首次调用：生成随机 32 字节主密钥，机器绑定加密后持久化，并返回该密钥。
/// - 后续调用：从密钥文件解密并返回同一密钥（幂等）。
pub trait KeyProvider {
    /// 获取或生成 256-bit（32 字节）的 SQLCipher 主密钥。
    ///
    /// # Errors
    /// - `KeyError::Io`：密钥文件读写失败
    /// - `KeyError::Generation`：随机密钥生成失败
    /// - `KeyError::Format`：密钥文件格式非法
    /// - `KeyError::Decrypt`：机器绑定解密失败（异机/损坏）
    /// - `KeyError::Kdf`：KEK 派生失败
    fn get_or_create_key(&self) -> Result<[u8; 32], KeyError>;
}

/// 生成 32 字节随机密钥。
///
/// 实现：uuid v4 内部调用 getrandom/OS CSPRNG，两次各生成 16 字节拼接为 32 字节。
/// Cargo.toml 已有 uuid = {features=["v4"]}，熵来源为 OS CSPRNG（不使用时间种子）。
pub fn generate_random_key() -> [u8; 32] {
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(a.as_bytes());
    key[16..].copy_from_slice(b.as_bytes());
    key
}

/// 读取本机机器标识（三平台分支），用作 KEK 派生的 passphrase。
///
/// - macOS：`ioreg -rd1 -c IOPlatformExpertDevice` 解析 `IOPlatformUUID`
/// - Linux：`/etc/machine-id`（回退 `/var/lib/dbus/machine-id`）
/// - Windows：`reg query HKLM\SOFTWARE\Microsoft\Cryptography /v MachineGuid` 解析 `MachineGuid`
///
/// # 降级回退（硬目标）
/// 任一平台取不到机器标识（子进程失败、文件缺失、解析空）→ 返回 [`FALLBACK_MACHINE_ID`]，
/// 退化为纯 0600 安全级而非 panic。子进程调用全程 `Result` 处理，绝不 panic。
fn machine_id() -> Vec<u8> {
    machine_id_with_reader(read_platform_machine_id)
}

/// 回退判定纯逻辑（reader 可注入，便于单测直击「读取失败→回退」分支）。
///
/// 真实入口 `machine_id()` 传 `read_platform_machine_id`；测试注入返回 None/空 Vec 的
/// reader 验证回退判别力——现有 `with_machine_id` 注入绕过真实读取，macOS ioreg 又总成功，
/// 故回退分支本身缺判别力测试，此处补上。
fn machine_id_with_reader(reader: impl Fn() -> Option<Vec<u8>>) -> Vec<u8> {
    reader()
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| FALLBACK_MACHINE_ID.to_vec())
}

/// 平台相关的机器标识读取（macOS）：ioreg 子进程解析 IOPlatformUUID。
#[cfg(target_os = "macos")]
fn read_platform_machine_id() -> Option<Vec<u8>> {
    let output = std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // 行形如：  "IOPlatformUUID" = "XXXXXXXX-XXXX-..."
    let line = text.lines().find(|l| l.contains("IOPlatformUUID"))?;
    extract_quoted_after_eq(line).map(String::into_bytes)
}

/// 平台相关的机器标识读取（Linux）：读 machine-id 文件，回退 dbus 路径。
#[cfg(target_os = "linux")]
fn read_platform_machine_id() -> Option<Vec<u8>> {
    let read_trimmed = |path: &str| {
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    read_trimmed("/etc/machine-id")
        .or_else(|| read_trimmed("/var/lib/dbus/machine-id"))
        .map(String::into_bytes)
}

/// 平台相关的机器标识读取（Windows）：reg query 解析 MachineGuid（免 winreg 依赖）。
#[cfg(target_os = "windows")]
fn read_platform_machine_id() -> Option<Vec<u8>> {
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Cryptography",
            "/v",
            "MachineGuid",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // 行形如：    MachineGuid    REG_SZ    xxxxxxxx-xxxx-...
    let line = text.lines().find(|l| l.contains("MachineGuid"))?;
    line.split_whitespace()
        .last()
        .map(|s| s.as_bytes().to_vec())
}

/// 其余平台（无已知机器标识来源）：直接走回退常量盐。
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn read_platform_machine_id() -> Option<Vec<u8>> {
    None
}

/// 提取形如 `"Key" = "Value"` 行中等号后引号内的值（macOS ioreg 解析辅助）。
#[cfg(target_os = "macos")]
fn extract_quoted_after_eq(line: &str) -> Option<String> {
    let after_eq = line.split('=').nth(1)?;
    let start = after_eq.find('"')? + 1;
    let rest = &after_eq[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// 本地文件密钥库：三平台统一实现，机器绑定 KEK + 0600 文件。
///
/// 密钥文件落 `config_dir/master.key`，格式（参照 portable.rs 头部思路）：
/// `magic(4) + version(1) + salt(32) + nonce(12) + aes-256-gcm(master_key)(48)`。
/// KEK = `Argon2id(passphrase = machine_id, salt = 文件内随机盐)`。
pub struct LocalKeyProvider {
    /// 密钥文件完整路径（config_dir/master.key）
    key_path: PathBuf,
    /// KEK 派生用机器标识（生产由 `machine_id()` 填充；测试可注入以模拟异机）
    machine_id: Vec<u8>,
}

impl LocalKeyProvider {
    /// 创建 LocalKeyProvider：密钥文件落在 `config_dir/master.key`，机器标识由平台函数填充。
    pub fn new(config_dir: &Path) -> Self {
        Self {
            key_path: config_dir.join(KEY_FILE_NAME),
            machine_id: machine_id(),
        }
    }

    /// 注入机器标识构造（测试专用）：用于模拟「同机复算」与「异机解密失败」。
    ///
    /// 真实入口走 `new`（machine_id 由平台函数填充）；本构造器让单测可控注入不同 id，
    /// 验证「文件单拷到异机解不开」与「降级回退仍能开库」两条核心安全路径。
    pub fn with_machine_id(config_dir: &Path, machine_id: &[u8]) -> Self {
        Self {
            key_path: config_dir.join(KEY_FILE_NAME),
            machine_id: machine_id.to_vec(),
        }
    }

    /// 从密钥文件解密主密钥，或在文件不存在时生成随机主密钥并机器绑定落盘（幂等）。
    fn load_or_generate(&self) -> Result<[u8; 32], KeyError> {
        match std::fs::read(&self.key_path) {
            Ok(blob) => self.decrypt_master_key(&blob),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Zeroizing 包裹新生成的主密钥：写盘后中间变量 drop 时清零，不在内存残留。
                let key = Zeroizing::new(generate_random_key());
                self.write_key_file(&key)?;
                Ok(*key)
            }
            Err(e) => Err(KeyError::Io(e.to_string())),
        }
    }

    /// 用机器绑定 KEK 加密主密钥并以 0600 落盘（首启路径）。
    fn write_key_file(&self, master_key: &[u8; KEY_LEN]) -> Result<(), KeyError> {
        let mut rng = rand::thread_rng();
        let mut salt = [0u8; SALT_LEN];
        rng.fill_bytes(&mut salt);
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rng.fill_bytes(&mut nonce_bytes);

        let kek = self.derive_kek(&salt)?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(kek.as_ref()));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), master_key.as_slice())
            .map_err(|_| KeyError::Generation)?;

        let mut blob = Vec::with_capacity(HEADER_LEN + ciphertext.len());
        blob.extend_from_slice(&MAGIC);
        blob.push(FORMAT_VERSION);
        blob.extend_from_slice(&salt);
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);

        std::fs::write(&self.key_path, &blob).map_err(|e| KeyError::Io(e.to_string()))?;
        self.harden_permissions()
    }

    /// 解析密钥文件，复算 KEK 解密出主密钥（后续启动路径）。
    fn decrypt_master_key(&self, blob: &[u8]) -> Result<[u8; KEY_LEN], KeyError> {
        if blob.len() < HEADER_LEN + 16 || blob[..4] != MAGIC || blob[4] != FORMAT_VERSION {
            return Err(KeyError::Format);
        }
        let salt: &[u8; SALT_LEN] = blob[5..5 + SALT_LEN]
            .try_into()
            .map_err(|_| KeyError::Format)?;
        let nonce_start = 5 + SALT_LEN;
        let nonce_bytes = &blob[nonce_start..nonce_start + NONCE_LEN];
        let ciphertext = &blob[HEADER_LEN..];

        let kek = self.derive_kek(salt)?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(kek.as_ref()));
        // Zeroizing 包裹解密出的明文主密钥：try_into 后中间 Vec drop 时清零，不在内存残留。
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
                .map_err(|_| KeyError::Decrypt)?,
        );

        plaintext
            .as_slice()
            .try_into()
            .map_err(|_| KeyError::Decrypt)
    }

    /// 从机器标识 + 文件内随机盐派生 KEK（Argon2id），返回 Zeroizing 包装的 32 字节。
    fn derive_kek(&self, salt: &[u8; SALT_LEN]) -> Result<Zeroizing<[u8; KEY_LEN]>, KeyError> {
        let params = Params::new(
            ARGON2_MEM_KIB,
            ARGON2_ITERATIONS,
            ARGON2_PARALLELISM,
            Some(KEY_LEN),
        )
        .map_err(|_| KeyError::Kdf)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut kek = Zeroizing::new([0u8; KEY_LEN]);
        argon2
            .hash_password_into(&self.machine_id, salt, kek.as_mut())
            .map_err(|_| KeyError::Kdf)?;
        Ok(kek)
    }

    /// unix 下把密钥文件权限收紧为 0600（仅属主可读写）；非 unix 平台跳过。
    fn harden_permissions(&self) -> Result<(), KeyError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.key_path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| KeyError::Io(e.to_string()))?;
        }
        Ok(())
    }
}

impl KeyProvider for LocalKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], KeyError> {
        self.load_or_generate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// 验证 generate_random_key 每次返回 32 字节
    #[test]
    fn generate_random_key_returns_32_bytes() {
        let key = generate_random_key();
        assert_eq!(key.len(), 32, "generate_random_key 应返回恰好 32 字节");
    }

    /// 两次独立生成的随机密钥不应相等（随机源熵正常，非恒真断言）
    #[test]
    fn generate_random_key_is_not_constant() {
        let a = generate_random_key();
        let b = generate_random_key();
        assert_ne!(a, b, "两次独立生成的随机密钥不应相等");
    }

    /// 首次调用应生成密钥并把 master.key 文件落盘
    #[test]
    fn first_call_creates_master_key_file() {
        let dir = tempdir().unwrap();
        let provider = LocalKeyProvider::new(dir.path());

        let key = provider.get_or_create_key().unwrap();

        assert_eq!(key.len(), 32, "首次生成的密钥应为 32 字节");
        assert!(
            dir.path().join("master.key").exists(),
            "首次调用后 master.key 应已落盘"
        );
    }

    /// 同一实例二次调用应幂等返回同一密钥（不重新生成）
    #[test]
    fn same_instance_is_idempotent() {
        let dir = tempdir().unwrap();
        let provider = LocalKeyProvider::new(dir.path());

        let first = provider.get_or_create_key().unwrap();
        let second = provider.get_or_create_key().unwrap();

        assert_eq!(first, second, "同一实例二次调用应返回同一密钥");
    }

    /// 新实例从已有文件读出与首个实例相同的密钥（跨实例持久化 + 同机复算 KEK）
    #[test]
    fn new_instance_reads_same_key_with_same_machine_id() {
        let dir = tempdir().unwrap();
        let id = b"machine-A";

        let first = LocalKeyProvider::with_machine_id(dir.path(), id)
            .get_or_create_key()
            .unwrap();
        let second = LocalKeyProvider::with_machine_id(dir.path(), id)
            .get_or_create_key()
            .unwrap();

        assert_eq!(first, second, "同机新实例应从文件解出同一密钥");
    }

    /// 异机模拟：用 machine-A 写盘后，machine-B 复算 KEK 解密应失败（文件单拷异机解不开）
    #[test]
    fn different_machine_id_fails_to_decrypt() {
        let dir = tempdir().unwrap();

        LocalKeyProvider::with_machine_id(dir.path(), b"machine-A")
            .get_or_create_key()
            .expect("machine-A 首启应成功");

        let result =
            LocalKeyProvider::with_machine_id(dir.path(), b"machine-B").get_or_create_key();

        assert!(
            matches!(result, Err(KeyError::Decrypt)),
            "异机 machine_id 解密应返回 Decrypt，实际：{result:?}"
        );
    }

    /// 降级回退：machine_id 不可用时退化为固定回退盐，仍能首启 + 跨实例开库
    #[test]
    fn fallback_machine_id_still_opens_keystore() {
        let dir = tempdir().unwrap();

        let first = LocalKeyProvider::with_machine_id(dir.path(), FALLBACK_MACHINE_ID)
            .get_or_create_key()
            .expect("回退盐首启应成功（永不弹密码硬目标）");
        let second = LocalKeyProvider::with_machine_id(dir.path(), FALLBACK_MACHINE_ID)
            .get_or_create_key()
            .expect("回退盐跨实例应能复算解密");

        assert_eq!(first, second, "回退盐下应能稳定开库读出同一密钥");
    }

    /// 损坏文件（截断/乱码）应报错而非 panic
    #[test]
    fn corrupt_file_reports_error_not_panic() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("master.key"), b"not a valid key file").unwrap();
        let provider = LocalKeyProvider::new(dir.path());

        let result = provider.get_or_create_key();

        assert!(
            matches!(result, Err(KeyError::Format) | Err(KeyError::Decrypt)),
            "损坏文件应报 Format/Decrypt 而非 panic，实际：{result:?}"
        );
    }

    /// unix 下密钥文件权限应为 0600（仅属主可读写）
    #[cfg(unix)]
    #[test]
    fn master_key_file_has_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let provider = LocalKeyProvider::new(dir.path());

        provider.get_or_create_key().unwrap();

        let mode = std::fs::metadata(dir.path().join("master.key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "master.key 权限应为 0600");
    }

    /// 真实 machine_id() 永不 panic 且非空（回退保证「永远能开库」）
    #[test]
    fn machine_id_never_empty() {
        let id = machine_id();
        assert!(
            !id.is_empty(),
            "machine_id 应永不为空（取不到时回退常量盐）"
        );
    }

    /// 判别力测试：reader 返回 None（模拟平台读取失败）→ 必须回退到 FALLBACK_MACHINE_ID。
    ///
    /// 现有测试用 `with_machine_id` 注入绕过真实 `read_platform_machine_id`，
    /// macOS ioreg 又总成功，故「回退逻辑被改成 panic/Err」的变异抓不到。
    /// 本测试用可注入 reader 直击回退分支：若回退被改坏，断言立即失败。
    #[test]
    fn machine_id_falls_back_to_constant_when_reader_returns_none() {
        let id = machine_id_with_reader(|| None);
        assert_eq!(
            id, FALLBACK_MACHINE_ID,
            "reader 返回 None 时应回退到 FALLBACK_MACHINE_ID"
        );
    }

    /// 判别力测试：reader 返回空 Vec（解析到空串）→ 同样视为不可用、回退常量盐。
    #[test]
    fn machine_id_falls_back_to_constant_when_reader_returns_empty() {
        let id = machine_id_with_reader(|| Some(Vec::new()));
        assert_eq!(
            id, FALLBACK_MACHINE_ID,
            "reader 返回空 Vec 时应回退到 FALLBACK_MACHINE_ID"
        );
    }

    /// 判别力测试：reader 取到机器标识时应原样采用，不走回退（区分两条分支）。
    #[test]
    fn machine_id_uses_reader_value_when_available() {
        let id = machine_id_with_reader(|| Some(b"real-machine-id".to_vec()));
        assert_eq!(id, b"real-machine-id", "reader 取到值时应原样采用、不回退");
    }

    /// 判别力测试（端到端硬目标）：平台读取失败（reader=None）回退后，
    /// LocalKeyProvider 仍能首启 + 跨实例开库（get_or_create_key 成功）——「永不弹密码、永远能开库」。
    #[test]
    fn local_provider_opens_with_fallback_when_reader_returns_none() {
        let dir = tempdir().unwrap();
        let fallback_id = machine_id_with_reader(|| None);

        let first = LocalKeyProvider::with_machine_id(dir.path(), &fallback_id)
            .get_or_create_key()
            .expect("平台读取失败回退后首启仍应成功");
        let second = LocalKeyProvider::with_machine_id(dir.path(), &fallback_id)
            .get_or_create_key()
            .expect("回退后跨实例仍应能复算解密");

        assert_eq!(first, second, "回退后应能稳定开库读出同一密钥");
    }
}
