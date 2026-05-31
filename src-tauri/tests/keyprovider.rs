//! KeyProvider 集成测试
//!
//! 验收项 V0-F3-A03：KeyProvider 抽象接口存在，v1 仅 KeychainKeyProvider 实现；
//! 密钥标记 ThisDeviceOnly 不漫游。
//! 验收项 V3-F2-A05：密钥可访问性——AfterFirstUnlock + ThisDeviceOnly（不漫游）。
//!
//! 测试策略：
//! - 用 keyring mock 后端替换真实 OS 钥匙串，测试全程 headless、不弹窗。
//! - Fake 实现 FakeKeyProvider 证明 trait 是可用抽象。
//! - 断言 KeychainKeyProvider 的 is_device_only 与 accessibility 语义。

use quickquick_lib::keyprovider::{KeyAccessibility, KeychainKeyProvider, KeyProvider};

/// Fake KeyProvider 实现，用于证明 trait 是可用抽象（不依赖 Keychain）
struct FakeKeyProvider {
    key: [u8; 32],
}

impl FakeKeyProvider {
    fn with_fixed_key(byte: u8) -> Self {
        Self { key: [byte; 32] }
    }
}

impl KeyProvider for FakeKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], quickquick_lib::keyprovider::KeyError> {
        Ok(self.key)
    }
}

/// V0-F3-A03 主验收测试：
/// a) 抽象可用：FakeKeyProvider 通过 &dyn KeyProvider 调用得到 32 字节。
/// b) 设备绑定：KeychainKeyProvider::is_device_only()==true 且 accessibility() 为 ThisDeviceOnly。
/// c) 幂等性：KeychainKeyProvider mock 模式下首次生成密钥、二次调用返回同一密钥。
#[test]
fn keyprovider_abstraction_and_device_only() {
    // Arrange
    let fake: &dyn KeyProvider = &FakeKeyProvider::with_fixed_key(0xAB);

    // Act
    let key = fake.get_or_create_key().expect("FakeKeyProvider 不应返回错误");

    // Assert：确实得到 32 字节，且值与预期一致（非恒真）
    assert_eq!(key.len(), 32, "密钥应为 32 字节");
    assert_eq!(key, [0xAB_u8; 32], "FakeKeyProvider 应返回预设的固定密钥");

    // Arrange：激活 keyring mock，使 KeychainKeyProvider 不碰真实 OS 钥匙串
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
    let provider = KeychainKeyProvider::new();

    // Assert：is_device_only 必须为 true（密钥不漫游）
    assert!(
        provider.is_device_only(),
        "KeychainKeyProvider 必须标记为 ThisDeviceOnly（不漫游）"
    );

    // Assert：accessibility() 返回 AfterFirstUnlockThisDeviceOnly 变体
    assert!(
        matches!(provider.accessibility(), KeyAccessibility::AfterFirstUnlockThisDeviceOnly),
        "accessibility() 应返回 AfterFirstUnlockThisDeviceOnly"
    );

    let key1 = provider
        .get_or_create_key()
        .expect("首次 get_or_create_key 不应失败");
    let key2 = provider
        .get_or_create_key()
        .expect("二次 get_or_create_key 不应失败");

    assert_eq!(key1.len(), 32, "首次密钥应为 32 字节");
    assert_eq!(key1, key2, "两次调用应返回同一密钥（幂等）");
}

/// V3-F2-A05 验收测试：key_accessibility_flags
///
/// 断言：
/// a) accessibility() == AfterFirstUnlockThisDeviceOnly
/// b) is_device_only() == true（不漫游）
/// c) is_after_first_unlock() == true（锁屏后台仍可读）
/// d) 实际存储属性标识为 AfterFirstUnlockThisDeviceOnly（非 WhenUnlocked）
/// e) synchronizable == false（不漫游 iCloud/凭据）
#[test]
fn key_accessibility_flags() {
    // Arrange：激活 keyring mock，全程 headless 不触碰真实钥匙串
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
    let provider = KeychainKeyProvider::new();

    // Assert a：accessibility 语义枚举
    assert!(
        matches!(provider.accessibility(), KeyAccessibility::AfterFirstUnlockThisDeviceOnly),
        "accessibility() 应返回 AfterFirstUnlockThisDeviceOnly"
    );

    // Assert b：设备绑定不漫游
    assert!(
        provider.is_device_only(),
        "is_device_only() 应为 true（密钥不漫游）"
    );

    // Assert c：锁屏后台仍可读语义
    assert!(
        provider.is_after_first_unlock(),
        "is_after_first_unlock() 应为 true（锁屏后台可读）"
    );

    // Assert d：实际存储属性标识（区分 AfterFirstUnlock vs WhenUnlocked）
    // 使用 provider.storage_attributes() 覆盖被测方法路径，而非旁路 Default impl
    let attrs = provider.storage_attributes();
    assert_eq!(
        attrs.accessibility_identifier(),
        "AfterFirstUnlockThisDeviceOnly",
        "存储属性标识应为 AfterFirstUnlockThisDeviceOnly，而非 WhenUnlocked"
    );

    // Assert e：synchronizable=false（不漫游 iCloud/凭据，非恒真：若为 true 则失败）
    assert!(
        !attrs.synchronizable(),
        "synchronizable 应为 false，密钥不得漫游 iCloud Keychain"
    );
}

/// 随机密钥生成单测：两次生成的 32 字节不相等（非恒真断言）
#[test]
fn random_key_generation_is_not_constant() {
    use quickquick_lib::keyprovider::generate_random_key;

    // Arrange & Act
    let key_a = generate_random_key();
    let key_b = generate_random_key();

    // Assert：两次独立生成不应相同（若相同则随机源不可用）
    assert_ne!(
        key_a, key_b,
        "两次独立生成的随机密钥不应相等（随机源熵正常）"
    );
    assert_eq!(key_a.len(), 32, "密钥应为 32 字节");
    assert_eq!(key_b.len(), 32, "密钥应为 32 字节");
}
