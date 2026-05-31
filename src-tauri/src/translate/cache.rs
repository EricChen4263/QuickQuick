//! 翻译缓存模块（V2-F1-S05）
//!
//! 职责：落 DB 持久化、四元组键、LRU 淘汰。
//!
//! 缓存键由 `(source_text, source_lang, target_lang, provider_id)` 四元组 FNV-1a 哈希组成。
//! provider_id 是键的一部分，保证换源必 miss，不同 provider 的缓存完全隔离。
//!
//! LRU 语义：
//! - `cache_get` 命中时刷新 `last_used_utc`
//! - `cache_put` 写入后若超 capacity，删除 `last_used_utc` 最旧的若干条至 capacity

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::db::DbError;

/// 待写入缓存的条目描述（用于减少 `cache_put_at` 参数数量）。
pub struct CacheEntry<'a> {
    pub source_text: &'a str,
    pub source_lang: &'a str,
    pub target_lang: &'a str,
    pub provider_id: &'a str,
    pub translated: &'a str,
}

/// 计算四元组缓存键（FNV-1a 64-bit 哈希，显式稳定，不依赖 std::hash 随机化）。
///
/// 键由四段按序哈希，段间插入 `\0` 分隔符防止前缀碰撞
/// （例如 `("ab","c")` 与 `("a","bc")` 产生不同哈希）。
/// `provider_id` 不同时键必然不同，换源必 miss。
pub fn cache_key(
    source_text: &str,
    source_lang: &str,
    target_lang: &str,
    provider_id: &str,
) -> String {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

    let mut hash = FNV_OFFSET;
    for segment in [source_text, source_lang, target_lang, provider_id] {
        for byte in segment.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        // 段间分隔符：非零字节 0x01 使哈希状态真正改变，防止空段前缀碰撞
        // （XOR 0 不改变状态；XOR 非零值 + mul 保证空段与非空段路径分叉）
        hash ^= 0x01_u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// 查询缓存：命中时刷新 `last_used_utc`（LRU 访问更新）并返回 translated；未命中返回 None。
///
/// 内部委托给 `cache_get_at`，使用当前系统时间作为刷新时间戳。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn cache_get(conn: &Connection, key: &str) -> Result<Option<String>, DbError> {
    cache_get_at(conn, key, current_utc_ms())
}

/// 查询缓存（可注入时间戳变体）：命中时用 `now_ms` 刷新 `last_used_utc`；未命中返回 None。
///
/// 与 `cache_get` 行为完全一致，区别是刷新时间戳由调用方传入，便于测试控制
/// LRU 刷新路径，避免依赖系统时钟导致测试不确定。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn cache_get_at(
    conn: &Connection,
    key: &str,
    now_ms: i64,
) -> Result<Option<String>, DbError> {
    use rusqlite::OptionalExtension;

    let result: Option<String> = conn
        .query_row(
            "SELECT translated FROM translation_cache WHERE cache_key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .optional()?;

    if result.is_some() {
        conn.execute(
            "UPDATE translation_cache SET last_used_utc = ?1 WHERE cache_key = ?2",
            rusqlite::params![now_ms, key],
        )?;
    }

    Ok(result)
}

/// 写入缓存条目（使用当前系统时间），写后若超 capacity 则 LRU 淘汰。
///
/// 相同 `cache_key` 时 UPSERT（更新 translated / last_used_utc）。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn cache_put(
    conn: &Connection,
    source_text: &str,
    source_lang: &str,
    target_lang: &str,
    provider_id: &str,
    translated: &str,
    capacity: usize,
) -> Result<(), DbError> {
    let entry = CacheEntry {
        source_text,
        source_lang,
        target_lang,
        provider_id,
        translated,
    };
    cache_put_at(conn, &entry, capacity, current_utc_ms())
}

/// 写入缓存条目（带显式时间戳），写后若超 capacity 则 LRU 淘汰。
///
/// 相同 `cache_key` 时 UPSERT（更新 translated / last_used_utc）。
/// 供测试注入可控时间戳，避免同毫秒内 LRU 顺序不确定。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn cache_put_at(
    conn: &Connection,
    entry: &CacheEntry<'_>,
    capacity: usize,
    now_ms: i64,
) -> Result<(), DbError> {
    let key = cache_key(
        entry.source_text,
        entry.source_lang,
        entry.target_lang,
        entry.provider_id,
    );

    conn.execute(
        "INSERT INTO translation_cache
             (cache_key, source_text, source_lang, target_lang, provider_id,
              translated, created_utc, last_used_utc)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT(cache_key) DO UPDATE SET
             translated    = excluded.translated,
             last_used_utc = excluded.last_used_utc",
        rusqlite::params![
            key,
            entry.source_text,
            entry.source_lang,
            entry.target_lang,
            entry.provider_id,
            entry.translated,
            now_ms
        ],
    )?;

    cache_evict_lru(conn, capacity)?;
    Ok(())
}

/// LRU 淘汰：当表中行数超过 capacity 时，物理删除 `last_used_utc` 最旧的若干条，
/// 直到行数等于 capacity 为止。返回实际删除条数。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn cache_evict_lru(conn: &Connection, capacity: usize) -> Result<usize, DbError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM translation_cache",
        [],
        |row| row.get(0),
    )?;

    let cap = capacity as i64;
    if count <= cap {
        return Ok(0);
    }

    let to_delete = count - cap;
    conn.execute(
        "DELETE FROM translation_cache
         WHERE cache_key IN (
             SELECT cache_key FROM translation_cache
             ORDER BY last_used_utc ASC
             LIMIT ?1
         )",
        rusqlite::params![to_delete],
    )?;

    Ok(to_delete as usize)
}

/// 获取当前 UTC 时间戳（毫秒）。
fn current_utc_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
