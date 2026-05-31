//! 集成测试：加密数据库核心行为
//!
//! 覆盖验收项：
//! - V0-F3-A01 db_auto_create_on_first_run
//! - V0-F3-A02 db_encrypted_wrong_key_fails
//! - V0-F3-A06 db_corrupt_backup_not_deleted
//! - V3-F2-A06 encryption_failure_tiered（失败分级与恢复）

use quickquick_lib::db;
use quickquick_lib::db::{
    apply_recovery_action, classify_failure, recovery_action, DbError, FailureTier, RecoveryAction,
};
use std::fs;
use tempfile::tempdir;

/// 固定 32 字节测试密钥（全 7），不依赖钥匙串
const KEY_A: [u8; 32] = [7u8; 32];
/// 不同的错误密钥
const KEY_B: [u8; 32] = [99u8; 32];

/// V0-F3-A01：文件不存在 → open_or_create 后文件被创建
#[test]
fn db_create_auto_creates_file_on_first_run() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");

    // 前置断言：文件尚不存在
    assert!(!db_path.exists(), "测试前 db 文件不应存在");

    // Act
    let conn = db::open_or_create(&db_path, &KEY_A).expect("open_or_create 应成功");
    drop(conn);

    // Assert：文件已被创建
    assert!(db_path.exists(), "open_or_create 后 db 文件应存在");
}

/// V0-F3-A01：已存在文件 → open_or_create 可重复打开（幂等）
#[test]
fn db_create_is_idempotent_on_subsequent_opens() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");

    // 首次创建
    let conn1 = db::open_or_create(&db_path, &KEY_A).expect("首次 open_or_create 应成功");
    drop(conn1);

    // Act：第二次打开同一文件（已有 schema）
    let conn2 = db::open_or_create(&db_path, &KEY_A).expect("第二次 open_or_create 应成功");
    drop(conn2);

    // Assert：文件存在且可正常打开（不抛错即通过）
    assert!(db_path.exists(), "重复打开后 db 文件应仍然存在");
}

/// V0-F3-A02：用正确密钥建库 → 用错误密钥打开应返回 Err
#[test]
fn db_encrypt_wrong_key_returns_error() {
    // Arrange：用 KEY_A 创建并写入一行
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");

    {
        let conn = db::open_or_create(&db_path, &KEY_A).expect("正确 key 建库应成功");
        drop(conn);
    }

    // Act：用 KEY_B（错误密钥）尝试打开
    let result = db::open_or_create(&db_path, &KEY_B);

    // Assert：必须返回错误
    assert!(
        result.is_err(),
        "错误密钥打开加密库应返回 Err，实际返回了 Ok"
    );
}

/// V0-F3-A02：密文落盘——文件头部不含 SQLite 明文魔数
#[test]
fn db_encrypt_ciphertext_on_disk() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");

    {
        let conn = db::open_or_create(&db_path, &KEY_A).expect("建库应成功");
        drop(conn);
    }

    // Act：读文件头 16 字节
    let header = fs::read(&db_path).expect("读 db 文件应成功");
    let sqlite_magic = b"SQLite format 3\x00";

    // Assert：开头不是 SQLite 明文魔数（证明密文落盘）
    assert!(
        header.len() >= 16,
        "db 文件应至少 16 字节，实际 {} 字节",
        header.len()
    );
    assert_ne!(
        &header[..16],
        sqlite_magic,
        "加密库头部不应为 SQLite 明文魔数，发现明文数据库"
    );
}

/// V0-F3-A06：损坏文件 + allow_rebuild=false → 返回 Err，备份文件存在
#[test]
fn db_recovery_corrupt_file_creates_backup_and_returns_err() {
    // Arrange：写入乱字节模拟损坏的 db
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let corrupt_data = b"this is not a valid sqlite or sqlcipher database file!!!";
    fs::write(&db_path, corrupt_data).expect("写损坏文件应成功");

    // Act：allow_rebuild=false
    let result = db::open_or_recover(&db_path, &KEY_A, false);

    // Assert 1：必须返回 Err（不建空库）
    assert!(
        result.is_err(),
        "损坏库 allow_rebuild=false 应返回 Err，实际返回了 Ok"
    );

    // Assert 2：备份文件存在（旧库被改名保留）
    let parent = db_path.parent().expect("路径有父目录");
    let backups: Vec<_> = fs::read_dir(parent)
        .expect("读目录应成功")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains("quickquick.db.corrupt-")
        })
        .collect();
    assert!(
        !backups.is_empty(),
        "损坏库应被改名备份（quickquick.db.corrupt-<utc>），未找到备份"
    );

    // Assert 3：原损坏内容被保留在备份里（未静默删）
    let backup_path = &backups[0].path();
    let backup_content = fs::read(backup_path).expect("读备份文件应成功");
    assert_eq!(
        backup_content, corrupt_data,
        "备份文件应保留原损坏内容，不得静默删除或覆盖"
    );
}

/// V0-F3-A06：损坏文件 + allow_rebuild=true → 备份在 + 新空库可正常打开
#[test]
fn db_recovery_allow_rebuild_creates_new_db_and_keeps_backup() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let corrupt_data = b"totally garbage bytes for sqlcipher to reject";
    fs::write(&db_path, corrupt_data).expect("写损坏文件应成功");

    // Act：allow_rebuild=true
    let result = db::open_or_recover(&db_path, &KEY_A, true);

    // Assert 1：返回 Ok（新空库已建）
    assert!(
        result.is_ok(),
        "损坏库 allow_rebuild=true 应返回 Ok（新空库），实际返回: {:?}",
        result.err()
    );
    drop(result);

    // Assert 2：备份文件存在
    let parent = db_path.parent().expect("路径有父目录");
    let backups: Vec<_> = fs::read_dir(parent)
        .expect("读目录应成功")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains("quickquick.db.corrupt-")
        })
        .collect();
    assert!(!backups.is_empty(), "备份文件应存在");

    // Assert 3：新空库可正常打开（幂等再开）
    let conn2 = db::open_or_create(&db_path, &KEY_A).expect("新空库应可再次打开");
    drop(conn2);
}

// V3-F2-A06：encryption_failure_tiered 失败分级与恢复

/// V3-F2-A06 (enc_failure)：瞬时错误（Backend）→ classify_failure 返回 Transient
#[test]
fn enc_failure_transient_backend_error_classifies_as_transient() {
    // Arrange：模拟钥匙串被拒/锁定，对应 KeyError::Backend 类场景；
    // 用 DbError::TransientKeychain 表示上层瞬时钥匙串拒绝
    let transient_err = DbError::transient_keychain_error("钥匙串被系统锁定，请重试");

    // Act
    let tier = classify_failure(&transient_err);

    // Assert：瞬时错误 → Transient
    assert_eq!(
        tier,
        FailureTier::Transient,
        "钥匙串被拒/锁定应分类为 Transient"
    );
}

/// V3-F2-A06 (enc_failure)：瞬时分级 → recovery_action 返回 RetryNoTouch
#[test]
fn enc_failure_transient_tier_maps_to_retry_no_touch() {
    // Arrange
    let tier = FailureTier::Transient;

    // Act
    let action = recovery_action(tier);

    // Assert：Transient → 不碰库，仅提示重试
    assert_eq!(
        action,
        RecoveryAction::RetryNoTouch,
        "Transient 分级应映射到 RetryNoTouch（绝不改名/删/重建库）"
    );
}

/// V3-F2-A06 (enc_failure)：瞬时失败时库文件原样保留（apply_recovery_action 非恒真）
///
/// 非恒真验证：写入真实文件后通过 apply_recovery_action(RetryNoTouch) 执行恢复调度；
/// 若实现误删/改动文件则 content_after != original_content，测试失败。
#[test]
fn enc_failure_transient_leaves_db_file_untouched() {
    // Arrange：写入有内容的真实文件，模拟现有库
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let original_content = b"original db content that must not be touched";
    fs::write(&db_path, original_content).expect("写文件应成功");

    // 确认 classify→action 映射正确（Transient → RetryNoTouch）
    let transient_err = DbError::transient_keychain_error("瞬时锁定");
    let tier = classify_failure(&transient_err);
    let action = recovery_action(tier);
    assert_eq!(tier, FailureTier::Transient, "TransientKeychain 应分类为 Transient");
    assert_eq!(action, RecoveryAction::RetryNoTouch, "Transient 应映射到 RetryNoTouch");

    // Act：执行恢复调度薄函数（RetryNoTouch 语义 = 不碰文件）
    apply_recovery_action(&db_path, RecoveryAction::RetryNoTouch)
        .expect("RetryNoTouch 不应返回错误");

    // Assert 1：文件仍存在且字节未变（若实现误删/改则此断言失败 = 非恒真）
    assert!(db_path.exists(), "瞬时失败后库文件应仍然存在");
    let content_after = fs::read(&db_path).expect("读文件应成功");
    assert_eq!(
        content_after, original_content,
        "RetryNoTouch 后库文件内容不得被改动"
    );

    // Assert 2：目录中无备份文件（RetryNoTouch 不触发改名）
    let parent = db_path.parent().expect("路径有父目录");
    let backups: Vec<_> = fs::read_dir(parent)
        .expect("读目录应成功")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
        .collect();
    assert!(
        backups.is_empty(),
        "RetryNoTouch 不得产生备份文件（库文件应原样保留）"
    );
}

/// V3-F2-A06 (enc_failure)：库损坏错误 → classify_failure 返回 Permanent
#[test]
fn enc_failure_corrupt_db_classifies_as_permanent() {
    // Arrange：DbError::Corrupt 代表库文件已损坏（永久性失败）
    let corrupt_err = DbError::Corrupt {
        backup_path: "/tmp/test.db.corrupt-1234".to_string(),
    };

    // Act
    let tier = classify_failure(&corrupt_err);

    // Assert：库损坏 → Permanent
    assert_eq!(
        tier,
        FailureTier::Permanent,
        "库损坏应分类为 Permanent"
    );
}

/// V3-F2-A06 (enc_failure)：SQLite 解密失败 → classify_failure 返回 Permanent
#[test]
fn enc_failure_sqlite_decrypt_failure_classifies_as_permanent() {
    // Arrange：SQLite 错误通常代表解密失败或格式错误（永久性失败）
    let sqlite_err = DbError::Sqlite(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error {
            code: rusqlite::ffi::ErrorCode::NotADatabase,
            extended_code: 26,
        },
        Some("file is not a database".to_string()),
    ));

    // Act
    let tier = classify_failure(&sqlite_err);

    // Assert：解密失败 → Permanent
    assert_eq!(
        tier,
        FailureTier::Permanent,
        "SQLite 解密/格式错误应分类为 Permanent"
    );
}

/// V3-F2-A06 (enc_failure)：永久分级 → recovery_action 返回 BackupAndConfirmRebuild
#[test]
fn enc_failure_permanent_tier_maps_to_backup_and_confirm_rebuild() {
    // Arrange
    let tier = FailureTier::Permanent;

    // Act
    let action = recovery_action(tier);

    // Assert：Permanent → 备份后需显式确认才重建
    assert_eq!(
        action,
        RecoveryAction::BackupAndConfirmRebuild,
        "Permanent 分级应映射到 BackupAndConfirmRebuild（改名备份+显式确认重建）"
    );
}

/// V3-F2-A06 (enc_failure)：永久失败 allow_rebuild=false → 备份存在且原内容保留（不静默删）
#[test]
fn enc_failure_permanent_backup_preserves_corrupt_content() {
    // Arrange：写入乱字节模拟损坏库
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let corrupt_data = b"enc_failure permanent test: corrupt bytes for sqlcipher";
    fs::write(&db_path, corrupt_data).expect("写损坏文件应成功");

    // Act：open_or_recover allow_rebuild=false（永久失败路径）
    let result = db::open_or_recover(&db_path, &KEY_A, false);

    // Assert 1：返回 Err（未重建空库）
    assert!(result.is_err(), "永久失败 allow_rebuild=false 应返回 Err");

    // Assert 2：备份文件存在
    let parent = db_path.parent().expect("路径有父目录");
    let backups: Vec<_> = fs::read_dir(parent)
        .expect("读目录应成功")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains("quickquick.db.corrupt-")
        })
        .collect();
    assert!(!backups.is_empty(), "永久失败应产生备份文件");

    // Assert 3：备份中保留原损坏内容（永不静默删）
    let backup_content = fs::read(backups[0].path()).expect("读备份文件应成功");
    assert_eq!(
        backup_content, corrupt_data,
        "备份文件应保留原损坏内容，不得静默删除或覆盖"
    );
}

/// V3-F2-A06 (enc_failure)：永久失败 allow_rebuild=true → 备份在 + 新空库可开
#[test]
fn enc_failure_permanent_allow_rebuild_creates_new_db_keeps_backup() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let corrupt_data = b"enc_failure rebuild test: corrupt content to backup";
    fs::write(&db_path, corrupt_data).expect("写损坏文件应成功");

    // Act：open_or_recover allow_rebuild=true（显式确认重建）
    let result = db::open_or_recover(&db_path, &KEY_A, true);

    // Assert 1：返回 Ok（新空库已建）
    assert!(
        result.is_ok(),
        "allow_rebuild=true 应返回 Ok，实际: {:?}",
        result.err()
    );
    drop(result);

    // Assert 2：备份文件存在，原损坏内容保留
    let parent = db_path.parent().expect("路径有父目录");
    let backups: Vec<_> = fs::read_dir(parent)
        .expect("读目录应成功")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains("quickquick.db.corrupt-")
        })
        .collect();
    assert!(!backups.is_empty(), "备份文件应存在");
    let backup_content = fs::read(backups[0].path()).expect("读备份文件应成功");
    assert_eq!(backup_content, corrupt_data, "备份应保留原损坏内容");

    // Assert 3：新空库可正常打开
    let conn2 = db::open_or_create(&db_path, &KEY_A).expect("新空库应可再次打开");
    drop(conn2);
}
