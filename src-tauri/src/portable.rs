//! 便携文件导出/导入模块（V3-F2-S06）
//!
//! 提供口令保护的便携文件加解密功能，用于跨设备备份与迁移剪贴板数据。
//!
//! # 便携文件格式（小端字节序，紧凑二进制）
//!
//! ```text
//! offset  len   字段
//! 0       4     magic: 0x51_51_42_50 ("QQBP" = QuickQuick Backup Portable)
//! 4       1     version: 0x01
//! 5       32    argon2id salt（随机，KDF 输入之一）
//! 37      12    AES-GCM nonce（随机，加密随机数）
//! 49      N     AES-GCM ciphertext + 16-byte authentication tag
//! ```
//!
//! # 安全设计
//! - KDF：Argon2id（m=65536 KiB, t=3 次迭代, p=4 并行度），派生 32 字节对称密钥。
//!   参数参考 OWASP 推荐（2023）：m≥64MB, t≥3, p=4。
//! - AEAD：AES-256-GCM，tag 16 字节，保障机密性与完整性。
//! - 口令及派生密钥绝不写入日志或错误消息。

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use thiserror::Error;
use zeroize::Zeroizing;

/// 便携文件 magic 头（"QQBP" Little-Endian u32）
const MAGIC: [u8; 4] = [0x51, 0x51, 0x42, 0x50];

/// 便携文件格式版本
const FORMAT_VERSION: u8 = 0x01;

/// Argon2id salt 长度（字节）
const SALT_LEN: usize = 32;

/// AES-GCM nonce 长度（字节）
const NONCE_LEN: usize = 12;

/// AES-256-GCM key 长度（字节）
const KEY_LEN: usize = 32;

/// 头部固定长度：magic(4) + version(1) + salt(32) + nonce(12)
const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN;

/// Argon2id 内存开销（KiB）：64 MB，符合 OWASP 2023 推荐下限
const ARGON2_MEM_KIB: u32 = 65536;

/// Argon2id 迭代次数：3 次，OWASP 推荐下限
const ARGON2_ITERATIONS: u32 = 3;

/// Argon2id 并行度：4 线程
const ARGON2_PARALLELISM: u32 = 4;

/// 便携文件操作错误类型
#[derive(Debug, Error)]
pub enum PortableError {
    /// 口令错误导致 AEAD 验签失败（最终呈现给用户）
    #[error("口令错误，无法解密便携文件")]
    WrongPassphrase,

    /// AES-GCM 解密/验签失败（内部详情不含口令）
    #[error("解密失败，文件可能已损坏或口令错误")]
    Decrypt,

    /// 便携文件格式非法（magic/version 不符或长度不足）
    #[error("便携文件格式无效")]
    Format,

    /// Argon2 KDF 参数构建或派生失败
    #[error("密钥派生失败")]
    Kdf,
}

/// 从口令派生 AES-256-GCM 对称密钥。
///
/// 使用 Argon2id 算法，参数：m=65536 KiB, t=3, p=4（OWASP 2023 推荐）。
/// 返回 `Zeroizing` 包装的 32 字节密钥：调用方 Drop 时自动清零内存，
/// 防止派生密钥在堆栈/堆上残留（纵深防御）。口令与密钥绝不写入日志。
fn derive_key(
    passphrase: &str,
    salt: &[u8; SALT_LEN],
) -> Result<Zeroizing<[u8; KEY_LEN]>, PortableError> {
    let params =
        Params::new(ARGON2_MEM_KIB, ARGON2_ITERATIONS, ARGON2_PARALLELISM, Some(KEY_LEN))
            .map_err(|_| PortableError::Kdf)?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    // Zeroizing 包装：Drop 时自动 memset(0)，确保密钥不在内存中残留
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut())
        .map_err(|_| PortableError::Kdf)?;

    Ok(key)
}

/// 导出便携文件：将明文数据用口令加密后打包为便携字节流。
///
/// 内部流程：生成随机 salt 和 nonce → Argon2id 派生密钥 → AES-256-GCM 加密 →
/// 拼接 magic/version/salt/nonce/ciphertext。
///
/// # Errors
/// - `PortableError::Kdf`：Argon2 参数非法或 hash 失败
/// - `PortableError::Decrypt`：AES-GCM 加密异常（极少见）
pub fn export_portable(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>, PortableError> {
    let mut rng = rand::thread_rng();

    // 生成随机 salt 和 nonce（均来自 OS CSPRNG）
    let mut salt = [0u8; SALT_LEN];
    rng.fill_bytes(&mut salt);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng.fill_bytes(&mut nonce_bytes);

    // KDF：口令 + salt → 256-bit 密钥（Zeroizing 包装，Drop 时自动清零）
    let key_bytes = derive_key(passphrase, &salt)?;
    let key = Key::<Aes256Gcm>::from_slice(key_bytes.as_ref());
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // AES-GCM 加密：输出含 ciphertext + 16-byte tag
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| PortableError::Decrypt)?;

    // 拼接便携格式：magic + version + salt + nonce + ciphertext
    let mut blob = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    blob.extend_from_slice(&MAGIC);
    blob.push(FORMAT_VERSION);
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);

    Ok(blob)
}

/// 导入便携文件：解析格式，用口令解密还原明文。
///
/// 内部流程：校验 magic/version → 提取 salt/nonce → Argon2id 派生密钥 →
/// AES-256-GCM 解密验签 → 返回明文。
///
/// # Errors
/// - `PortableError::Format`：magic 不符、版本未知或长度不足
/// - `PortableError::Kdf`：KDF 失败
/// - `PortableError::WrongPassphrase`：AEAD 验签失败（口令错误或数据篡改）
pub fn import_portable(blob: &[u8], passphrase: &str) -> Result<Vec<u8>, PortableError> {
    // 最小长度校验：头部 + AES-GCM tag（16 字节）
    if blob.len() < HEADER_LEN + 16 {
        return Err(PortableError::Format);
    }

    // 校验 magic
    if blob[..4] != MAGIC {
        return Err(PortableError::Format);
    }

    // 校验 version
    if blob[4] != FORMAT_VERSION {
        return Err(PortableError::Format);
    }

    // 提取 salt 和 nonce
    let salt: &[u8; SALT_LEN] = blob[5..5 + SALT_LEN]
        .try_into()
        .map_err(|_| PortableError::Format)?;

    let nonce_start = 5 + SALT_LEN;
    let nonce_bytes: &[u8; NONCE_LEN] = blob[nonce_start..nonce_start + NONCE_LEN]
        .try_into()
        .map_err(|_| PortableError::Format)?;

    let ciphertext = &blob[HEADER_LEN..];

    // KDF：口令 + salt → 256-bit 密钥（Zeroizing 包装，Drop 时自动清零）
    let key_bytes = derive_key(passphrase, salt)?;
    let key = Key::<Aes256Gcm>::from_slice(key_bytes.as_ref());
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    // AES-GCM 解密：tag 验签失败意味着口令错误或数据损坏
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| PortableError::WrongPassphrase)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 derive_key 对相同输入产生相同输出（确定性 KDF）
    #[test]
    fn derive_key_is_deterministic() {
        // Arrange
        let passphrase = "test-passphrase";
        let salt = [0x42u8; SALT_LEN];

        // Act
        let key1 = derive_key(passphrase, &salt).expect("KDF 不应失败");
        let key2 = derive_key(passphrase, &salt).expect("KDF 第二次不应失败");

        // Assert: 相同输入 → 相同密钥（确定性）
        assert_eq!(key1, key2, "KDF 必须是确定性的");
        assert_eq!(key1.len(), KEY_LEN, "派生密钥应为 32 字节");
    }

    /// 验证不同 salt 产生不同密钥（salt 隔离）
    #[test]
    fn derive_key_different_salts_produce_different_keys() {
        // Arrange
        let passphrase = "same-passphrase";
        let salt_a = [0x01u8; SALT_LEN];
        let salt_b = [0x02u8; SALT_LEN];

        // Act
        let key_a = derive_key(passphrase, &salt_a).expect("KDF A 不应失败");
        let key_b = derive_key(passphrase, &salt_b).expect("KDF B 不应失败");

        // Assert: 不同 salt → 不同密钥
        assert_ne!(key_a, key_b, "不同 salt 必须产生不同密钥");
    }
}
