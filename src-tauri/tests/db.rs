//! 集成测试：加密数据库核心行为
//!
//! 覆盖验收项：
//! - V0-F3-A01 db_auto_create_on_first_run
//! - V0-F3-A02 db_encrypted_wrong_key_fails
//! - V0-F3-A06 db_corrupt_backup_not_deleted

use quickquick_lib::db;
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
