//! 加密数据库模块：SQLCipher 开库、schema 预埋、软删、GC、失败恢复
//!
//! 设计对齐：设计文档§六（失败/恢复语义）、§十（schema 预埋铁律）
//!
//! 核心函数：
//! - `open_or_create`  — 打开或新建加密库，并确保 schema 已初始化
//! - `open_or_recover` — 带损坏检测的打开：损坏时改名备份，按标志决定是否重建
//! - `soft_delete`     — 软删（置墓碑，非物理删）
//! - `gc_purge_deleted`— 本地物理清理 GC（删除 is_deleted=1 的行）
//!
//! 安全约定：
//! - 密钥以 raw key 格式传入（`PRAGMA key = "x'<hex>'"`)，不写入日志
//! - 打开后立即执行 `PRAGMA user_version` 轻量查询触发解密校验
//! - 永不静默删除数据库文件（设计§六硬约束）

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use thiserror::Error;

// ── 错误类型 ─────────────────────────────────────────────────────────────────

/// 数据库操作错误
#[derive(Debug, Error)]
pub enum DbError {
    /// SQLite / SQLCipher 操作失败（含解密失败）
    #[error("数据库操作失败：{0}")]
    Sqlite(#[from] rusqlite::Error),

    /// 文件 I/O 失败（改名备份、读写等）
    #[error("文件 I/O 失败：{0}")]
    Io(#[from] std::io::Error),

    /// 数据库文件损坏（解密失败或格式非法）
    #[error("数据库文件损坏，旧库已备份为 {backup_path}")]
    Corrupt {
        /// 备份文件路径（改名后的路径）
        backup_path: String,
    },
}

// ── 公共 API ─────────────────────────────────────────────────────────────────

/// 打开或新建加密数据库，确保 schema 已初始化。
///
/// # 密钥格式
/// 使用 SQLCipher raw key：`PRAGMA key = "x'<64-hex-chars>'"` 形式。
/// raw key 直接作为 AES-256 密钥材料，无需 PBKDF2 派生，与 KeyProvider 的
/// 32 字节密钥直接对应。
///
/// # 验证时机
/// 打开后立即执行 `PRAGMA user_version` 触发解密校验；错误密钥在此处暴露。
///
/// # Errors
/// - `DbError::Sqlite`：解密失败或 SQL 执行失败
/// - `DbError::Io`：路径目录无法创建
pub fn open_or_create(path: &Path, key: &[u8; 32]) -> Result<Connection, DbError> {
    let conn = open_with_key(path, key)?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// 带损坏检测的打开：损坏时改名备份，按 `allow_rebuild` 决定是否重建空库。
///
/// # 行为
/// 1. 尝试 `open_or_create`
/// 2. 成功 → 直接返回
/// 3. 判定为永久损坏（解密/格式错误）→ 改名备份（绝不删除原内容）
///    - `allow_rebuild=false` → 返回 `DbError::Corrupt`（不建空库）
///    - `allow_rebuild=true`  → 备份后重建新空库并返回 `Ok`
///
/// # 设计§六硬约束
/// 永不静默删除或覆盖原文件；备份命名格式：`<原名>.corrupt-<utc_secs>`。
///
/// # Errors
/// - `DbError::Corrupt`：损坏且 `allow_rebuild=false`
/// - `DbError::Io`：改名或重建时 I/O 失败
/// - `DbError::Sqlite`：重建后 schema 初始化失败
pub fn open_or_recover(
    path: &Path,
    key: &[u8; 32],
    allow_rebuild: bool,
) -> Result<Connection, DbError> {
    match open_or_create(path, key) {
        Ok(conn) => return Ok(conn),
        Err(DbError::Sqlite(_)) => {
            // 解密/格式失败 → 视为永久损坏，走备份流程
        }
        Err(other) => return Err(other),
    }

    let backup_path = backup_corrupt_file(path)?;

    if !allow_rebuild {
        return Err(DbError::Corrupt {
            backup_path: backup_path.to_string_lossy().into_owned(),
        });
    }

    // allow_rebuild=true：备份已完成，重建新空库
    let conn = open_or_create(path, key)?;
    Ok(conn)
}

/// 软删一条记录（置墓碑，不物理删除）。
///
/// 更新字段：`is_deleted=1`、`deleted_at_utc=now`、`last_modified_utc=now`。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败（含 id 不存在时影响行数为 0，不报错）
pub fn soft_delete(conn: &Connection, id: &str) -> Result<(), DbError> {
    let now_ms = current_utc_ms();
    conn.execute(
        "UPDATE clip_items
         SET is_deleted = 1,
             deleted_at_utc = ?1,
             last_modified_utc = ?2
         WHERE id = ?3",
        rusqlite::params![now_ms, now_ms, id],
    )?;
    Ok(())
}

/// 物理清理 GC：删除所有 `is_deleted=1` 的行，返回清理条数。
///
/// 仅删除已标记软删的行；正常行不受影响。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn gc_purge_deleted(conn: &Connection) -> Result<u64, DbError> {
    let count = conn.execute(
        "DELETE FROM clip_items WHERE is_deleted = 1",
        [],
    )?;
    Ok(count as u64)
}

// ── 内部辅助函数 ──────────────────────────────────────────────────────────────

/// 用指定 key 打开 SQLCipher 数据库并触发解密校验。
///
/// 开库成功后立即开启外键约束（`PRAGMA foreign_keys = ON`）。
/// rusqlite/SQLite 每条连接默认外键为 OFF，必须在每次开库后显式启用。
fn open_with_key(path: &Path, key: &[u8; 32]) -> Result<Connection, DbError> {
    // 确保父目录存在（首次启动时目录可能未创建）
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;

    // I-01：SQLCipher 的 PRAGMA key 不支持 `?` 参数化（上游约束，SQLCipher 协议限制）。
    // 此处字符串构造为代码库中唯一的参数化例外：
    //   - hex_key 来自受控 [u8;32]，内容仅 64 位十六进制字符（0-9/a-f），不可注入；
    //   - 未启用 SQLite trace 钩子，故 PRAGMA 语句不会写入日志或泄漏密钥材料。
    let hex_key = hex_encode(key);
    conn.execute_batch(&format!("PRAGMA key = \"x'{hex_key}'\";" ))?;

    // 轻量查询触发解密校验：错误密钥在此暴露，避免延迟到写操作时才报错
    conn.execute_batch("PRAGMA user_version;")?;

    // I-03：每条连接必须显式开启外键约束（SQLite 默认 OFF）
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    Ok(conn)
}

/// 初始化 schema（幂等，使用 IF NOT EXISTS）。
///
/// 预埋字段遵循设计§十铁律：UUID 主键、UTC 时间戳、last_modified、墓碑、图片表拆分。
///
/// I-03（§十预埋）：
/// - `ensure_schema` 内再次确保 foreign_keys = ON（防止调用方绕过 open_with_key 直连）。
/// - `clip_images.clip_item_id` 声明外键约束，并设 ON DELETE CASCADE：
///   GC 物理删除 clip_items 行时，关联图片自动级联清理（设计§五分级清理语义）。
fn ensure_schema(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        -- 剪贴板条目表
        CREATE TABLE IF NOT EXISTS clip_items (
            id                TEXT PRIMARY KEY NOT NULL,  -- UUID，永不复用
            content           TEXT,
            kind              TEXT NOT NULL DEFAULT 'text',
            created_utc       INTEGER NOT NULL,            -- UTC epoch ms
            last_modified_utc INTEGER NOT NULL,            -- UTC epoch ms
            is_deleted        INTEGER NOT NULL DEFAULT 0,  -- 墓碑：0=正常 1=软删
            deleted_at_utc    INTEGER                      -- 软删时间（UTC epoch ms）
        );

        -- 图片表：缩略图/原图拆分（设计§五/§十）
        -- ON DELETE CASCADE：GC 删除 clip_items 时级联清理关联图片（§五分级清理）
        CREATE TABLE IF NOT EXISTS clip_images (
            id                TEXT PRIMARY KEY NOT NULL,  -- UUID，永不复用
            clip_item_id      TEXT REFERENCES clip_items(id) ON DELETE CASCADE,
            thumbnail         BLOB,                       -- 缩略图 BLOB
            original          BLOB,                       -- 原图 BLOB
            original_present  INTEGER NOT NULL DEFAULT 0, -- 1=有原图 0=仅缩略图（降级态）
            created_utc       INTEGER NOT NULL,            -- UTC epoch ms
            last_modified_utc INTEGER NOT NULL,            -- UTC epoch ms
            is_deleted        INTEGER NOT NULL DEFAULT 0,  -- 墓碑
            deleted_at_utc    INTEGER                      -- 软删时间
        );
        ",
    )?;
    Ok(())
}

/// 将损坏文件改名备份，返回备份路径。
///
/// 备份格式：`<原路径>.corrupt-<utc_secs>`。
/// 永不删除原内容，仅 rename（OS 层原子操作）。
fn backup_corrupt_file(path: &Path) -> Result<std::path::PathBuf, DbError> {
    let utc_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let original_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let backup_name = format!("{}.corrupt-{}", original_name, utc_secs);

    let backup_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(backup_name);

    std::fs::rename(path, &backup_path)?;
    Ok(backup_path)
}

/// 获取当前 UTC 时间戳（毫秒）。
fn current_utc_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// 将字节切片编码为小写十六进制字符串。
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ── 模块内单元测试 ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encode_32_bytes_produces_64_chars() {
        let key = [0xabu8; 32];
        let hex = hex_encode(&key);
        assert_eq!(hex.len(), 64, "32 字节应编码为 64 个十六进制字符");
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()), "应全为十六进制字符");
    }

    #[test]
    fn hex_encode_known_value() {
        let key = [0x07u8; 32];
        let hex = hex_encode(&key);
        assert_eq!(&hex[..2], "07", "0x07 应编码为 '07'");
    }
}
