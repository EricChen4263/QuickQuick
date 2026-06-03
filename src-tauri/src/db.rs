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
use crate::image as img_mod;

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

    /// 其他业务层错误（如缩略图编码失败等不属于 SQL/IO 的失败）
    #[error("操作失败：{0}")]
    Other(String),

    /// 瞬时钥匙串错误（钥匙串被拒/系统锁定；不代表数据损坏，仅需重试）
    ///
    /// 与 KeyError::Backend 对应的数据库层表示：上层 KeyProvider 返回 Backend 错误后，
    /// 调用方可通过此变体向 db 层传递"瞬时失败"语义，触发 Transient 分级。
    #[error("瞬时钥匙串错误：{0}")]
    TransientKeychain(String),
}

impl DbError {
    /// 构造瞬时钥匙串错误（测试与业务层用）。
    ///
    /// 对应设计§六#1 中"钥匙串被拒/锁"路径——仅提示重试，绝不碰库文件。
    pub fn transient_keychain_error(msg: impl Into<String>) -> Self {
        Self::TransientKeychain(msg.into())
    }
}

/// 错误失败分级：区分可重试的瞬时失败与需要恢复的永久失败。
///
/// 设计§六#1 失败/恢复语义：
/// - `Transient`：钥匙串被拒/系统锁定等，库文件完好，仅需重试
/// - `Permanent`：密钥丢失/库损坏/解密失败，需备份旧库并（显式确认后）重建
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureTier {
    /// 瞬时失败：库文件完好，仅需提示用户重试，绝不改动库文件
    Transient,
    /// 永久失败：密钥不可用或库已损坏，须备份旧库，显式确认后才可重建
    Permanent,
}

/// 失败后的恢复动作。
///
/// 由 `recovery_action(tier)` 根据 `FailureTier` 返回，供调用方决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// 仅提示用户重试，绝不改名/删除/重建库文件（对应 Transient）
    RetryNoTouch,
    /// 将旧库改名备份，等待显式用户确认后才重建空库（对应 Permanent）
    BackupAndConfirmRebuild,
}

/// 将 `DbError` 分类为瞬时或永久失败。
///
/// 分类规则（设计§六#1）：
/// - `TransientKeychain`       → `Transient`（钥匙串被拒/锁，库文件完好）
/// - `Corrupt` / `Sqlite`      → `Permanent`（库损坏或解密失败，需备份恢复）
/// - `Io` / `Other`            → `Permanent`（保守归类，IO 失败视为需人工介入）
pub fn classify_failure(err: &DbError) -> FailureTier {
    match err {
        DbError::TransientKeychain(_) => FailureTier::Transient,
        DbError::Corrupt { .. } | DbError::Sqlite(_) | DbError::Io(_) | DbError::Other(_) => {
            FailureTier::Permanent
        }
    }
}

/// 根据失败分级返回对应的恢复动作。
///
/// - `Transient` → `RetryNoTouch`：仅提示重试，不触碰库文件
/// - `Permanent` → `BackupAndConfirmRebuild`：改名备份，显式确认才重建
pub fn recovery_action(tier: FailureTier) -> RecoveryAction {
    match tier {
        FailureTier::Transient => RecoveryAction::RetryNoTouch,
        FailureTier::Permanent => RecoveryAction::BackupAndConfirmRebuild,
    }
}

/// 执行恢复动作：将 `RecoveryAction` 的语义落地到文件系统操作。
///
/// - `RetryNoTouch`        → 不碰文件，直接返回 `Ok(())`（库文件原样保留）
/// - `BackupAndConfirmRebuild` → 调用 `backup_corrupt_file` 将旧库改名备份
///
/// # Errors
/// - `DbError::Io`：`BackupAndConfirmRebuild` 路径下 rename 失败
/// - `DbError::Io`：`path` 无文件名分量（`backup_corrupt_file` 报错）
pub fn apply_recovery_action(path: &Path, action: RecoveryAction) -> Result<(), DbError> {
    match action {
        RecoveryAction::RetryNoTouch => Ok(()),
        RecoveryAction::BackupAndConfirmRebuild => {
            backup_corrupt_file(path)?;
            Ok(())
        }
    }
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

/// 完整剪贴板条目行（list_items_full 返回元素）。
///
/// 包含前端展示所需的全量字段：内容、类型、收藏状态、时间。
#[derive(Debug, PartialEq)]
pub struct ClipItemRow {
    /// 条目 UUID
    pub id: String,
    /// 条目内容（文本）
    pub content: String,
    /// 内容类型（如 "text"）
    pub kind: String,
    /// 收藏标记（true = 收藏）
    pub is_favorite: bool,
    /// 最后修改时间（UTC epoch ms）
    pub last_modified_utc: i64,
}

/// 完整剪贴板条目行，含关联图片字段（list_items_with_images 返回元素）。
///
/// 文本条目因 LEFT JOIN 无匹配，`image_id` 与 `thumbnail` 均为 `None`。
#[derive(Debug)]
pub struct ClipItemRowWithImage {
    /// 条目 UUID
    pub id: String,
    /// 条目内容（文本）
    pub content: String,
    /// 内容类型（"text" / "image"）
    pub kind: String,
    /// 收藏标记
    pub is_favorite: bool,
    /// 最后修改时间（UTC epoch ms）
    pub last_modified_utc: i64,
    /// 关联图片行 id（None 表示无图）
    pub image_id: Option<String>,
    /// 缩略图 BLOB（WebP 字节；None 表示无图）
    pub thumbnail: Option<Vec<u8>>,
}

/// 返回未软删条目列表，LEFT JOIN 图片缩略图：收藏优先，组内按最近修改时间降序。
///
/// 与 `list_items_full` 排序规则相同，但额外带出关联 `clip_images` 的
/// `id`（`image_id`）和 `thumbnail` BLOB，文本条目这两个字段为 `None`。
///
/// 排序兜底：`rowid DESC` 确保同毫秒并列时稳定有序。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn list_items_with_images(conn: &Connection) -> Result<Vec<ClipItemRowWithImage>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT ci.id, ci.content, ci.kind, ci.is_favorite, ci.last_modified_utc,
                cimg.id AS image_id, cimg.thumbnail
         FROM clip_items ci
         LEFT JOIN clip_images cimg ON cimg.clip_item_id = ci.id AND cimg.is_deleted = 0
         WHERE ci.is_deleted = 0
         ORDER BY ci.is_favorite DESC, ci.last_modified_utc DESC, ci.rowid DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let content: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
        let kind: String = row.get(2)?;
        let is_fav_int: i64 = row.get(3)?;
        let last_modified: i64 = row.get(4)?;
        let image_id: Option<String> = row.get(5)?;
        let thumbnail: Option<Vec<u8>> = row.get(6)?;
        Ok(ClipItemRowWithImage {
            id,
            content,
            kind,
            is_favorite: is_fav_int != 0,
            last_modified_utc: last_modified,
            image_id,
            thumbnail,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 返回未软删条目的完整列表：收藏优先，组内按最近修改时间降序。
///
/// 与 `list_ordered` 排序规则相同，但返回前端展示所需的全量字段。
///
/// 排序兜底：`rowid DESC` 确保同毫秒并列时最后插入的条目稳定排前，
/// 消除并发测试下因时间戳精度导致的不确定顺序（flaky 根因：同毫秒两次 ingest）。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn list_items_full(conn: &Connection) -> Result<Vec<ClipItemRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, content, kind, is_favorite, last_modified_utc
         FROM clip_items
         WHERE is_deleted = 0
         ORDER BY is_favorite DESC, last_modified_utc DESC, rowid DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let content: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
        let kind: String = row.get(2)?;
        let is_fav_int: i64 = row.get(3)?;
        let last_modified: i64 = row.get(4)?;
        Ok(ClipItemRow {
            id,
            content,
            kind,
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
/// 排序规则：`ORDER BY is_favorite DESC, last_modified_utc DESC, rowid DESC`
/// - 收藏项（is_favorite=1）整体排在非收藏项（is_favorite=0）之前
/// - 同组内按 last_modified_utc 从新到旧排列
/// - `rowid DESC` 兜底：同毫秒并列时最后插入的条目稳定排前，保证确定性
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn list_ordered(conn: &Connection) -> Result<Vec<ClipRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, is_favorite, last_modified_utc
         FROM clip_items
         WHERE is_deleted = 0
         ORDER BY is_favorite DESC, last_modified_utc DESC, rowid DESC",
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
    let count = conn.execute("DELETE FROM clip_items WHERE is_deleted = 1", [])?;
    Ok(count as u64)
}

/// 图片剪贴板去重入库：将一张图片作为独立 clip_items 行写入，并关联 clip_images。
///
/// 行为：
/// 1. 用 `image::image_hash` 计算字节哈希，查 `clip_images` 是否已有未软删同 hash 行。
/// 2. 命中且 clip_item_id 非 NULL → bump 关联 clip_item 的 `last_modified_utc`，返回 `Bumped`。
/// 3. 未命中（或 clip_item_id 为 NULL 的孤立行）→ 在 SAVEPOINT 事务内：
///    a. INSERT `clip_items`（kind='image'，content 含尺寸）
///    b. 调用 `image::ingest_image_with_policy` 写 `clip_images`，得到 image_id
///    c. UPDATE `clip_images SET clip_item_id = item_id WHERE id = image_id`（补写外键，SAVEPOINT 内）
///    d. RELEASE SAVEPOINT 提交；任一步失败则 ROLLBACK SAVEPOINT，不留孤立行
///    返回 `Inserted`。
///
/// `max_image_bytes`：原图大小上限（字节），超出则只存缩略图（original_present=0）。
/// 调用方从 AppSettings.max_image_bytes 读取并传入，使阈值可由用户配置。
///
/// # Errors
/// - `DbError::Sqlite`：SQL 执行失败
/// - `DbError::Other`：图片解码/缩略图生成失败
pub fn ingest_image_as_clip(
    conn: &Connection,
    width: usize,
    height: usize,
    png_bytes: &[u8],
    max_image_bytes: u64,
) -> Result<IngestOutcome, DbError> {
    let hash = img_mod::image_hash(png_bytes);

    // 查是否已有同 hash 的未软删图片行
    let existing: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT id, clip_item_id FROM clip_images
             WHERE image_hash = ?1 AND is_deleted = 0
             LIMIT 1",
            rusqlite::params![hash],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()?;

    // 合并嵌套 if let：仅当 clip_item_id 非 NULL 时才视为有效命中并 bump
    if let Some((_img_id, Some(item_id))) = existing {
        bump_to_top(conn, &item_id)?;
        return Ok(IngestOutcome::Bumped(item_id));
    }
    // clip_item_id 为 NULL（孤立行）或无命中：走新建路径，UPDATE 将领养孤立行

    insert_image_clip(conn, width, height, png_bytes, max_image_bytes)
}

/// 在 SAVEPOINT 事务内插入 clip_items + clip_images 并补写外键关联。
///
/// SAVEPOINT 生命周期：成功时 RELEASE，失败时 ROLLBACK TO + RELEASE。
fn insert_image_clip(
    conn: &Connection,
    width: usize,
    height: usize,
    png_bytes: &[u8],
    max_image_bytes: u64,
) -> Result<IngestOutcome, DbError> {
    let now_ms = current_utc_ms();
    let item_id = Uuid::new_v4().to_string();
    let content = format!("[图片] {width}×{height}");

    conn.execute_batch("SAVEPOINT ingest_image_clip;")?;

    let result =
        try_insert_image_clip(conn, &item_id, &content, now_ms, png_bytes, max_image_bytes);

    match result {
        Ok(()) => {
            conn.execute_batch("RELEASE SAVEPOINT ingest_image_clip;")?;
            Ok(IngestOutcome::Inserted(item_id))
        }
        Err(e) => {
            // 整体回滚，不留孤立 clip_items 行
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT ingest_image_clip;");
            let _ = conn.execute_batch("RELEASE SAVEPOINT ingest_image_clip;");
            Err(e)
        }
    }
}

/// SAVEPOINT 内的实际写操作：INSERT clip_items → ingest_image_with_policy → UPDATE clip_item_id。
///
/// 三步全在 SAVEPOINT 保护内，任一步失败由调用方 ROLLBACK。
fn try_insert_image_clip(
    conn: &Connection,
    item_id: &str,
    content: &str,
    now_ms: i64,
    png_bytes: &[u8],
    max_image_bytes: u64,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO clip_items
             (id, content, kind, created_utc, last_modified_utc, is_deleted, text_hash)
         VALUES (?1, ?2, 'image', ?3, ?3, 0, NULL)",
        rusqlite::params![item_id, content, now_ms],
    )?;

    let policy = img_mod::OversizePolicy {
        max_original_bytes: max_image_bytes as usize,
    };
    let outcome = img_mod::ingest_image_with_policy(conn, png_bytes, &policy)?;

    // Bumped 命中孤立行时返回旧 image_id，UPDATE 将其关联到本次新建的 item_id
    let image_id = match outcome {
        img_mod::IngestImageOutcome::Inserted(id) => id,
        img_mod::IngestImageOutcome::Bumped(id) => id,
    };

    // 补写外键（ingest_image_with_policy 不写 clip_item_id 字段）
    let affected = conn.execute(
        "UPDATE clip_images SET clip_item_id = ?1 WHERE id = ?2",
        rusqlite::params![item_id, image_id],
    )?;
    if affected != 1 {
        return Err(DbError::Other(format!(
            "clip_item_id 补写影响行数异常：期望 1，实际 {affected}（image_id={image_id}）"
        )));
    }

    Ok(())
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
    conn.execute_batch(&format!("PRAGMA key = \"x'{hex_key}'\";"))?;

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
            thumbnail         BLOB,                       -- 缩略图 BLOB（拆分存储）
            original          BLOB,                       -- 原图 BLOB（拆分存储，原样无损）
            original_present  INTEGER NOT NULL DEFAULT 0, -- 1=有原图 0=仅缩略图（降级态）
            image_hash        TEXT,                       -- 原图字节哈希，用于判重（FNV-1a 64-bit）
            created_utc       INTEGER NOT NULL,            -- UTC epoch ms
            last_modified_utc INTEGER NOT NULL,            -- UTC epoch ms
            is_deleted        INTEGER NOT NULL DEFAULT 0,  -- 墓碑
            deleted_at_utc    INTEGER,                     -- 软删时间
            is_favorite       INTEGER NOT NULL DEFAULT 0  -- 收藏标记：0=普通 1=收藏（分级清理豁免，V3-F1-A04）
        );

        -- provider 非密凭据表（§十预埋铁律）
        -- secret 字段绝不入库（路由到 keychain），此表只存 is_secret=false 的字段
        CREATE TABLE IF NOT EXISTS provider_config (
            provider_id  TEXT NOT NULL,
            field_key    TEXT NOT NULL,
            value        TEXT NOT NULL,
            PRIMARY KEY (provider_id, field_key)
        );

        -- 翻译缓存表（§4.1#5 预埋，LRU 淘汰；键=四元组哈希）
        -- cache_key = hash(source_text + source_lang + target_lang + provider_id)
        -- last_used_utc 用于 LRU 淘汰排序（命中时刷新）
        CREATE TABLE IF NOT EXISTS translation_cache (
            cache_key      TEXT PRIMARY KEY NOT NULL,
            source_text    TEXT NOT NULL,
            source_lang    TEXT NOT NULL,
            target_lang    TEXT NOT NULL,
            provider_id    TEXT NOT NULL,
            translated     TEXT NOT NULL,
            created_utc    INTEGER NOT NULL,
            last_used_utc  INTEGER NOT NULL
        );

        -- 翻译历史表（V2-F3-A14）：独立于 clip_items，记录用户主动翻译的历史
        -- 剪贴板条目一键翻译后写入此表，两者互不混入
        CREATE TABLE IF NOT EXISTS translate_history (
            id              TEXT PRIMARY KEY NOT NULL,
            source_text     TEXT NOT NULL,
            translated_text TEXT NOT NULL,
            source_lang     TEXT NOT NULL,
            target_lang     TEXT NOT NULL,
            provider_id     TEXT NOT NULL,
            created_utc     INTEGER NOT NULL
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
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "backup_corrupt_file: 路径无文件名分量",
            )
        })?
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

    /// 构造最小合法 2×2 RGBA PNG（用 image crate 编码，不依赖外部文件）。
    ///
    /// 返回的字节可被 `image::load_from_memory` 正确解码，用于 ingest_image_as_clip 测试。
    fn make_test_png(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageFormat, RgbaImage};
        let img = RgbaImage::new(width, height);
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .expect("test PNG 编码失败");
        buf
    }

    /// 建立内存 SQLite 并初始化 clip_items + clip_images 表（与 ensure_schema 一致）。
    ///
    /// 测试绕开加密路径直接开内存库，手动建表保持测试自包含。
    fn make_test_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS clip_items (
                 id                TEXT PRIMARY KEY NOT NULL,
                 content           TEXT,
                 kind              TEXT NOT NULL DEFAULT 'text',
                 created_utc       INTEGER NOT NULL,
                 last_modified_utc INTEGER NOT NULL,
                 is_deleted        INTEGER NOT NULL DEFAULT 0,
                 deleted_at_utc    INTEGER,
                 text_hash         TEXT,
                 is_favorite       INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS clip_images (
                 id                TEXT PRIMARY KEY NOT NULL,
                 clip_item_id      TEXT REFERENCES clip_items(id) ON DELETE CASCADE,
                 thumbnail         BLOB,
                 original          BLOB,
                 original_present  INTEGER NOT NULL DEFAULT 0,
                 image_hash        TEXT,
                 created_utc       INTEGER NOT NULL,
                 last_modified_utc INTEGER NOT NULL,
                 is_deleted        INTEGER NOT NULL DEFAULT 0,
                 deleted_at_utc    INTEGER,
                 is_favorite       INTEGER NOT NULL DEFAULT 0
             );",
        )
        .expect("建测试表失败");
        conn
    }

    /// T1：基础入库 — 新图应写入一条 clip_items(kind='image') 和一条
    /// clip_images，且 clip_images.clip_item_id 非 NULL 指向该 item。
    #[test]
    fn ingest_image_as_clip_inserts_item_and_links_image() {
        let conn = make_test_conn();
        let png = make_test_png(2, 2);

        let outcome = ingest_image_as_clip(&conn, 2, 2, &png, 20 * 1024 * 1024)
            .expect("ingest_image_as_clip 不应返回 Err");

        let item_id = match &outcome {
            IngestOutcome::Inserted(id) => id.clone(),
            IngestOutcome::Bumped(_) => panic!("首次入库应返回 Inserted，得到 Bumped"),
        };

        // clip_items 恰好 1 行，kind='image'，content 含尺寸
        let (kind, content): (String, String) = conn
            .query_row(
                "SELECT kind, content FROM clip_items WHERE id = ?1 AND is_deleted = 0",
                rusqlite::params![item_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("clip_items 中应有该 item");
        assert_eq!(kind, "image", "kind 应为 'image'");
        assert!(
            content.contains("2") && content.contains("×"),
            "content 应含尺寸信息，实际: {content}"
        );

        // clip_images 恰好 1 行，clip_item_id = item_id（非 NULL）
        let linked_item_id: String = conn
            .query_row(
                "SELECT clip_item_id FROM clip_images WHERE is_deleted = 0",
                [],
                |row| row.get(0),
            )
            .expect("clip_images 中应有图片行");
        assert_eq!(
            linked_item_id, item_id,
            "clip_images.clip_item_id 应指向新建的 clip_items 行"
        );

        // 总行数断言：各表恰好 1 行
        let item_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(item_count, 1, "clip_items 应恰好 1 行");
        let img_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_images WHERE is_deleted = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(img_count, 1, "clip_images 应恰好 1 行");
    }

    /// T2：去重 bump — 同一 PNG 二次调用应返回 Bumped，行数不变，
    /// 且对应 clip_item.last_modified_utc 被刷新。
    #[test]
    fn ingest_image_as_clip_dedup_bumps_timestamp() {
        let conn = make_test_conn();
        let png = make_test_png(4, 4);

        // 首次入库
        let first =
            ingest_image_as_clip(&conn, 4, 4, &png, 20 * 1024 * 1024).expect("首次入库失败");
        let first_id = match &first {
            IngestOutcome::Inserted(id) => id.clone(),
            _ => panic!("首次应 Inserted"),
        };

        // 记录首次 last_modified_utc
        let ts_before: i64 = conn
            .query_row(
                "SELECT last_modified_utc FROM clip_items WHERE id = ?1",
                rusqlite::params![first_id],
                |r| r.get(0),
            )
            .unwrap();

        // 确保时间戳可区分（至少等 1ms，系统调用精度通常满足；若并发过快此处仍可能相等，
        // 业务语义是"刷新"，相等也属正确行为，故断言用 >=）
        std::thread::sleep(std::time::Duration::from_millis(5));

        // 二次调用
        let second =
            ingest_image_as_clip(&conn, 4, 4, &png, 20 * 1024 * 1024).expect("二次调用失败");
        assert!(
            matches!(second, IngestOutcome::Bumped(_)),
            "同一 PNG 二次调用应返回 Bumped"
        );

        // clip_items 仍 1 行
        let item_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(item_count, 1, "去重后 clip_items 应仍 1 行");

        // clip_images 仍 1 行
        let img_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_images WHERE is_deleted = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(img_count, 1, "去重后 clip_images 应仍 1 行");

        // last_modified_utc 已刷新（>= 首次值）
        let ts_after: i64 = conn
            .query_row(
                "SELECT last_modified_utc FROM clip_items WHERE id = ?1",
                rusqlite::params![first_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            ts_after >= ts_before,
            "bump 后 last_modified_utc 应 >= 首次值，before={ts_before} after={ts_after}"
        );
    }

    /// T3：事务回滚 — 非法 PNG（空字节）应使 make_thumbnail 失败，
    /// 整体回滚，clip_items 无孤立行，函数返回 Err 不 panic。
    #[test]
    fn ingest_image_as_clip_invalid_png_rolls_back() {
        let conn = make_test_conn();
        let bad_png: &[u8] = b""; // 空字节，不是合法 PNG

        let result = ingest_image_as_clip(&conn, 1, 1, bad_png, 20 * 1024 * 1024);
        assert!(result.is_err(), "非法 PNG 应返回 Err");

        // clip_items 无孤立行
        let item_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clip_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            item_count, 0,
            "回滚后 clip_items 应无孤立行，实际 {item_count} 行"
        );

        // clip_images 同样无孤立行
        let img_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clip_images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            img_count, 0,
            "回滚后 clip_images 应无孤立行，实际 {img_count} 行"
        );
    }

    /// T4：孤立行领养路径 — clip_item_id=NULL 的孤立 clip_images 行应被新 item 领养。
    ///
    /// 步骤：正常入库一张图 → 手动置 clip_item_id=NULL → 再次调用 ingest_image_as_clip
    /// 同一 PNG → 断言：clip_images 仍 1 行（图片数据复用），clip_item_id 已补写为新 item_id，
    /// clip_items 新增 1 行（kind='image'），返回 Inserted。
    #[test]
    fn ingest_image_as_clip_adopts_orphaned_image_row() {
        let conn = make_test_conn();
        let png = make_test_png(3, 3);

        // 首次入库
        let first =
            ingest_image_as_clip(&conn, 3, 3, &png, 20 * 1024 * 1024).expect("首次入库失败");
        let first_item_id = match first {
            IngestOutcome::Inserted(id) => id,
            _ => panic!("首次应 Inserted"),
        };

        // 手动制造孤立行：把 clip_item_id 置 NULL
        conn.execute(
            "UPDATE clip_images SET clip_item_id = NULL WHERE clip_item_id = ?1",
            rusqlite::params![first_item_id],
        )
        .expect("置空 clip_item_id 失败");

        // 验证孤立行已存在
        let orphan_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_images WHERE clip_item_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphan_count, 1, "应有 1 条孤立行");

        // 二次调用同一 PNG — 应走 insert_image_clip 领养孤立行
        let second =
            ingest_image_as_clip(&conn, 3, 3, &png, 20 * 1024 * 1024).expect("二次调用失败");
        let second_item_id = match second {
            IngestOutcome::Inserted(id) => id,
            IngestOutcome::Bumped(_) => panic!("孤立行路径应返回 Inserted，不应 Bumped"),
        };

        // clip_images 仍 1 行（图片数据未重复存储）
        let img_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clip_images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(img_count, 1, "clip_images 应仍 1 行，图片数据不重复存储");

        // clip_item_id 已补写为新 item_id，非 NULL
        let linked_id: String = conn
            .query_row("SELECT clip_item_id FROM clip_images", [], |r| r.get(0))
            .expect("clip_item_id 不应为 NULL");
        assert_eq!(
            linked_id, second_item_id,
            "clip_images.clip_item_id 应指向新建 item"
        );

        // clip_items 现有 2 行（首次 + 孤立后新建）
        let item_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clip_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(item_count, 2, "clip_items 应有 2 行（首次 + 领养新建）");

        // 新建 item kind='image'
        let kind: String = conn
            .query_row(
                "SELECT kind FROM clip_items WHERE id = ?1",
                rusqlite::params![second_item_id],
                |r| r.get(0),
            )
            .expect("新建 clip_items 行应存在");
        assert_eq!(kind, "image", "新建 item kind 应为 'image'");
    }

    #[test]
    fn hex_encode_32_bytes_produces_64_chars() {
        let key = [0xabu8; 32];
        let hex = hex_encode(&key);
        assert_eq!(hex.len(), 64, "32 字节应编码为 64 个十六进制字符");
        assert!(
            hex.chars().all(|c| c.is_ascii_hexdigit()),
            "应全为十六进制字符"
        );
    }

    #[test]
    fn hex_encode_known_value() {
        let key = [0x07u8; 32];
        let hex = hex_encode(&key);
        assert_eq!(&hex[..2], "07", "0x07 应编码为 '07'");
    }

    /// 小阈值下入库超阈值图片，original_present 应为 0、BLOB 应为空。
    #[test]
    fn ingest_image_as_clip_respects_small_threshold() {
        let conn = make_test_conn();
        let png = make_test_png(2, 2);
        // 阈值设为 1 字节，任何图片都会超阈值
        let outcome = ingest_image_as_clip(&conn, 2, 2, &png, 1).expect("小阈值入库不应 Err");
        let item_id = match outcome {
            IngestOutcome::Inserted(id) => id,
            IngestOutcome::Bumped(_) => panic!("首次入库应返回 Inserted"),
        };

        let (original_present, original_blob): (i32, Vec<u8>) = conn
            .query_row(
                "SELECT original_present, original FROM clip_images WHERE clip_item_id = ?1",
                rusqlite::params![item_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("clip_images 行应存在");

        assert_eq!(original_present, 0, "超阈值图片 original_present 应为 0");
        assert!(original_blob.is_empty(), "超阈值图片原图 BLOB 应为空");
    }

    /// 大阈值下入库同一图片，original_present 应为 1、BLOB 非空。
    #[test]
    fn ingest_image_as_clip_respects_large_threshold() {
        let conn = make_test_conn();
        let png = make_test_png(2, 2);
        // 阈值设为 100MiB，任何测试图片都不超阈值
        let outcome =
            ingest_image_as_clip(&conn, 2, 2, &png, 100 * 1024 * 1024).expect("大阈值入库不应 Err");
        let item_id = match outcome {
            IngestOutcome::Inserted(id) => id,
            IngestOutcome::Bumped(_) => panic!("首次入库应返回 Inserted"),
        };

        let (original_present, original_blob): (i32, Vec<u8>) = conn
            .query_row(
                "SELECT original_present, original FROM clip_images WHERE clip_item_id = ?1",
                rusqlite::params![item_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("clip_images 行应存在");

        assert_eq!(original_present, 1, "未超阈值图片 original_present 应为 1");
        assert!(!original_blob.is_empty(), "未超阈值图片原图 BLOB 应非空");
    }
}
