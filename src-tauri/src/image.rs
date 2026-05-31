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
        Self { max_original_bytes: DEFAULT_MAX_ORIGINAL }
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
        (max_edge, (h as f64 * max_edge as f64 / w as f64).round() as u32)
    } else {
        ((w as f64 * max_edge as f64 / h as f64).round() as u32, max_edge)
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

    let thumbnail = make_thumbnail(original)
        .map_err(|e| DbError::Other(format!("缩略图生成失败: {e}")))?;

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
pub fn get_original_present(
    conn: &Connection,
    id: &str,
) -> Result<Option<i32>, DbError> {
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
pub fn get_image_original(
    conn: &Connection,
    id: &str,
) -> Result<Option<Vec<u8>>, DbError> {
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
pub fn get_image_thumbnail(
    conn: &Connection,
    id: &str,
) -> Result<Option<Vec<u8>>, DbError> {
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

/// 获取当前 UTC 时间戳（毫秒）。
fn current_utc_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
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
}
