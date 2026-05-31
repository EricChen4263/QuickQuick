//! 加密数据库模块：SQLCipher 开库、schema 预埋、软删、GC、失败恢复、去重入库
//!
//! 设计对齐：设计文档§六（失败/恢复语义）、§十（schema 预埋铁律）、§三（去重+置顶刷新）
//!           §九.2（★ 置顶收藏）、§五（收藏永远豁免清理）
//!
//! 核心函数：
//! - `open_or_create`       — 打开或新建加密库，并确保 schema 已初始化
//! - `open_or_recover`      — 带损坏检测的打开：损坏时改名备份，按标志决定是否重建
//! - `soft_delete`          — 软删（置墓碑，非物理删）
//! - `gc_purge_deleted`     — 本地物理清理 GC（删除 is_deleted=1 的行）
//! - `ingest`               — 去重入库：同 text_hash 存在则 bump，否则插入新行
//! - `bump_to_top`          — 显式置顶：仅更新 last_modified_utc（不新建记录）
//! - `count_live`           — 查询未软删行数（测试/业务用）
//! - `top_id`               — 返回 last_modified_utc 最新的未软删行 id（测试/业务用）
//! - `set_favorite`         — 设置/取消收藏（is_favorite 字段）
//! - `list_ordered`         — 返回排序后的条目列表（收藏优先，组内按最近）
//! - `cleanup_keep_recent`  — 容量裁剪：只删非收藏的旧项，收藏永远豁免
//!
//! 安全约定：
//! - 密钥以 raw key 格式传入（`PRAGMA key = "x'<hex>'"`)，不写入日志
//! - 打开后立即执行 `PRAGMA user_version` 轻量查询触发解密校验
//! - 永不静默删除数据库文件（设计§六硬约束）

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};
use thiserror::Error;
use uuid::Uuid;

use crate::clipboard::CapturedItem;

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

/// 列表排序后的条目行（list_ordered 返回元素）。
///
/// 含业务排序所需的最小字段集。
#[derive(Debug, PartialEq)]
pub struct ClipRow {
    /// 条目 UUID
    pub id: String,
    /// 收藏标记（true = 收藏）
    pub is_favorite: bool,
    /// 最后修改时间（UTC epoch ms）
    pub last_modified_utc: i64,
}

/// 设置或取消收藏。
///
/// 同时刷新 `last_modified_utc`，使收藏操作反映到时间线。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败（id 不存在时影响行数为 0，不报错）
pub fn set_favorite(conn: &Connection, id: &str, fav: bool) -> Result<(), DbError> {
    let now_ms = current_utc_ms();
    let fav_int: i64 = if fav { 1 } else { 0 };
    conn.execute(
        "UPDATE clip_items
         SET is_favorite = ?1,
             last_modified_utc = ?2
         WHERE id = ?3 AND is_deleted = 0",
        rusqlite::params![fav_int, now_ms, id],
    )?;
    Ok(())
}

/// 返回未软删条目的排序列表：收藏优先，组内按最近修改时间降序。
///
/// 排序规则：`ORDER BY is_favorite DESC, last_modified_utc DESC`
/// - 收藏项（is_favorite=1）整体排在非收藏项（is_favorite=0）之前
/// - 同组内按 last_modified_utc 从新到旧排列
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn list_ordered(conn: &Connection) -> Result<Vec<ClipRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, is_favorite, last_modified_utc
         FROM clip_items
         WHERE is_deleted = 0
         ORDER BY is_favorite DESC, last_modified_utc DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let is_fav_int: i64 = row.get(1)?;
        let last_modified: i64 = row.get(2)?;
        Ok(ClipRow {
            id,
            is_favorite: is_fav_int != 0,
            last_modified_utc: last_modified,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 容量裁剪：只删非收藏的旧项，收藏项永远豁免。
///
/// 语义：非收藏的未软删条目中，按 last_modified_utc 降序保留最新 `keep_count` 条，
/// 超出部分软删（置墓碑）。收藏项（is_favorite=1）**完全不计入也不删除**。
///
/// 选择软删而非物理删：与 `soft_delete` 语义一致（设计§六），GC 负责后续物理清理。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn cleanup_keep_recent(conn: &Connection, keep_count: usize) -> Result<usize, DbError> {
    let now_ms = current_utc_ms();
    // 查询需要被软删的非收藏旧项 id：跳过最新的 keep_count 条，删除余下的
    let mut stmt = conn.prepare(
        "SELECT id FROM clip_items
         WHERE is_deleted = 0 AND is_favorite = 0
         ORDER BY last_modified_utc DESC",
    )?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut all_ids: Vec<String> = Vec::new();
    for r in rows {
        all_ids.push(r?);
    }

    // 超出 keep_count 的部分才需要清理
    let to_delete = if all_ids.len() > keep_count {
        &all_ids[keep_count..]
    } else {
        return Ok(0);
    };

    let mut deleted = 0usize;
    for id in to_delete {
        conn.execute(
            "UPDATE clip_items
             SET is_deleted = 1,
                 deleted_at_utc = ?1,
                 last_modified_utc = ?1
             WHERE id = ?2",
            rusqlite::params![now_ms, id],
        )?;
        deleted += 1;
    }
    Ok(deleted)
}

/// `ingest` 的返回结果：新建行 vs 原行置顶刷新。
#[derive(Debug, PartialEq)]
pub enum IngestOutcome {
    /// 内容不存在，插入了新行；携带新行的 id
    Inserted(String),
    /// 内容已存在，原行已置顶刷新（last_modified_utc 已更新）；携带原行 id
    Bumped(String),
}

/// 去重入库：将捕获到的 `CapturedItem` 写入数据库，自动去重。
///
/// 流程：
/// 1. 计算 `item.text` 的 `text_hash`
/// 2. 查询 `clip_items` 中是否有未软删的同 hash 行
/// 3. 命中 → 调用 `bump_to_top`（刷新 `last_modified_utc`），返回 `Bumped(id)`
/// 4. 未命中 → 插入新行，返回 `Inserted(id)`
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn ingest(conn: &Connection, item: &CapturedItem) -> Result<IngestOutcome, DbError> {
    let hash = text_hash(&item.text);
    let now_ms = current_utc_ms();

    // 查询是否已有未软删的同 hash 行
    let existing_id: Option<String> = conn
        .query_row(
            "SELECT id FROM clip_items WHERE text_hash = ?1 AND is_deleted = 0 LIMIT 1",
            rusqlite::params![hash],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    if let Some(id) = existing_id {
        // 命中：置顶刷新，不新建行
        bump_to_top(conn, &id)?;
        return Ok(IngestOutcome::Bumped(id));
    }

    // 未命中：插入新行
    let new_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO clip_items
             (id, content, kind, created_utc, last_modified_utc, is_deleted, text_hash)
         VALUES (?1, ?2, 'text', ?3, ?3, 0, ?4)",
        rusqlite::params![new_id, item.text, now_ms, hash],
    )?;

    Ok(IngestOutcome::Inserted(new_id))
}

/// 显式置顶：仅更新 `last_modified_utc = now`，不新建任何记录。
///
/// 列表按 `last_modified_utc DESC` 排序时，该行将排到最前。
/// 业务语义：「置顶」由本函数**显式改库**实现，绝不通过重新捕获产生新记录。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败（id 不存在时影响行数为 0，不报错）
pub fn bump_to_top(conn: &Connection, id: &str) -> Result<(), DbError> {
    let now_ms = current_utc_ms();
    conn.execute(
        "UPDATE clip_items SET last_modified_utc = ?1 WHERE id = ?2",
        rusqlite::params![now_ms, id],
    )?;
    Ok(())
}

/// 返回未软删的行数。
///
/// 用于测试断言和业务层「历史条目总数」查询。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn count_live(conn: &Connection) -> Result<i64, DbError> {
    let n = conn.query_row(
        "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 0",
        [],
        |row| row.get(0),
    )?;
    Ok(n)
}

/// 返回 `last_modified_utc` 最新的未软删行的 `id`（即当前「最前」条目）。
///
/// 用于测试断言和业务层「取最新条目」查询。
/// 库为空或全部软删时返回 `Ok(None)`。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn top_id(conn: &Connection) -> Result<Option<String>, DbError> {
    let result = conn
        .query_row(
            "SELECT id FROM clip_items WHERE is_deleted = 0
             ORDER BY last_modified_utc DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(result)
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
            deleted_at_utc    INTEGER,                     -- 软删时间（UTC epoch ms）
            text_hash         TEXT,                        -- 纯文本内容哈希，用于去重（非加密，判重用途）
            is_favorite       INTEGER NOT NULL DEFAULT 0   -- 收藏标记：0=普通 1=收藏（§九.2 ★置顶收藏）
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

        -- provider 非密凭据表（§十预埋铁律）
        -- secret 字段绝不入库（路由到 keychain），此表只存 is_secret=false 的字段
        CREATE TABLE IF NOT EXISTS provider_config (
            provider_id  TEXT NOT NULL,
            field_key    TEXT NOT NULL,
            value        TEXT NOT NULL,
            PRIMARY KEY (provider_id, field_key)
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

/// 计算文本判重哈希（显式稳定 FNV-1a 64-bit，跨 Rust 版本/构建一致；非加密，仅用于内容去重）。
fn text_hash(text: &str) -> String {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
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
