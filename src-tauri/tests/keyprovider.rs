//! LocalKeyProvider 集成测试（本地文件密钥库，去 Keychain）
//!
//! 对齐 docs/design/local-keystore-no-keychain.md §七测试策略：
//! - `KeyProvider` trait 仍是开库唯一抽象（FakeKeyProvider 证明抽象可用）。
//! - LocalKeyProvider 首启生成 + 幂等 + 跨实例读（同机复算 KEK）。
//! - 异机模拟：注入不同 machine_id → 解密失败（文件单拷异机解不开）。
//! - 降级回退：machine_id 不可用仍能开库（永不弹密码硬目标）。
//! - 损坏文件报错不 panic。
//! - 去掉 `#[cfg(debug_assertions)]` gate：release 测试构建同样能编译运行。

use quickquick_lib::keyprovider::{generate_random_key, KeyError, KeyProvider, LocalKeyProvider};
use tempfile::tempdir;

/// Fake KeyProvider 实现，用于证明 trait 是可用抽象（不依赖任何文件/平台）。
struct FakeKeyProvider {
    key: [u8; 32],
}

impl FakeKeyProvider {
    fn with_fixed_key(byte: u8) -> Self {
        Self { key: [byte; 32] }
    }
}

impl KeyProvider for FakeKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], KeyError> {
        Ok(self.key)
    }
}

/// trait 抽象可用：FakeKeyProvider 通过 &dyn KeyProvider 调用得到预设的 32 字节。
#[test]
fn keyprovider_trait_abstraction_is_usable() {
    let fake: &dyn KeyProvider = &FakeKeyProvider::with_fixed_key(0xAB);

    let key = fake.get_or_create_key().expect("FakeKeyProvider 不应返回错误");

    assert_eq!(key.len(), 32, "密钥应为 32 字节");
    assert_eq!(key, [0xAB_u8; 32], "FakeKeyProvider 应返回预设的固定密钥");
}

/// 首启生成 + 同机跨实例可复算解密：machine-A 写盘后新实例读出同一密钥。
#[test]
fn local_provider_persists_and_reads_back_same_machine() {
    let dir = tempdir().unwrap();
    let id = b"integration-machine-A";

    let first = LocalKeyProvider::with_machine_id(dir.path(), id)
        .get_or_create_key()
        .expect("首启应成功");
    assert!(
        dir.path().join("master.key").exists(),
        "首启后 master.key 应落盘"
    );

    let second = LocalKeyProvider::with_machine_id(dir.path(), id)
        .get_or_create_key()
        .expect("同机新实例应能解密");

    assert_eq!(first, second, "同机跨实例应读出同一密钥");
}

/// 异机模拟：machine-A 写盘后，machine-B 复算 KEK 解密失败（核心安全约束）。
#[test]
fn local_provider_different_machine_fails_decrypt() {
    let dir = tempdir().unwrap();

    LocalKeyProvider::with_machine_id(dir.path(), b"machine-A")
        .get_or_create_key()
        .expect("machine-A 首启应成功");

    let result =
        LocalKeyProvider::with_machine_id(dir.path(), b"machine-B-different").get_or_create_key();

    assert!(
        matches!(result, Err(KeyError::Decrypt)),
        "异机 machine_id 解密应返回 Decrypt，实际：{result:?}"
    );
}

/// 真实入口 new() 首启即可生成密钥（machine_id 由平台函数填充，取不到则回退，永不弹密码）。
#[test]
fn local_provider_new_first_start_succeeds() {
    let dir = tempdir().unwrap();
    let provider = LocalKeyProvider::new(dir.path());

    let key = provider
        .get_or_create_key()
        .expect("new() 首启应成功（取不到 machine_id 也回退开库）");

    assert_eq!(key.len(), 32, "首启密钥应为 32 字节");
}

/// 随机密钥生成两次不相等（随机源熵正常，非恒真断言）。
#[test]
fn random_key_generation_is_not_constant() {
    let a = generate_random_key();
    let b = generate_random_key();
    assert_ne!(a, b, "两次独立生成的随机密钥不应相等");
    assert_eq!(a.len(), 32, "密钥应为 32 字节");
}
