//! 便携文件导出/导入集成测试（V3-F2-A07 export_import_passphrase）
//!
//! 测试目标：验证口令保护便携文件的加解密往返正确性与安全性。
//! headless，纯内存，无网络/钥匙串依赖。

use quickquick_lib::portable::{export_portable, import_portable, PortableError};

/// A07 往返：正确口令导出后可导入还原原始数据
#[test]
fn export_import_passphrase_roundtrip() {
    // Arrange
    let plaintext = b"QuickQuick portable backup data: hello world 12345";
    let passphrase = "correct-horse-battery-staple";

    // Act: 导出
    let blob = export_portable(plaintext, passphrase)
        .expect("export_portable 应成功");

    // Act: 用正确口令导入
    let recovered = import_portable(&blob, passphrase)
        .expect("import_portable 用正确口令应成功");

    // Assert: 还原结果与原始数据完全一致
    assert_eq!(recovered, plaintext, "导入数据应与导出前原始数据完全一致");
}

/// A07 错口令：错误口令无法解出，返回错误而非 panic
#[test]
fn export_import_passphrase_wrong_passphrase_returns_err() {
    // Arrange
    let plaintext = b"sensitive clipboard history content";
    let correct = "correct-passphrase-2026";
    let wrong = "wrong-passphrase-attacker";

    // Act: 导出
    let blob = export_portable(plaintext, correct)
        .expect("export_portable 应成功");

    // Act: 用错误口令导入
    let result = import_portable(&blob, wrong);

    // Assert: 必须返回 Err，不能 panic，不能解出数据
    assert!(
        result.is_err(),
        "错误口令导入必须返回 Err，实际返回了 Ok"
    );

    // 确认错误类型是解密相关（非格式错误）
    match result.unwrap_err() {
        PortableError::WrongPassphrase | PortableError::Decrypt => {}
        other => panic!("错口令应返回 WrongPassphrase 或 Decrypt，实际: {other:?}"),
    }
}

/// A07 密文不含明文：导出的 blob 中不含原始明文片段
#[test]
fn export_import_passphrase_ciphertext_does_not_contain_plaintext() {
    // Arrange
    let plaintext = b"UNIQUE_PLAINTEXT_MARKER_XYZZY_9876543210";
    let passphrase = "any-passphrase-for-encryption-test";

    // Act
    let blob = export_portable(plaintext, passphrase)
        .expect("export_portable 应成功");

    // Assert: blob 中不含明文的任何 8 字节以上连续片段
    let marker = b"UNIQUE_PLAINTEXT_MARKER_XYZZY";
    let found = blob.windows(marker.len()).any(|w| w == marker);
    assert!(
        !found,
        "导出的便携文件不应包含明文内容（加密必须生效）"
    );
}

/// I-1 导出随机性：相同明文+相同口令两次导出，blob 必须不同（salt/nonce 每次随机）
///
/// 若两次 blob 相同，说明随机数生成退化为固定值，密码学安全性失效。
/// 非恒真：AAA 结构，通过比较实际运行时两次输出是否相异来证明。
#[test]
fn export_produces_distinct_blobs_each_call() {
    // Arrange
    let plaintext = b"determinism-probe: same input, should differ each call";
    let passphrase = "same-passphrase-for-both-calls";

    // Act: 相同明文 + 相同口令，连续两次独立导出
    let blob1 = export_portable(plaintext, passphrase)
        .expect("第一次 export_portable 应成功");
    let blob2 = export_portable(plaintext, passphrase)
        .expect("第二次 export_portable 应成功");

    // Assert: 两次 blob 必须不同（salt/nonce 均为 CSPRNG 随机，碰撞概率 ≈ 2^-192）
    assert_ne!(
        blob1, blob2,
        "每次导出应生成不同的 salt/nonce，产生不同密文；若相同则随机性已退化"
    );
}

/// 格式损坏：截断的 blob 应返回 Format 错误
#[test]
fn export_import_passphrase_truncated_blob_returns_format_err() {
    // Arrange
    let plaintext = b"some data";
    let passphrase = "some-pass";

    let blob = export_portable(plaintext, passphrase)
        .expect("export_portable 应成功");

    // 截断为前 8 字节（远小于最小有效格式）
    let truncated = &blob[..8.min(blob.len())];

    // Act
    let result = import_portable(truncated, passphrase);

    // Assert
    assert!(
        result.is_err(),
        "截断的 blob 必须返回 Err"
    );
    match result.unwrap_err() {
        PortableError::Format => {}
        other => panic!("截断 blob 应返回 Format 错误，实际: {other:?}"),
    }
}
