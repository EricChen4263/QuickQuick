//! 图片捕获入库模块：字节哈希判重、原图无损存、缩略图/原图拆分 BLOB
//!
//! 设计对齐：设计文档§五#1（图片 BLOB 入库）、§五#2（超大图跳过）、
//!           §五#3（缩略图 WebP/256px/q75）、§十（schema 预埋铁律）
//!
//! 核心函数：
//! - `image_hash`                — 原图字节哈希（FNV-1a 64-bit，与文本哈希区分）
//! - `make_thumbnail`            — 解码原图，缩放至最长边 ≤256px，编码为 WebP q75
//! - `ingest_image`              — 去重入库（默认 policy，原图无损，向后兼容 s01）
//! - `ingest_image_with_policy`  — 带超大图 policy 入库：超阈值跳过原图只留缩略图
//! - `get_image_original`        — 按 id 取原图 BLOB
//! - `get_image_thumbnail`       — 按 id 取缩略图 BLOB
//! - `get_original_present`      — 按 id 取 original_present 标记（0=已跳过，1=已存）
//! - `image_count`               — 未软删行数（测试/业务用）
//! - `tiered_cleanup`            — 分级清理（由 `pipeline` 在新增图片行后自动触发，P1-4）
//! - `total_image_bytes`         — 未软删图片行总字节数（清理阈值判据）
//!
//! 安全约定：
//! - 所有 SQL 使用参数化查询，无字符串拼接
//! - 无裸 unwrap/panic（错误通过 ImageError/DbError 向上传播）

use std::time::{SystemTime, UNIX_EPOCH};

use image::imageops::FilterType;
use rusqlite::{Connection, OptionalExtension};
use uuid::Uuid;

use crate::db::DbError;

/// 缩略图最长边上限（标准屏）。
///
/// Retina/HiDPI 场景可用 THUMB_MAX_EDGE_RETINA=320；当前以 256 为准，
/// 保留常量便于未来切换。
const THUMB_MAX_EDGE: u32 = 256;

/// WebP 编码质量（0.0–100.0）。75 在文件大小与视觉质量间取得良好平衡。
const THUMB_QUALITY: f32 = 75.0;

/// 默认原图大小上限（20 MiB）。超出则跳过原图、只存缩略图。
pub const DEFAULT_MAX_ORIGINAL: usize = 20 * 1024 * 1024;

/// 超大图处理策略。`max_original_bytes` 可在测试中设为较小值验证可配性。
#[derive(Debug, Clone)]
pub struct OversizePolicy {
    /// 原图字节数上限，超出则跳过原图存储。
    pub max_original_bytes: usize,
}

impl Default for OversizePolicy {
    fn default() -> Self {
        Self {
            max_original_bytes: DEFAULT_MAX_ORIGINAL,
        }
    }
}

/// 缩略图生成/编码错误。
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    /// 原图解码失败（格式不支持或字节损坏）
    #[error("原图解码失败: {0}")]
    Decode(#[from] image::ImageError),
    /// WebP 编码失败
    #[error("WebP 编码失败: {0}")]
    Encode(String),
}

/// `ingest_image` / `ingest_image_with_policy` 的返回结果。
#[derive(Debug, PartialEq)]
pub enum IngestImageOutcome {
    /// 原图字节不存在，插入了新行；携带新行的 id
    Inserted(String),
    /// 原图字节已存在（字节哈希命中），原行已刷新 last_modified_utc；携带原行 id
    Bumped(String),
}

/// 将原图解码、缩放至最长边 ≤ `THUMB_MAX_EDGE`，编码为 WebP 质量 `THUMB_QUALITY`。
///
/// 缩放滤波器使用 `Lanczos3`，在缩小时视觉质量优于 Nearest/Bilinear。
/// 原图小于等于最长边阈值时不做放大，直接编码。
///
/// # Errors
/// - `ImageError::Decode`：原图字节无法解码（格式不支持或损坏）
/// - `ImageError::Encode`：WebP 编码内部失败（libwebp 返回错误）
pub fn make_thumbnail(original: &[u8]) -> Result<Vec<u8>, ImageError> {
    let img = image::load_from_memory(original)?;

    let scaled = scale_to_max_edge(img, THUMB_MAX_EDGE);

    let rgb = scaled.to_rgb8();
    let (w, h) = (rgb.width(), rgb.height());
    let encoder = webp::Encoder::from_rgb(rgb.as_raw(), w, h);
    let webp_memory = encoder
        .encode_simple(false, THUMB_QUALITY)
        .map_err(|e| ImageError::Encode(format!("{e:?}")))?;
    Ok(webp_memory.to_vec())
}

/// 按最长边约束等比缩放图片。若已满足则原样返回，不做放大。
fn scale_to_max_edge(img: image::DynamicImage, max_edge: u32) -> image::DynamicImage {
    let (w, h) = (img.width(), img.height());
    let long = w.max(h);
    if long <= max_edge {
        return img;
    }
    // 等比缩放：target_w/h 按最长边比例计算
    let (target_w, target_h) = if w >= h {
        (
            max_edge,
            (h as f64 * max_edge as f64 / w as f64).round() as u32,
        )
    } else {
        (
            (w as f64 * max_edge as f64 / h as f64).round() as u32,
            max_edge,
        )
    };
    img.resize_exact(target_w, target_h, FilterType::Lanczos3)
}

/// 计算原图字节哈希（显式稳定 FNV-1a 64-bit，按字节流，跨 Rust 版本一致）。
///
/// 与 `db::text_hash` 区分：本函数对 `&[u8]` 逐字节操作，用于图片内容判重；
/// `text_hash` 对 `&str` 的字节视图操作，用于文本内容判重。两者哈希空间互不干扰。
///
/// 非加密哈希，仅用于判重（碰撞概率极低但不为零，业务可接受）。
pub fn image_hash(bytes: &[u8]) -> String {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// 带超大图策略的去重入库。
///
/// 流程：
/// 1. 计算 `image_hash`
/// 2. 查询是否已有未软删的同 hash 行
/// 3. 命中 → 刷新 `last_modified_utc`，返回 `Bumped(id)`
/// 4. 未命中 → 生成缩略图（始终生成）；若 `original.len() > policy.max_original_bytes`
///    则跳过原图存储（BLOB 存空字节、`original_present=0`），否则正常存原图
///    （`original_present=1`）；插入新行，返回 `Inserted(id)`
///
/// # Errors
/// - `DbError::Sqlite`：SQL 执行失败
/// - `DbError::Other`：缩略图生成失败（编码错误，含错误描述）
pub fn ingest_image_with_policy(
    conn: &Connection,
    original: &[u8],
    policy: &OversizePolicy,
) -> Result<IngestImageOutcome, DbError> {
    let hash = image_hash(original);
    let now_ms = current_utc_ms();

    let existing_id: Option<String> = conn
        .query_row(
            "SELECT id FROM clip_images WHERE image_hash = ?1 AND is_deleted = 0 LIMIT 1",
            rusqlite::params![hash],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    if let Some(id) = existing_id {
        conn.execute(
            "UPDATE clip_images SET last_modified_utc = ?1 WHERE id = ?2",
            rusqlite::params![now_ms, id],
        )?;
        return Ok(IngestImageOutcome::Bumped(id));
    }

    let thumbnail =
        make_thumbnail(original).map_err(|e| DbError::Other(format!("缩略图生成失败: {e}")))?;

    let is_oversize = original.len() > policy.max_original_bytes;
    let (stored_original, original_present): (&[u8], i32) = if is_oversize {
        // 超大图：跳过原图，BLOB 存空字节，标记 original_present=0
        (b"", 0)
    } else {
        (original, 1)
    };

    let new_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO clip_images
             (id, thumbnail, original, original_present, image_hash, created_utc, last_modified_utc, is_deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 0)",
        rusqlite::params![new_id, thumbnail, stored_original, original_present, hash, now_ms],
    )?;

    Ok(IngestImageOutcome::Inserted(new_id))
}

/// 按 id 取 `original_present` 标记（0=原图过大未存，1=原图已存）。
///
/// 未找到行或行已软删时返回 `Ok(None)`。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn get_original_present(conn: &Connection, id: &str) -> Result<Option<i32>, DbError> {
    let result = conn
        .query_row(
            "SELECT original_present FROM clip_images WHERE id = ?1 AND is_deleted = 0",
            rusqlite::params![id],
            |row| row.get::<_, i32>(0),
        )
        .optional()?;
    Ok(result)
}

/// 去重入库：将图片原图和缩略图写入 `clip_images` 表，自动按原图字节哈希判重。
///
/// 流程：
/// 1. 计算 `original` 的 `image_hash`
/// 2. 查询 `clip_images` 中是否有未软删的同 hash 行
/// 3. 命中 → 刷新 `last_modified_utc`，返回 `Bumped(id)`
/// 4. 未命中 → 插入新行：原图原样存入（无损，不转码），返回 `Inserted(id)`
///
/// # 原图无损保证
/// `original` 字节以 BLOB 形式原样写入，取回时逐字节相同。
///
/// # clip_item_id 缺口声明（当前阶段 NULL，GC 级联不生效）
/// 当前阶段 `clip_item_id` 不写入（保持 NULL）。`clip_images` 表的
/// `clip_item_id` 列上设有 `ON DELETE CASCADE` 外键约束，但该约束仅对
/// 非 NULL 值触发（SQLite 标准行为）；NULL 行不会随父行 `clip_items`
/// 删除而级联删除，因此 GC 级联路径对本函数写入的行**不生效**。
/// 与 `clip_item_id` 绑定及对应 GC 路径留待分级清理 story **V3-F1-A04** 补全。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn ingest_image(
    conn: &Connection,
    original: &[u8],
    thumbnail: &[u8],
) -> Result<IngestImageOutcome, DbError> {
    let hash = image_hash(original);
    let now_ms = current_utc_ms();

    // 查询是否已有未软删的同 hash 行
    let existing_id: Option<String> = conn
        .query_row(
            "SELECT id FROM clip_images WHERE image_hash = ?1 AND is_deleted = 0 LIMIT 1",
            rusqlite::params![hash],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    if let Some(id) = existing_id {
        // 命中：刷新 last_modified_utc，不新建行
        conn.execute(
            "UPDATE clip_images SET last_modified_utc = ?1 WHERE id = ?2",
            rusqlite::params![now_ms, id],
        )?;
        return Ok(IngestImageOutcome::Bumped(id));
    }

    // 未命中：插入新行，原图原样存入（无损）
    let new_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO clip_images
             (id, thumbnail, original, original_present, image_hash, created_utc, last_modified_utc, is_deleted)
         VALUES (?1, ?2, ?3, 1, ?4, ?5, ?5, 0)",
        rusqlite::params![new_id, thumbnail, original, hash, now_ms],
    )?;

    Ok(IngestImageOutcome::Inserted(new_id))
}

/// 按 id 取原图 BLOB。
///
/// 未找到行或行已软删时返回 `Ok(None)`。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn get_image_original(conn: &Connection, id: &str) -> Result<Option<Vec<u8>>, DbError> {
    let result = conn
        .query_row(
            "SELECT original FROM clip_images WHERE id = ?1 AND is_deleted = 0",
            rusqlite::params![id],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()?;
    Ok(result)
}

/// 按 id 取缩略图 BLOB。
///
/// 未找到行或行已软删时返回 `Ok(None)`。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn get_image_thumbnail(conn: &Connection, id: &str) -> Result<Option<Vec<u8>>, DbError> {
    let result = conn
        .query_row(
            "SELECT thumbnail FROM clip_images WHERE id = ?1 AND is_deleted = 0",
            rusqlite::params![id],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()?;
    Ok(result)
}

/// 返回未软删的图片行数。
///
/// 用于测试断言和业务层「图片总数」查询。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn image_count(conn: &Connection) -> Result<i64, DbError> {
    let n = conn.query_row(
        "SELECT COUNT(*) FROM clip_images WHERE is_deleted = 0",
        [],
        |row| row.get(0),
    )?;
    Ok(n)
}

/// 分级清理策略。`max_total_bytes` 可在测试中设为较小值验证可配性。
#[derive(Debug, Clone)]
pub struct CleanupPolicy {
    /// 图片总字节上限（原图 + 缩略图合计）。超出时触发分级清理。
    pub max_total_bytes: i64,
}

impl Default for CleanupPolicy {
    fn default() -> Self {
        Self {
            max_total_bytes: DEFAULT_MAX_TOTAL,
        }
    }
}

/// 分级清理报告：记录本次清理腾掉的原图数、整条删除数与回收的孤立墓碑数。
#[derive(Debug, Default, PartialEq)]
pub struct CleanupReport {
    /// 第一级：腾掉原图（strip original）的行数
    pub stripped_originals: usize,
    /// 第二级：整条软删的行数（`clip_images` 与其父 `clip_items` 同步软删）
    pub deleted_rows: usize,
    /// 第三级：物理回收的孤立图片墓碑行数（`clip_item_id` 为空、无父条目可级联）
    pub purged_orphan_images: usize,
}

/// 默认图片总量上限（500 MiB）。
pub const DEFAULT_MAX_TOTAL: i64 = 500 * 1024 * 1024;

/// 计算未软删图片行的总字节数（原图 BLOB + 缩略图 BLOB 合计）。
///
/// SQLite 的 `length()` 对 NULL 返回 NULL，`COALESCE` 保证空/NULL BLOB 计为 0。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn total_image_bytes(conn: &Connection) -> Result<i64, DbError> {
    let total = conn.query_row(
        "SELECT COALESCE(SUM(COALESCE(length(original), 0) + COALESCE(length(thumbnail), 0)), 0)
         FROM clip_images WHERE is_deleted = 0",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(total)
}

/// 判断指定行是否处于降级态（original_present=0，即无本地原图）。
///
/// 三种情形统一映射：超大图未存 / 本地清理腾掉 / v2 同步未拉，
/// 均表示为 `original_present=0`，通过本函数统一判断。
///
/// 行不存在或已软删时返回 `Ok(false)`。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn is_degraded(conn: &Connection, id: &str) -> Result<bool, DbError> {
    let result = conn
        .query_row(
            "SELECT original_present FROM clip_images WHERE id = ?1 AND is_deleted = 0",
            rusqlite::params![id],
            |row| row.get::<_, i32>(0),
        )
        .optional()?;
    Ok(result.is_some_and(|v| v == 0))
}

/// 腾空指定行的原图 BLOB，并将 `original_present` 置为 0（降级态）。
///
/// 缩略图保留不动。调用后该行进入与超大图未存相同的"有缩略图无本地原图"状态。
/// 仅操作未软删行（`AND is_deleted = 0`），软删行静默跳过，不触发语义不一致。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn strip_original(conn: &Connection, id: &str) -> Result<(), DbError> {
    conn.execute(
        "UPDATE clip_images SET original = X'', original_present = 0 WHERE id = ?1 AND is_deleted = 0",
        rusqlite::params![id],
    )?;
    Ok(())
}

/// 分级清理：按 `policy.max_total_bytes` 控制图片总存储量。
///
/// 三级策略：
///
/// **第一级**：当 total > max，按 `created_utc` 最旧优先、仅非收藏（`is_favorite=0`），
/// 逐个调用 `strip_original`（清掉原图 BLOB、`original_present=0`、保留缩略图），
/// 直到 total ≤ max 或无可腾原图的非收藏行。
///
/// **第二级**：若第一级完成后 total 仍 > max（缩略图本身也超限），
/// 按 `created_utc` 最旧优先、仅非收藏，逐行「整条」软删：`clip_images` 行与它关联的
/// `clip_items` 父条目**同步**软删（否则前端会留下既无缩略图、又无法粘贴的幽灵条目，
/// 且父条目从未标墓碑 → 级联 GC 永不触发 → BLOB 永不释放）。
///
/// **第三级**：回收没有父条目可级联的孤立图片墓碑（见
/// [`crate::db::purge_orphan_image_tombstones`]）——这类行任何 GC 路径都碰不到，
/// 不回收就是永久占库。
///
/// 第三级**只在本次清理真的动过行时**才执行（`stripped_originals > 0 ||
/// deleted_rows > 0`）：孤立墓碑只可能由第二级删除或历史遗留产生，历史遗留会在
/// 「第一次真正需要清理」时被顺带回收——那正是有空间压力的时候。日常零压力的清理轮次
/// （未超限）不该为一次全表 `DELETE ... WHERE is_deleted = 1` 付费：
/// 20k 行量级实测约 88ms，而图片连续复制时每轮都会走到这里。
///
/// **收藏永远豁免**：`is_favorite=1` 的行不参与任何清理，
/// 其 `clip_items` 父条目同样不动。
///
/// # 触发点与阈值
/// 由 [`crate::pipeline::capture_and_ingest`] 在「本轮新增了图片行」之后自动调用，
/// 阈值取 [`CleanupPolicy::default`]（[`DEFAULT_MAX_TOTAL`] = 500 MiB），
/// 与存储页「总量上限 500 MB · 超出按分级清理」的承诺同源。
/// 文本轮次与图片去重命中都不会触发，避免每 500ms 付一次聚合开销。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn tiered_cleanup(conn: &Connection, policy: &CleanupPolicy) -> Result<CleanupReport, DbError> {
    // 第一级：strip 最旧非收藏原图
    let stripped_originals = strip_oldest_originals(conn, policy.max_total_bytes)?;

    // 第二级：若 strip 后仍超限，整条软删最旧非收藏
    let deleted_rows = if total_image_bytes(conn)? > policy.max_total_bytes {
        delete_oldest_nonfavorite_rows(conn, policy.max_total_bytes)?
    } else {
        0
    };

    // 第三级：只在本次清理真的动过行时才回收孤立墓碑。
    // 判据用"本次动了行"而不是"是否超限"：孤立墓碑本就是清理的副产物或历史遗留，
    // 没有清理动作的轮次（未超限）不该为一次全表 DELETE 付 ~88ms（20k 行实测）。
    let purged_orphan_images = if stripped_originals > 0 || deleted_rows > 0 {
        crate::db::purge_orphan_image_tombstones(conn)? as usize
    } else {
        0
    };

    Ok(CleanupReport {
        stripped_originals,
        deleted_rows,
        purged_orphan_images,
    })
}

/// 第一级清理：strip 最旧非收藏行的原图，直到总量 ≤ max 或无可腾行。
///
/// 本地累计已释放字节以减少全表扫描：候选查询时一并 SELECT length(original)，
/// 每次 strip 后直接从估算总量中减去该行原图大小，仅当本地估算超限时继续下一条，
/// 避免每条都触发全表 SUM 扫描（O(N²) → O(N)）。
///
/// 返回实际 strip 的行数。
fn strip_oldest_originals(conn: &Connection, max_bytes: i64) -> Result<usize, DbError> {
    // 候选查询同时取 length(original)，一并传入避免额外 SELECT
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(length(original), 0) FROM clip_images
         WHERE is_deleted = 0 AND is_favorite = 0 AND original_present = 1
         ORDER BY created_utc ASC",
    )?;
    let candidates: Vec<(String, i64)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<Result<_, _>>()?;

    // 取一次当前总量作为基准；后续用本地累计释放量估算，不再循环全表扫
    let mut estimated_total = total_image_bytes(conn)?;
    let mut stripped = 0usize;
    for (id, orig_len) in &candidates {
        if estimated_total <= max_bytes {
            break;
        }
        strip_original(conn, id)?;
        // strip 后原图 BLOB 置为 X''（0 字节），减去已知大小更新估算
        estimated_total -= orig_len;
        stripped += 1;
    }
    Ok(stripped)
}

/// 第二级清理：整条软删最旧非收藏行，直到总量 ≤ max 或无可删行。
///
/// "整条"= `clip_images` 行 + 它关联的 `clip_items` 父条目一起软删：
/// - 只软删图片行会让父条目继续出现在列表里（`list_items_with_images` 以 `clip_items`
///   为主表），前端渲染出既无缩略图、又无法粘贴的幽灵条目；
/// - 父条目从不标墓碑，`db::gc_purge_deleted` 就永远不会级联删除图片行，
///   BLOB 也就永远不会从库文件里释放。
///
/// 父条目软删复用 [`crate::db::soft_delete`]（与用户手动删除同一语义、同一时间戳字段），
/// 不手写 SQL 绕开。
///
/// 孤立行（`clip_item_id IS NULL`）没有可软删的父条目：图片行照常软删，
/// 其物理回收由第三级 [`crate::db::purge_orphan_image_tombstones`] 负责，
/// 不依赖父条目级联。
///
/// 本地累计估算：候选查询一并取整行字节数与父条目 id，每删一行就减去已知字节，
/// 不在循环体里重复全表 SUM（与第一级同样的 O(N²) → O(N) 优化）。
///
/// 返回实际整条软删的行数。
fn delete_oldest_nonfavorite_rows(conn: &Connection, max_bytes: i64) -> Result<usize, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, clip_item_id,
                COALESCE(length(original), 0) + COALESCE(length(thumbnail), 0)
         FROM clip_images
         WHERE is_deleted = 0 AND is_favorite = 0
         ORDER BY created_utc ASC",
    )?;
    let candidates: Vec<(String, Option<String>, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<Result<_, _>>()?;

    // 取一次当前总量作为基准；后续用本地累计释放量估算，不再循环全表扫
    let mut estimated_total = total_image_bytes(conn)?;
    let now_ms = current_utc_ms();
    let mut deleted = 0usize;
    for (image_id, clip_item_id, row_bytes) in &candidates {
        if estimated_total <= max_bytes {
            break;
        }
        conn.execute(
            "UPDATE clip_images
             SET is_deleted = 1, deleted_at_utc = ?1, last_modified_utc = ?1
             WHERE id = ?2",
            rusqlite::params![now_ms, image_id],
        )?;
        // 父条目同步软删；孤立行没有父条目，其回收归第三级
        if let Some(item_id) = clip_item_id {
            crate::db::soft_delete(conn, item_id)?;
        }
        estimated_total -= row_bytes;
        deleted += 1;
    }
    Ok(deleted)
}

/// 获取当前 UTC 时间戳（毫秒）。
fn current_utc_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// 测试专用：建一个带完整 schema 的临时加密库，返回（临时目录, 连接）。
///
/// 用生产建表语句（`db::open_or_create`）而不是手抄 schema，避免测试与真实表结构漂移。
/// 供本模块与 `pipeline` 的清理相关测试共用，避免两处各写一份建库逻辑。
#[cfg(test)]
pub(crate) fn open_test_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn =
        crate::db::open_or_create(&dir.path().join("qq.db"), &[7u8; 32]).expect("建库应成功");
    (dir, conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_hash_deterministic_same_bytes() {
        let bytes = vec![0x89u8, 0x50, 0x4E, 0x47];
        let h1 = image_hash(&bytes);
        let h2 = image_hash(&bytes);
        assert_eq!(h1, h2, "相同字节序列应产生相同哈希");
    }

    /// 末位仅差一字节（`[1,2,3]` vs `[1,2,4]`），验证哈希函数对相邻输入的判别力。
    /// 倒序序列（`[1,2,3]` vs `[3,2,1]`）差异过大，无法体现边界灵敏度。
    #[test]
    fn image_hash_differs_on_last_byte_only() {
        let h1 = image_hash(&[1, 2, 3]);
        let h2 = image_hash(&[1, 2, 4]);
        assert_ne!(h1, h2, "末位仅差一字节时哈希应不同");
    }

    /// 空序列与单字节边界：验证极端输入不 panic 且两者哈希不同（非恒真）。
    #[test]
    fn image_hash_empty_and_single_byte_boundary() {
        let h_empty = image_hash(&[]);
        let h_one = image_hash(&[0x00]);
        assert_eq!(h_empty.len(), 16, "空序列应产生 16 字符哈希");
        assert_eq!(h_one.len(), 16, "单字节序列应产生 16 字符哈希");
        assert_ne!(h_empty, h_one, "空序列与单字节序列哈希应不同");
    }

    #[test]
    fn image_hash_produces_16_hex_chars() {
        let h = image_hash(&[0xAB, 0xCD]);
        assert_eq!(h.len(), 16, "FNV-1a 64-bit 哈希应产生 16 个十六进制字符");
        assert!(
            h.chars().all(|c| c.is_ascii_hexdigit()),
            "哈希应全为十六进制字符"
        );
    }
    /// 造一条未软删图片行（最小合法 schema 要求：id + created_utc + last_modified_utc）。
    ///
    /// `original_size` / `thumb_size` 用来精确控制总字节，便于造阈值边界。
    fn insert_row(
        conn: &Connection,
        id: &str,
        created_utc: i64,
        original_size: usize,
        thumb_size: usize,
        is_favorite: bool,
    ) {
        conn.execute(
            "INSERT INTO clip_images
                (id, clip_item_id, thumbnail, original, original_present, image_hash,
                 created_utc, last_modified_utc, is_deleted, is_favorite)
             VALUES (?1, NULL, ?2, ?3, 1, ?4, ?5, ?5, 0, ?6)",
            rusqlite::params![
                id,
                vec![0xABu8; thumb_size],
                vec![0xCDu8; original_size],
                format!("hash-{id}"),
                created_utc,
                i32::from(is_favorite),
            ],
        )
        .expect("插入 clip_images 行应成功");
    }

    #[test]
    fn tiered_cleanup_is_noop_when_total_equals_limit() {
        // Arrange：总量刚好等于阈值（边界用 >，等于不应动手）
        let (_dir, conn) = open_test_db();
        insert_row(&conn, "old", 1_000, 100, 10, false);
        let total = total_image_bytes(&conn).expect("total");
        let policy = CleanupPolicy {
            max_total_bytes: total,
        };

        // Act
        let report = tiered_cleanup(&conn, &policy).expect("tiered_cleanup 应成功");

        // Assert：报告两字段为 0，且原图未被腾空
        assert_eq!(
            report,
            CleanupReport::default(),
            "总量等于上限时不应做任何清理"
        );
        assert_eq!(
            get_original_present(&conn, "old").expect("ok"),
            Some(1),
            "等于阈值时不应腾空原图"
        );
    }

    #[test]
    fn tiered_cleanup_strips_only_until_back_under_limit() {
        // Arrange：三行非收藏，阈值刚好够留下最新一行的原图
        let (_dir, conn) = open_test_db();
        insert_row(&conn, "a-oldest", 1_000, 100, 10, false);
        insert_row(&conn, "b-middle", 2_000, 100, 10, false);
        insert_row(&conn, "c-newest", 3_000, 100, 10, false);
        let total = total_image_bytes(&conn).expect("total");
        let policy = CleanupPolicy {
            // 留出最新一行（110 字节）+ 中间缩略图，故只需腾空最旧一行的原图
            max_total_bytes: total - 100,
        };

        // Act
        let report = tiered_cleanup(&conn, &policy).expect("tiered_cleanup 应成功");

        // Assert 1：只腾空一行原图，且没有整条删除
        assert_eq!(report.stripped_originals, 1, "只需腾空最旧一行的原图");
        assert_eq!(report.deleted_rows, 0, "回到阈值内后不应再整条删");
        // Assert 2：腾的是最旧那行，且缩略图仍在
        assert_eq!(
            get_original_present(&conn, "a-oldest").expect("ok"),
            Some(0)
        );
        assert_eq!(
            get_original_present(&conn, "b-middle").expect("ok"),
            Some(1)
        );
        assert_eq!(
            get_original_present(&conn, "c-newest").expect("ok"),
            Some(1)
        );
        assert_eq!(
            get_image_thumbnail(&conn, "a-oldest")
                .expect("ok")
                .map(|t| t.len()),
            Some(10),
            "腾空原图必须保留缩略图"
        );
        // Assert 3：清理后确实落在阈值内
        assert!(
            total_image_bytes(&conn).expect("total") <= policy.max_total_bytes,
            "清理后总量应不超过阈值"
        );
    }

    #[test]
    fn tiered_cleanup_stops_when_only_favorites_remain() {
        // Arrange：两行全是收藏，阈值设为 0（必然超限）
        let (_dir, conn) = open_test_db();
        insert_row(&conn, "fav-old", 1_000, 100, 10, true);
        insert_row(&conn, "fav-new", 2_000, 100, 10, true);
        let policy = CleanupPolicy { max_total_bytes: 0 };

        // Act：必须能正常返回（不能因「超限但无可清理行」而无限循环）
        let report = tiered_cleanup(&conn, &policy).expect("tiered_cleanup 应成功");

        // Assert：一行都不动，且总量仍超限（这是被豁免的合法状态）
        assert_eq!(
            report,
            CleanupReport::default(),
            "只剩收藏行时不应清理任何东西"
        );
        assert_eq!(get_original_present(&conn, "fav-old").expect("ok"), Some(1));
        assert_eq!(get_original_present(&conn, "fav-new").expect("ok"), Some(1));
        assert!(total_image_bytes(&conn).expect("total") > 0);
    }

    #[test]
    fn tiered_cleanup_report_fields_match_each_level() {
        // Arrange：缩略图本身就超限（阈值 0）→ 两级都会被触发
        let (_dir, conn) = open_test_db();
        insert_row(&conn, "old", 1_000, 50, 10, false);
        insert_row(&conn, "new", 2_000, 50, 10, false);
        let policy = CleanupPolicy { max_total_bytes: 0 };

        // Act
        let report = tiered_cleanup(&conn, &policy).expect("tiered_cleanup 应成功");

        // Assert：第一级字段 = 被腾空原图的行数（两行都先被 strip）
        assert_eq!(report.stripped_originals, 2, "两行非收藏原图都应先被腾空");
        // Assert：第二级字段 = 被整条软删的行数（缩略图仍超限，故两行都被删）
        assert_eq!(report.deleted_rows, 2, "缩略图仍超限时两行都应被整条软删");
        // Assert：第三级字段 = 物理回收的孤立墓碑数（helper 插的行没有父条目）
        assert_eq!(
            report.purged_orphan_images, 2,
            "无父条目的孤立墓碑应在同一次清理里被物理回收"
        );
        // Assert：软删后已无活跃行，且孤立墓碑不会留下来占库
        assert_eq!(image_count(&conn).expect("image_count"), 0);
        assert_eq!(
            physical_image_rows(&conn),
            0,
            "孤立墓碑必须被物理删除，不能变成永久占库的 BLOB"
        );
    }

    /// 造一张最小 PNG（真实入库路径需要合法图片字节）。
    fn tiny_png(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        use image::ImageEncoder;
        let img = image::RgbImage::from_pixel(width, height, image::Rgb(rgb));
        let mut buf = Vec::new();
        image::codecs::png::PngEncoder::new(&mut buf)
            .write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
            .expect("PNG 编码应成功");
        buf
    }

    /// `clip_images` 物理行数（含墓碑）。
    fn physical_image_rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM clip_images", [], |row| row.get(0))
            .expect("统计物理行数应成功")
    }

    /// 走真实入库路径造一条图片条目（同时创建 clip_items 父条目与外键关联）。
    fn ingest_image_row(conn: &Connection, rgb: [u8; 3]) -> String {
        let png = tiny_png(2, 2, rgb);
        let outcome = crate::db::ingest_image_as_clip(conn, 2, 2, &png, usize::MAX as u64)
            .expect("图片入库应成功");
        match outcome {
            crate::db::IngestOutcome::Inserted(id) => id,
            crate::db::IngestOutcome::Bumped(id) => id,
        }
    }

    #[test]
    fn second_level_soft_deletes_parent_items_too() {
        // Arrange：两条真实入库的图片（各有 clip_items 父条目），阈值 0 强制第二级
        let (_dir, conn) = open_test_db();
        ingest_image_row(&conn, [255, 0, 0]);
        ingest_image_row(&conn, [0, 255, 0]);
        assert_eq!(crate::db::count_live(&conn).expect("count_live"), 2);

        // Act
        let report = tiered_cleanup(&conn, &CleanupPolicy { max_total_bytes: 0 }).expect("cleanup");

        // Assert 1：两级都生效
        assert_eq!(report.deleted_rows, 2, "两行非收藏都应被整条软删");
        // Assert 2：图片行活跃数归零
        assert_eq!(image_count(&conn).expect("image_count"), 0);
        // Assert 3：父条目同步软删——这是本 BLOCK 的核心
        assert_eq!(
            crate::db::count_live(&conn).expect("count_live"),
            0,
            "第二级必须同步软删 clip_items 父条目，否则前端留下幽灵条目"
        );
        // Assert 4：列表不再渲染任何 image 占位行
        let listed = crate::db::list_items_with_images(&conn).expect("list");
        assert!(
            listed.is_empty(),
            "清理后列表应为空，实际仍有 {} 条幽灵条目",
            listed.len()
        );
    }

    #[test]
    fn gc_after_second_level_physically_releases_image_blobs() {
        // Arrange：真实入库两条图片 → 第二级整条软删
        let (_dir, conn) = open_test_db();
        ingest_image_row(&conn, [255, 0, 0]);
        ingest_image_row(&conn, [0, 0, 255]);
        let report = tiered_cleanup(&conn, &CleanupPolicy { max_total_bytes: 0 }).expect("cleanup");
        assert_eq!(report.deleted_rows, 2);
        // 软删阶段：墓碑行仍在（靠 GC 物理清）
        assert_eq!(physical_image_rows(&conn), 2, "软删阶段不应物理删行");

        // Act：跑既有 GC
        let purged = crate::db::gc_purge_deleted(&conn).expect("gc");

        // Assert 1：父条目墓碑被物理删除
        assert_eq!(purged, 2, "两个 clip_items 墓碑应被物理删除");
        // Assert 2：外键级联把图片行也物理删了 —— BLOB 真正释放
        assert_eq!(
            physical_image_rows(&conn),
            0,
            "父条目被物理删除后，clip_images 应被 ON DELETE CASCADE 级联清理"
        );
    }

    #[test]
    fn favorite_parent_item_survives_second_level_cleanup() {
        // Arrange：一条收藏图片 + 一条非收藏图片，阈值 0 强制第二级
        let (_dir, conn) = open_test_db();
        let fav_item_id = ingest_image_row(&conn, [1, 2, 3]);
        // 收藏标记打在图片行上（分级清理按它豁免）与父条目上（用于断言父条目未被动）
        conn.execute(
            "UPDATE clip_images SET is_favorite = 1 WHERE clip_item_id = ?1",
            rusqlite::params![fav_item_id],
        )
        .expect("标记收藏应成功");
        conn.execute(
            "UPDATE clip_items SET is_favorite = 1 WHERE id = ?1",
            rusqlite::params![fav_item_id],
        )
        .expect("标记父条目收藏应成功");
        ingest_image_row(&conn, [9, 9, 9]);

        // Act
        let report = tiered_cleanup(&conn, &CleanupPolicy { max_total_bytes: 0 }).expect("cleanup");

        // Assert 1：只清了非收藏那一条
        assert_eq!(report.deleted_rows, 1, "只应清理 1 条非收藏行");
        // Assert 2：收藏图片行与其原图都还在
        // 注意：ingest_image_as_clip 返回的是 clip_items id，图片行 id 需按外键查回
        let fav_image_id: String = conn
            .query_row(
                "SELECT id FROM clip_images WHERE clip_item_id = ?1",
                rusqlite::params![fav_item_id],
                |row| row.get(0),
            )
            .expect("应能查到收藏项的图片行 id");
        assert_eq!(
            get_original_present(&conn, &fav_image_id).expect("ok"),
            Some(1),
            "收藏项原图必须豁免"
        );
        let fav_live: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_images WHERE clip_item_id = ?1 AND is_deleted = 0",
                rusqlite::params![fav_item_id],
                |row| row.get(0),
            )
            .expect("查询收藏图片行应成功");
        assert_eq!(fav_live, 1, "收藏图片行不应被软删");
        // Assert 3：收藏的父条目也不许被动
        let fav_item_live: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_items WHERE id = ?1 AND is_deleted = 0",
                rusqlite::params![fav_item_id],
                |row| row.get(0),
            )
            .expect("查询收藏父条目应成功");
        assert_eq!(fav_item_live, 1, "收藏父条目不应被软删");
        assert_eq!(
            crate::db::count_live(&conn).expect("count_live"),
            1,
            "活跃条目只剩收藏那一条"
        );
    }

    #[test]
    fn orphan_image_tombstones_are_reclaimed_without_parent_cascade() {
        // Arrange：两条孤立图片行（clip_item_id 为空，没有可级联的父条目）
        let (_dir, conn) = open_test_db();
        insert_row(&conn, "orphan-a", 1_000, 50, 10, false);
        insert_row(&conn, "orphan-b", 2_000, 50, 10, false);

        // Act
        let report = tiered_cleanup(&conn, &CleanupPolicy { max_total_bytes: 0 }).expect("cleanup");

        // Assert 1：孤立墓碑在同一次清理里被物理回收（不留永久占库的 BLOB）
        assert_eq!(
            report.purged_orphan_images, 2,
            "孤立图片墓碑应被第三级物理回收"
        );
        // Assert 2：物理行数归零——孤立行再无 GC 死角
        assert_eq!(physical_image_rows(&conn), 0);
        // Assert 3：purge 自身幂等——再跑一次应返回 0
        // （此前这条锚在 gc_purge_deleted 上，属锚错对象：回收孤立行的是 purge，不是 GC）
        assert_eq!(
            crate::db::purge_orphan_image_tombstones(&conn).expect("purge"),
            0,
            "已无孤立墓碑时 purge 应返回 0（幂等）"
        );
        assert_eq!(physical_image_rows(&conn), 0);
    }

    #[test]
    fn orphan_tombstones_are_skipped_on_idle_rounds_and_reclaimed_on_real_cleanup() {
        // Arrange：一条真实入库的活图片（有父条目）+ 一条历史遗留的孤立墓碑
        let (_dir, conn) = open_test_db();
        let live_item_id = ingest_image_row(&conn, [7, 8, 9]);
        insert_row(&conn, "legacy-orphan", 500, 30, 10, false);
        conn.execute(
            "UPDATE clip_images SET is_deleted = 1 WHERE id = 'legacy-orphan'",
            [],
        )
        .expect("标记历史孤立墓碑应成功");
        assert_eq!(physical_image_rows(&conn), 2);

        // Act 1：未超限轮次（阈值远大于总量）→ 不做任何清理
        let idle = tiered_cleanup(
            &conn,
            &CleanupPolicy {
                max_total_bytes: i64::MAX,
            },
        )
        .expect("idle cleanup");

        // Assert 1：零动作，且没有为回收墓碑付一次全表 DELETE
        assert_eq!(
            idle,
            CleanupReport::default(),
            "未超限轮次不应有任何清理动作"
        );
        assert_eq!(
            idle.purged_orphan_images, 0,
            "未超限轮次不应执行孤立墓碑回收"
        );
        assert_eq!(
            physical_image_rows(&conn),
            2,
            "未超限轮次不应减少任何行（历史孤立墓碑留着等真正需要清理时再收）"
        );

        // Act 2：有空间压力的轮次（阈值 0）→ 真清理，顺带回收历史遗留孤立墓碑
        let busy = tiered_cleanup(&conn, &CleanupPolicy { max_total_bytes: 0 }).expect("cleanup");

        // Assert 2：这次真的动了行，故第三级被执行
        assert!(
            busy.stripped_originals > 0 || busy.deleted_rows > 0,
            "阈值 0 时应发生实际清理，实际: {busy:?}"
        );
        assert_eq!(
            busy.purged_orphan_images, 1,
            "第一次真正清理时应回收那条历史遗留的孤立墓碑"
        );
        // Assert 3：被回收的是孤立行；有父条目的图片行虽已软删，仍等 GC 走级联
        let orphan_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_images WHERE id = 'legacy-orphan'",
                [],
                |row| row.get(0),
            )
            .expect("查询应成功");
        assert_eq!(orphan_exists, 0, "历史孤立墓碑应被物理删除");
        let live_image_alive: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_images WHERE clip_item_id = ?1",
                rusqlite::params![live_item_id],
                |row| row.get(0),
            )
            .expect("查询应成功");
        assert_eq!(
            live_image_alive, 1,
            "有父条目的图片行不应被第三级回收（其物理清除走 GC 级联）"
        );
    }
}
