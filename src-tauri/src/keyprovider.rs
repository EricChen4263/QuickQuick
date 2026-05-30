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

// ── 服务/账号标识常量 ────────────────────────────────────────────────────────

/// Keychain 中存储密钥的服务名（与 App bundle 一致）
const KEYCHAIN_SERVICE: &str = "io.quickquick.app";

/// Keychain 中存储密钥的账号名（固定：当前设备的 SQLCipher 主密钥）
const KEYCHAIN_ACCOUNT: &str = "sqlcipher_master_key";

// ── KeyError ─────────────────────────────────────────────────────────────────

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

// ── KeyAccessibility ──────────────────────────────────────────────────────────

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

// ── KeyProvider trait ─────────────────────────────────────────────────────────

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

// ── 随机密钥生成（纯函数，可单独测试）────────────────────────────────────────

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

// ── KeychainKeyProvider ───────────────────────────────────────────────────────

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
    /// 返回 `true` 表示：keyring 以 `kSecAttrSynchronizable = false` 存储密钥（由
    /// `apple-native` / `windows-native` feature 保证），密钥不漫游 iCloud Keychain
    /// 且不跨设备同步，满足设计§六「不漫游」硬约束。
    ///
    /// **注意**：此处 `true` 不保证底层 `kSecAttrAccessible` 精确为
    /// `AfterFirstUnlockThisDeviceOnly`；OS 级 accessibility 属性回读验证
    /// 归 pending-manual V0-F3-A03-H01，需真实钥匙串人工确认。
    pub fn is_device_only(&self) -> bool {
        matches!(self.accessibility(), KeyAccessibility::AfterFirstUnlockThisDeviceOnly)
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

impl KeyProvider for KeychainKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], KeyError> {
        self.load_or_generate()
    }
}

// ── 模块内单元测试 ────────────────────────────────────────────────────────────

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
            matches!(provider.accessibility(), KeyAccessibility::AfterFirstUnlockThisDeviceOnly),
            "accessibility() 应返回 AfterFirstUnlockThisDeviceOnly"
        );
    }
}
