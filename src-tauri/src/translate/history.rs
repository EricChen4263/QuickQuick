//! 翻译历史模块（V2-F3-A14）
//!
//! 职责：
//! 1. 向独立的 `translate_history` 表写入翻译历史条目
//! 2. 支持剪贴板条目「一键翻译」：读 clip_items 原文，写入 translate_history
//! 3. 翻译历史与剪贴板历史严格分开存储，互不混入

use rusqlite::Connection;
use uuid::Uuid;

use crate::db::DbError;

/// 向 `translate_history` 表插入一条翻译历史，返回新记录的 id。
///
/// 所有文本字段均使用参数化查询，不拼接 SQL（安全§10）。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn add_translate_history(
    conn: &Connection,
    source_text: &str,
    translated_text: &str,
    source_lang: &str,
    target_lang: &str,
    provider_id: &str,
) -> Result<String, DbError> {
    let id = Uuid::new_v4().to_string();
    let now_ms = current_utc_ms();
    conn.execute(
        "INSERT INTO translate_history
             (id, source_text, translated_text, source_lang, target_lang, provider_id, created_utc)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, source_text, translated_text, source_lang, target_lang, provider_id, now_ms],
    )?;
    Ok(id)
}

/// 查询 `translate_history` 表的记录总数。
///
/// 用于测试断言和业务层「翻译历史条目数」查询。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn translate_history_count(conn: &Connection) -> Result<i64, DbError> {
    let n = conn.query_row(
        "SELECT COUNT(*) FROM translate_history",
        [],
        |row| row.get(0),
    )?;
    Ok(n)
}

/// 剪贴板条目「一键翻译」：读取 clip_items 中指定条目的原文，写入 translate_history。
///
/// 返回新翻译历史条目的 id。
/// clip_items 本身不受影响（独立存储语义）。
///
/// # Errors
/// - `DbError::Sqlite`：SQL 执行失败，或 `clip_id` 不存在（query_row 返回 QueryReturnedNoRows）
pub fn translate_clip_item(
    conn: &Connection,
    clip_id: &str,
    translated_text: &str,
    source_lang: &str,
    target_lang: &str,
    provider_id: &str,
) -> Result<String, DbError> {
    let source_text: String = conn.query_row(
        "SELECT content FROM clip_items WHERE id = ?1 AND is_deleted = 0",
        rusqlite::params![clip_id],
        |row| row.get(0),
    )?;
    add_translate_history(conn, &source_text, translated_text, source_lang, target_lang, provider_id)
}

/// 获取当前 UTC 时间戳（毫秒）。
fn current_utc_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
