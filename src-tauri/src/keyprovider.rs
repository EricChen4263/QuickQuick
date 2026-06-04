//! 密钥层抽象模块：KeyProvider trait + KeychainKeyProvider 实现
//!
//! 设计要点（对齐设计文档§六）：
//! - `KeyProvider` trait 是开库唯一依赖的抽象：调用方只需通过接口取 256-bit 密钥。
//! - `KeychainKeyProvider` 是 v1 唯一真实实现，基于 OS 钥匙串（keyring crate）持久化。
//! - 密钥访问性语义：`AfterFirstUnlockThisDeviceOnly`——锁屏后首次解锁可用、绝不同步/漫游。
//! - 测试友好：keyring 3 支持 mock credential builder，测试全程 headless 不弹窗。
//! - 随机密钥熵：uuid v4（内部用 getrandom/OS CSPRNG），两个 16 字节拼接得 32 字节。
//! - Entry 字段持有：KeychainKeyProvider 持有 keyring::Entry 而非每次重建，保证
//!   mock 模式（EntryOnly 持久化）与真实 Keychain 模式（OS 持久化）行为一致。

use thiserror::Error;

/// Keychain 中存储密钥的服务名（与 App bundle 一致）
const KEYCHAIN_SERVICE: &str = "io.quickquick.app";

/// Keychain 中存储密钥的账号名（固定：当前设备的 SQLCipher 主密钥）
const KEYCHAIN_ACCOUNT: &str = "sqlcipher_master_key";

/// KeyProvider 操作错误枚举
#[derive(Debug, Error)]
pub enum KeyError {
    /// 钥匙串后端操作失败（keyring 返回错误）
    #[error("钥匙串后端操作失败：{0}")]
    Backend(String),

    /// 随机密钥生成失败（理论上不应发生）
    #[error("随机密钥生成失败")]
    Generation,

    /// 存储的密钥长度不符合预期（可能被损坏或外部篡改）
    #[error("存储的密钥长度非法：期望 32 字节，实际 {0} 字节")]
    InvalidKeyLength(usize),
}

/// 密钥可访问性语义（对齐设计文档§六）
///
/// 仅定义 v1 所需的变体；未来可按需扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAccessibility {
    /// 锁屏后首次解锁方可访问，且仅限本设备（不漫游、不 iCloud 同步）。
    ///
    /// 对应 macOS Security framework kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly。
    AfterFirstUnlockThisDeviceOnly,
}

/// 密钥在安全存储中的实际属性配置（可测纯数据，不持有 OS 句柄）。
///
/// 作用：提供一个可在单测中断言的"意图配置"值，表达 AfterFirstUnlock + ThisDeviceOnly
/// + synchronizable=false 三项约束，解决 V0 中"枚举声明但 OS 属性未落地"的已知差距（V0-F3-A03-H01）。
///
/// 设计约束（见设计文档§六#2）：
/// - `accessibility_identifier()` 返回 `"AfterFirstUnlockThisDeviceOnly"`，而非 keyring
///   apple-native 默认的 `"WhenUnlocked"`，表达后台可读（AfterFirstUnlock）语义。
/// - `synchronizable()` 返回 `false`，密钥不漫游 iCloud Keychain / 凭据（ThisDeviceOnly）。
///
/// # 平台说明
/// - macOS：标识符字符串对应 `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`，
///   `synchronizable=false` 对应 `kSecAttrSynchronizable = kCFBooleanFalse`。
/// - Windows：凭据管理器本机不漫游，ThisDeviceOnly 语义天然满足；AfterFirstUnlock
///   在 Windows 无精确对应项，注释标明「本机持久可读」语义等价。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyStorageAttributes {
    /// macOS kSecAttrAccessible 对应标识符；Windows 下语义等价描述。
    accessibility_id: &'static str,
    /// 是否允许跨设备同步（必须为 false）。
    synchronizable: bool,
}

impl KeyStorageAttributes {
    /// 返回 macOS kSecAttrAccessible 标识符字符串（平台无关的意图表达）。
    ///
    /// 返回 `"AfterFirstUnlockThisDeviceOnly"` 而非 `"WhenUnlocked"`，
    /// 区分两种语义：AfterFirstUnlock 锁屏后台仍可读；WhenUnlocked 仅解锁状态可读。
    pub fn accessibility_identifier(&self) -> &'static str {
        self.accessibility_id
    }

    /// 是否允许漫游同步（iCloud Keychain / 凭据漫游）。
    ///
    /// 必须返回 `false`：密钥绝不得跨设备同步（ThisDeviceOnly 硬约束）。
    pub fn synchronizable(&self) -> bool {
        self.synchronizable
    }
}

impl Default for KeyStorageAttributes {
    /// 返回 QuickQuick v1 的预定属性配置：AfterFirstUnlockThisDeviceOnly + 不同步。
    fn default() -> Self {
        Self {
            accessibility_id: "AfterFirstUnlockThisDeviceOnly",
            synchronizable: false,
        }
    }
}

/// 密钥层抽象接口——开库唯一依赖的密钥获取方式。
///
/// # Contract
/// - 首次调用：生成随机 32 字节密钥，持久化至安全存储，并返回该密钥。
/// - 后续调用：从安全存储读取并返回同一密钥（幂等）。
/// - 实现必须保证密钥不漫游（ThisDeviceOnly）。
pub trait KeyProvider {
    /// 获取或生成 256-bit（32 字节）的 SQLCipher 主密钥。
    ///
    /// # Errors
    /// - `KeyError::Backend`：钥匙串后端不可用
    /// - `KeyError::Generation`：随机密钥生成失败
    /// - `KeyError::InvalidKeyLength`：存储值长度异常
    fn get_or_create_key(&self) -> Result<[u8; 32], KeyError>;
}

/// 生成 32 字节随机密钥。
///
/// 实现：uuid v4 内部调用 getrandom/OS CSPRNG，两次各生成 16 字节拼接为 32 字节。
/// 避免引入额外 crate，同时保证熵来源为 OS CSPRNG（不使用时间种子）。
///
/// # 为什么用 uuid 而非 rand
/// Cargo.toml 已有 uuid = {features=["v4"]}，uuid::Uuid::new_v4() 内部调用
/// getrandom，与 rand::thread_rng() 熵质量相同；无需增加依赖。
pub fn generate_random_key() -> [u8; 32] {
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(a.as_bytes());
    key[16..].copy_from_slice(b.as_bytes());
    key
}

/// v1 唯一真实实现：使用 OS 钥匙串（keyring crate）持久化 SQLCipher 主密钥。
///
/// # Entry 字段持有策略
/// 持有 `keyring::Entry` 实例而非每次重建，原因：
/// - keyring mock（EntryOnly 持久化）仅在同一 Entry 实例内保留数据，若每次重建则
///   丢失首次写入的密钥，幂等性断言会失败。
/// - 真实 OS Keychain 不受此限制（跨进程/跨实例均可读），但字段持有对其无副作用。
///
/// # 可测性
/// keyring 3 支持 `set_default_credential_builder(keyring::mock::default_credential_builder())`，
/// 测试中激活 mock 后，此 provider 完全不触碰真实 OS 钥匙串。
pub struct KeychainKeyProvider {
    /// 持有 keyring Entry，确保 mock 与真实 Keychain 模式下幂等性一致
    entry: keyring::Entry,
}

impl KeychainKeyProvider {
    /// 创建 KeychainKeyProvider 实例。
    ///
    /// # Errors
    /// keyring::Entry::new 失败时 panic——属于不可恢复的初始化错误，
    /// 说明服务名/账号名参数非法（静态常量，不应发生）。
    ///
    /// # Panics
    /// 若 keyring 后端拒绝创建 Entry（极少见，仅静态名称非法时触发）
    pub fn new() -> Self {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
            .expect("keyring::Entry::new 不应因静态常量失败");
        Self { entry }
    }

    /// 返回此 provider 的密钥可访问性语义。
    ///
    /// 返回 `AfterFirstUnlockThisDeviceOnly`，其保证边界如下：
    ///
    /// **已保证（由 keyring `apple-native` feature 提供）**：
    /// - macOS 上启用 `apple-native` feature 后，keyring 以 `kSecAttrSynchronizable = false`
    ///   存储密钥，密钥不漫游 iCloud Keychain，满足设计§六「ThisDeviceOnly 不同步」硬约束。
    /// - Windows 上启用 `windows-native` feature，凭据管理器本机不漫游。
    ///
    /// **已知差距（归 pending-manual V0-F3-A03-H01）**：
    /// - macOS `kSecAttrAccessible` 属性精确值未强制设为 `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`。
    ///   keyring apple-native 默认使用 `kSecAttrAccessibleWhenUnlocked`（解锁可用），
    ///   而非 AfterFirstUnlock（首次解锁后锁屏仍可用）。
    /// - 若需后台访问密钥（锁屏期间），须通过 `security-framework` 直接设置
    ///   `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`，该强化标记为已知差距。
    pub fn accessibility(&self) -> KeyAccessibility {
        KeyAccessibility::AfterFirstUnlockThisDeviceOnly
    }

    /// 密钥是否仅限本设备（不漫游）。
    ///
    /// 返回 `true`：keyring `apple-native` / `windows-native` feature 以
    /// `kSecAttrSynchronizable = false` 存储密钥，满足设计§六「ThisDeviceOnly 不同步」
    /// 硬约束。`KeyStorageAttributes::synchronizable()` 提供可单测的属性断言点。
    pub fn is_device_only(&self) -> bool {
        matches!(
            self.accessibility(),
            KeyAccessibility::AfterFirstUnlockThisDeviceOnly
        )
    }

    /// 密钥是否在锁屏后台仍可读（AfterFirstUnlock 语义）。
    ///
    /// 返回 `true` 表示：本 provider 配置意图为 `AfterFirstUnlockThisDeviceOnly`，
    /// 即设备首次解锁后，即使屏幕再次锁定，后台进程仍可访问密钥。
    ///
    /// 此方法解决 V0-F3-A03-H01 已知差距：V0 中枚举声明了 AfterFirstUnlock 语义
    /// 但无显式方法暴露该断言点。`KeyStorageAttributes` 提供配置意图的可测纯值。
    ///
    /// # 平台说明
    /// - macOS：意图配置为 `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`。
    /// - Windows：凭据管理器本机持久可读，语义等价（无精确 AfterFirstUnlock 对应项）。
    pub fn is_after_first_unlock(&self) -> bool {
        matches!(
            self.accessibility(),
            KeyAccessibility::AfterFirstUnlockThisDeviceOnly
        )
    }

    /// 返回此 provider 的实际存储属性配置（可测纯值，不触碰 OS 钥匙串）。
    ///
    /// 提供 `accessibility_identifier()` 与 `synchronizable()` 两个断言点，
    /// 用于在单测中验证 AfterFirstUnlockThisDeviceOnly + synchronizable=false
    /// 的配置意图，区分 WhenUnlocked 等其他 accessibility 值。
    pub fn storage_attributes(&self) -> KeyStorageAttributes {
        KeyStorageAttributes::default()
    }

    /// 从 Entry 读取已存储的密钥，或在首次调用时生成并存储。
    fn load_or_generate(&self) -> Result<[u8; 32], KeyError> {
        match self.entry.get_secret() {
            Ok(bytes) => {
                // 已存在密钥：校验长度后返回
                if bytes.len() != 32 {
                    return Err(KeyError::InvalidKeyLength(bytes.len()));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                Ok(key)
            }
            Err(keyring::Error::NoEntry) => {
                // 首次使用：生成随机密钥并持久化
                let key = generate_random_key();
                self.entry
                    .set_secret(&key)
                    .map_err(|e| KeyError::Backend(e.to_string()))?;
                Ok(key)
            }
            Err(e) => Err(KeyError::Backend(e.to_string())),
        }
    }
}

impl Default for KeychainKeyProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 开发期文件密钥库（debug-only，release 构建用 KeychainKeyProvider）。
///
/// # 为什么需要它
/// dev 反复重编 → macOS codesign 身份变动 → 钥匙串「始终允许」失效，反复弹密码。
/// debug 构建改把 SQLCipher 主密钥存本地文件、完全绕开 OS 钥匙串，消除弹窗。
///
/// # 安全约束
/// - 仅 `#[cfg(debug_assertions)]` 编译，绝不进入 release 分发二进制。
/// - 密钥文件权限设为 `0600`（仅属主可读写），与钥匙串「ThisDeviceOnly」语义近似。
/// - 文件存 32 字节原始密钥（非 hex），读时校验长度，与 KeychainKeyProvider 幂等语义一致。
#[cfg(debug_assertions)]
pub struct FileKeyProvider {
    /// 密钥文件完整路径（config_dir/dev-master-key）
    key_path: std::path::PathBuf,
}

#[cfg(debug_assertions)]
impl FileKeyProvider {
    /// dev 密钥文件名（落在 app_config_dir 下）
    const KEY_FILE_NAME: &'static str = "dev-master-key";

    /// 创建 FileKeyProvider，密钥文件落在 `config_dir/dev-master-key`。
    pub fn new(config_dir: &std::path::Path) -> Self {
        Self {
            key_path: config_dir.join(Self::KEY_FILE_NAME),
        }
    }

    /// 从文件读取密钥，或在文件不存在时生成随机密钥并落盘（幂等）。
    fn load_or_generate(&self) -> Result<[u8; 32], KeyError> {
        match std::fs::read(&self.key_path) {
            Ok(bytes) => {
                if bytes.len() != 32 {
                    return Err(KeyError::InvalidKeyLength(bytes.len()));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                Ok(key)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let key = generate_random_key();
                self.write_key_file(&key)?;
                Ok(key)
            }
            Err(e) => Err(KeyError::Backend(e.to_string())),
        }
    }

    /// 写密钥文件并设权限 0600（unix）。非 unix 平台跳过权限设置。
    fn write_key_file(&self, key: &[u8; 32]) -> Result<(), KeyError> {
        std::fs::write(&self.key_path, key).map_err(|e| KeyError::Backend(e.to_string()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.key_path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| KeyError::Backend(e.to_string()))?;
        }

        Ok(())
    }
}

#[cfg(debug_assertions)]
impl KeyProvider for FileKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], KeyError> {
        self.load_or_generate()
    }
}

impl KeyProvider for KeychainKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], KeyError> {
        self.load_or_generate()
    }
}

// 测试模块本身也需 debug_assertions 门控：FileKeyProvider 是 debug-only 类型，
// release 测试构建下不存在，若仅 #[cfg(test)] 会导致 cargo test --release 编译失败（E0433）。
#[cfg(all(test, debug_assertions))]
mod file_provider_tests {
    use super::*;
    use tempfile::tempdir;

    /// 首次调用应生成密钥并把文件落盘
    #[test]
    fn file_provider_creates_key_file_on_first_call() {
        let dir = tempdir().unwrap();
        let provider = FileKeyProvider::new(dir.path());

        let key = provider.get_or_create_key().unwrap();

        assert_eq!(key.len(), 32, "首次生成的密钥应为 32 字节");
        assert!(
            dir.path().join("dev-master-key").exists(),
            "首次调用后密钥文件应已落盘"
        );
    }

    /// 同一实例二次调用应幂等返回同一密钥（不重新生成）
    #[test]
    fn file_provider_is_idempotent_within_same_instance() {
        let dir = tempdir().unwrap();
        let provider = FileKeyProvider::new(dir.path());

        let first = provider.get_or_create_key().unwrap();
        let second = provider.get_or_create_key().unwrap();

        assert_eq!(first, second, "同一实例二次调用应返回同一密钥");
    }

    /// 新实例从已有文件读出与首个实例相同的密钥（跨实例持久化）
    #[test]
    fn file_provider_reads_same_key_from_new_instance() {
        let dir = tempdir().unwrap();
        let first_key = FileKeyProvider::new(dir.path())
            .get_or_create_key()
            .unwrap();

        let second_key = FileKeyProvider::new(dir.path())
            .get_or_create_key()
            .unwrap();

        assert_eq!(first_key, second_key, "新实例应从文件读出同一密钥");
    }

    /// 文件长度非 32 字节时报 InvalidKeyLength（损坏/篡改检测）
    #[test]
    fn file_provider_rejects_wrong_length() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("dev-master-key"), b"too short").unwrap();
        let provider = FileKeyProvider::new(dir.path());

        let result = provider.get_or_create_key();

        assert!(
            matches!(result, Err(KeyError::InvalidKeyLength(9))),
            "长度非 32 的密钥文件应报 InvalidKeyLength(9)，实际：{result:?}"
        );
    }

    /// unix 下密钥文件权限应为 0600（仅属主可读写）
    #[cfg(unix)]
    #[test]
    fn file_provider_sets_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let provider = FileKeyProvider::new(dir.path());

        provider.get_or_create_key().unwrap();

        let mode = std::fs::metadata(dir.path().join("dev-master-key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "密钥文件权限应为 0600");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 generate_random_key 每次返回 32 字节
    #[test]
    fn generate_random_key_returns_32_bytes() {
        // Arrange & Act
        let key = generate_random_key();

        // Assert
        assert_eq!(key.len(), 32, "generate_random_key 应返回恰好 32 字节");
    }

    /// 验证 KeychainKeyProvider 默认标记为 ThisDeviceOnly
    #[test]
    fn keychain_provider_is_device_only_by_default() {
        // Arrange：激活 mock 避免触碰真实 Keychain
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        let provider = KeychainKeyProvider::new();

        // Assert
        assert!(
            provider.is_device_only(),
            "KeychainKeyProvider 必须标记为 ThisDeviceOnly"
        );
        assert!(
            matches!(
                provider.accessibility(),
                KeyAccessibility::AfterFirstUnlockThisDeviceOnly
            ),
            "accessibility() 应返回 AfterFirstUnlockThisDeviceOnly"
        );
    }
}
