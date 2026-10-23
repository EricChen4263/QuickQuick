//! 便携文件导出/导入模块
//!
//! 提供口令保护的便携文件加解密功能，用于跨设备备份与迁移剪贴板数据。
//!
//! 本模块分两层：
//! 1. **加密层**：`export_portable` / `import_portable`，对任意明文做口令加密；
//! 2. **载荷层**：`encode_backup_payload` / `decode_backup_payload`，
//!    定义剪贴板历史在便携文件内部的明文结构（见下文「备份载荷格式」）。
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
//! # 备份载荷格式
//!
//! 便携文件里的明文是自描述的 UTF-8 JSON 文档（`format` + `version` 双字段）：
//!
//! ```json
//! {
//!   "format": "qqbackup",
//!   "version": 1,
//!   "createdUtc": 1700000000000,
//!   "appVersion": "0.3.8",
//!   "items": [
//!     {
//!       "kind": "text",
//!       "content": "纯文本内容",
//!       "isFavorite": false,
//!       "createdUtc": 1700000000000,
//!       "lastModifiedUtc": 1700000000000
//!     },
//!     {
//!       "kind": "richtext",
//!       "content": "富文本内容",
//!       "html": "<b>富文本内容</b>",
//!       "isFavorite": true,
//!       "createdUtc": 1700000000000,
//!       "lastModifiedUtc": 1700000000000
//!     },
//!     {
//!       "kind": "image",
//!       "content": "[图片] 800×600",
//!       "isFavorite": false,
//!       "createdUtc": 1700000000000,
//!       "lastModifiedUtc": 1700000000000,
//!       "image": {
//!         "imageHash": "0123456789abcdef",
//!         "originalPresent": true,
//!         "thumbnailBase64": "iVBORw0KGgo...",
//!         "originalBase64": "iVBORw0KGgo..."
//!       }
//!     }
//!   ]
//! }
//! ```
//!
//! 兼容边界：
//! - **未知字段不失败**：读端按 serde 默认忽略未识别字段，将来加字段不会让旧版本解析失败；
//! - **未知版本显式失败**：`version` 高于 [`PAYLOAD_VERSION`] 时报
//!   [`PortableError::UnsupportedVersion`]，绝不猜测新版本语义；
//! - **可选字段带默认值**：`html` / `image` / `appVersion` 缺省即取 `None` / 空串。
//!
//! 图片条目的判重键 `imageHash` 随载荷显式携带：降级条目（`originalPresent=false`）
//! 的原图从未落库，哈希无法再由字节重算，只能沿用库内的判重键。
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
use serde::{Deserialize, Serialize};
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
    ///
    /// 文案刻意把「口令错误」与「文件损坏」并列：AEAD 验签失败在密码学上无法
    /// 区分两者，断言其一会让用户误判文件状态。
    #[error("备份口令错误，或文件已损坏")]
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

    /// 备份载荷解析失败（明文不是合法 JSON，或格式标识/必填字段不符）
    #[error("备份载荷格式无效")]
    Payload,

    /// 备份载荷版本高于当前实现支持的版本
    #[error("备份文件版本 {found} 不受支持（当前最高支持 {supported}）")]
    UnsupportedVersion {
        /// 文件里声明的版本号
        found: u32,
        /// 本实现能解析的最高版本号
        supported: u32,
    },
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
    let params = Params::new(
        ARGON2_MEM_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(KEY_LEN),
    )
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

/// 备份载荷的格式标识（写入 `format` 字段，解析时校验）
pub const PAYLOAD_FORMAT: &str = "qqbackup";

/// 备份载荷的当前版本（写入 `version` 字段）
pub const PAYLOAD_VERSION: u32 = 1;

/// 一份完整备份载荷（便携文件解密后的明文文档）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPayload {
    /// 格式标识，必须等于 [`PAYLOAD_FORMAT`]
    pub format: String,
    /// 载荷版本，必须落在 `1..=PAYLOAD_VERSION` 内
    pub version: u32,
    /// 备份生成时刻（UTC epoch ms）
    pub created_utc: i64,
    /// 生成该备份的应用版本（仅供诊断，解析时不校验）
    #[serde(default)]
    pub app_version: String,
    /// 条目列表，顺序即写出时的库内排序（收藏优先，组内按最近修改降序）
    pub items: Vec<BackupItem>,
}

/// 备份中的单个剪贴板条目。
///
/// 文本类条目用 `content` + 可选 `html` 表达；图片条目的内容描述放在 `content`，
/// 图片字节与判重键放在 `image`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupItem {
    /// 条目类型："text" | "richtext" | "image"
    pub kind: String,
    /// 条目内容（文本内容，或图片条目的尺寸描述）
    pub content: String,
    /// 富文本 HTML（仅 richtext 条目有）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    /// 收藏标记
    #[serde(default)]
    pub is_favorite: bool,
    /// 条目创建时刻（UTC epoch ms）
    pub created_utc: i64,
    /// 条目最后修改时刻（UTC epoch ms）
    pub last_modified_utc: i64,
    /// 图片字段（仅 image 条目有）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<BackupImage>,
}

/// 备份中图片条目的字节与判重信息（PNG 以 base64 承载）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupImage {
    /// 原图字节的判重键（FNV-1a 64-bit 十六进制），导入时用于 `image_hash` 去重
    pub image_hash: String,
    /// 原图是否随载荷携带（false = 原图曾在库内被降级剥离）
    #[serde(default)]
    pub original_present: bool,
    /// 缩略图 PNG 的 base64
    pub thumbnail_base64: String,
    /// 原图 PNG 的 base64；`original_present=false` 时缺省
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_base64: Option<String>,
}

/// 把备份载荷序列化为明文 JSON 字节，随后交给 [`export_portable`] 加密。
///
/// # Errors
/// - `PortableError::Payload`：序列化失败（正常数据不会发生）
pub fn encode_backup_payload(payload: &BackupPayload) -> Result<Vec<u8>, PortableError> {
    serde_json::to_vec(payload).map_err(|_| PortableError::Payload)
}

/// 解析明文 JSON 字节为备份载荷，并校验格式标识与版本。
///
/// 未知字段按 serde 默认忽略（向前兼容）；版本高于 [`PAYLOAD_VERSION`] 时显式报错。
///
/// # Errors
/// - `PortableError::Payload`：不是合法 JSON、格式标识不符或版本号为 0
/// - `PortableError::UnsupportedVersion`：版本高于本实现支持的版本
pub fn decode_backup_payload(plaintext: &[u8]) -> Result<BackupPayload, PortableError> {
    let payload: BackupPayload =
        serde_json::from_slice(plaintext).map_err(|_| PortableError::Payload)?;

    if payload.format != PAYLOAD_FORMAT || payload.version == 0 {
        return Err(PortableError::Payload);
    }

    if payload.version > PAYLOAD_VERSION {
        return Err(PortableError::UnsupportedVersion {
            found: payload.version,
            supported: PAYLOAD_VERSION,
        });
    }

    Ok(payload)
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

    /// 构造覆盖全部字段的备份载荷（中文文本、HTML、图片、收藏位、时间戳）。
    fn sample_payload() -> BackupPayload {
        BackupPayload {
            format: PAYLOAD_FORMAT.to_string(),
            version: PAYLOAD_VERSION,
            created_utc: 1_700_000_000_000,
            app_version: "0.3.8".to_string(),
            items: vec![
                BackupItem {
                    kind: "text".to_string(),
                    content: "中文纯文本：剪贴板历史第一条".to_string(),
                    html: None,
                    is_favorite: false,
                    created_utc: 1_700_000_000_001,
                    last_modified_utc: 1_700_000_000_002,
                    image: None,
                },
                BackupItem {
                    kind: "richtext".to_string(),
                    content: "富文本条目".to_string(),
                    html: Some("<b>富文本条目</b><br/>第二行".to_string()),
                    is_favorite: true,
                    created_utc: 1_700_000_000_003,
                    last_modified_utc: 1_700_000_000_004,
                    image: None,
                },
                BackupItem {
                    kind: "image".to_string(),
                    content: "[图片] 800×600".to_string(),
                    html: None,
                    is_favorite: true,
                    created_utc: 1_700_000_000_005,
                    last_modified_utc: 1_700_000_000_006,
                    image: Some(BackupImage {
                        image_hash: "0123456789abcdef".to_string(),
                        original_present: true,
                        thumbnail_base64: "dGh1bWJuYWls".to_string(),
                        original_base64: Some("b3JpZ2luYWw=".to_string()),
                    }),
                },
            ],
        }
    }

    /// 载荷序列化/反序列化往返：全部字段（含中文、HTML、图片、收藏位、时间戳）逐一保持
    #[test]
    fn backup_payload_round_trip_preserves_all_fields() {
        // Arrange
        let payload = sample_payload();

        // Act
        let encoded = encode_backup_payload(&payload).expect("载荷序列化不应失败");
        let decoded = decode_backup_payload(&encoded).expect("载荷反序列化不应失败");

        // Assert: 整体相等即覆盖每个字段的具体值
        assert_eq!(decoded, payload, "往返后载荷必须与原始载荷完全一致");
        assert_eq!(
            decoded.items[1].html.as_deref(),
            Some("<b>富文本条目</b><br/>第二行")
        );
        assert_eq!(
            decoded.items[2].image.as_ref().unwrap().image_hash,
            "0123456789abcdef"
        );
        assert!(decoded.items[2].is_favorite, "收藏位必须保持");
        assert_eq!(decoded.items[0].created_utc, 1_700_000_000_001);
    }

    /// 向前兼容：未知字段被忽略而非报错（将来加字段不破坏旧版本解析）
    #[test]
    fn decode_backup_payload_ignores_unknown_fields() {
        // Arrange: 顶层与条目层各注入一个本实现不认识的字段
        let json = r#"{
            "format": "qqbackup",
            "version": 1,
            "createdUtc": 1700000000000,
            "appVersion": "9.9.9",
            "futureSection": {"anything": [1, 2, 3]},
            "items": [{
                "kind": "text",
                "content": "带未知字段的条目",
                "isFavorite": true,
                "createdUtc": 1,
                "lastModifiedUtc": 2,
                "futureFlag": "忽略我"
            }]
        }"#;

        // Act
        let decoded = decode_backup_payload(json.as_bytes()).expect("未知字段不应导致解析失败");

        // Assert
        assert_eq!(decoded.items.len(), 1);
        assert_eq!(decoded.items[0].content, "带未知字段的条目");
        assert!(decoded.items[0].is_favorite);
        assert_eq!(decoded.app_version, "9.9.9");
    }

    /// 格式标识不符 → Payload 错误，不猜测内容
    #[test]
    fn decode_backup_payload_rejects_foreign_format() {
        // Arrange
        let json = r#"{"format":"some-other-tool","version":1,"createdUtc":0,"items":[]}"#;

        // Act
        let result = decode_backup_payload(json.as_bytes());

        // Assert
        assert!(
            matches!(result, Err(PortableError::Payload)),
            "外来格式必须被拒绝"
        );
    }

    /// 版本高于当前支持 → UnsupportedVersion，且带上实际读到的版本号
    #[test]
    fn decode_backup_payload_rejects_newer_version() {
        // Arrange
        let json = r#"{"format":"qqbackup","version":99,"createdUtc":0,"items":[]}"#;

        // Act
        let result = decode_backup_payload(json.as_bytes());

        // Assert
        match result {
            Err(PortableError::UnsupportedVersion { found, supported }) => {
                assert_eq!(found, 99);
                assert_eq!(supported, PAYLOAD_VERSION);
            }
            other => panic!("更高版本应返回 UnsupportedVersion，实际：{other:?}"),
        }
    }

    /// 明文不是 JSON → Payload 错误，不 panic
    #[test]
    fn decode_backup_payload_rejects_non_json() {
        // Act
        let result = decode_backup_payload(b"\x00\x01\x02not-json");

        // Assert
        assert!(matches!(result, Err(PortableError::Payload)));
    }

    /// 载荷经真实加密层往返：加密 → 解密 → 解析后字段不变
    #[test]
    fn backup_payload_survives_encrypt_decrypt_round_trip() {
        // Arrange
        let payload = sample_payload();
        let passphrase = "备份口令-SENTINEL-不要泄漏";
        let plaintext = encode_backup_payload(&payload).expect("载荷序列化不应失败");

        // Act
        let blob = export_portable(&plaintext, passphrase).expect("加密不应失败");
        let recovered = import_portable(&blob, passphrase).expect("解密不应失败");
        let decoded = decode_backup_payload(&recovered).expect("载荷解析不应失败");

        // Assert
        assert_eq!(decoded, payload, "加密往返后载荷必须保持一致");
    }
}
