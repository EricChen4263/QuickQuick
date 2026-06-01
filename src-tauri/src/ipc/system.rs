//! 系统域 IPC 命令层（里程碑 3 · V5-F-system）
//!
//! 命令清单（前端通过 invoke 对应命令名调用）：
//! - `get_storage_stats`           — 存储统计（活跃条目数 + 库文件大小）
//! - `cleanup_history`             — 清理历史（容量裁剪 + GC 物理删除）
//! - `open_accessibility_settings` — 打开 macOS 辅助功能系统设置深链
//! - `paste_to_front`              — 将指定条目写回系统剪贴板（降级实现）
//!
//! 降级说明（paste_to_front）：
//! 当前实现仅写回剪贴板，不模拟 ⌘V 按键注入。真实自动粘贴需要
//! Accessibility 授权 + CGEvent，留后续版本实现（见 paste_to_front_impl 注释）。

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::{Manager, State};

use crate::db::{self, DbError};
use crate::ipc::{with_db, AppDb};
use crate::onboarding::ACCESSIBILITY_DEEPLINK;

/// 存储统计 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatsDto {
    /// 活跃（未软删）条目总数
    pub live_count: i64,
    /// 数据库文件大小（字节），文件不存在时为 0
    pub file_size_bytes: u64,
}

/// 历史清理结果 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResultDto {
    /// 本轮软删条目数（超出保留上限的旧条目）
    pub soft_deleted: usize,
    /// 物理删除的墓碑行数
    pub purged: u64,
}

/// 粘贴结果 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteResultDto {
    /// 粘贴路径："full_paste" | "write_back_only"
    pub outcome: String,
}

/// 保留条目上限：超过此数量的非收藏旧条目将被软删。
const KEEP_RECENT_COUNT: usize = 500;

/// get_storage_stats 的纯函数实现，可在测试中直接调用。
///
/// 返回活跃条目数与 db 文件大小。`db_path` 为 None 时文件大小返回 0。
///
/// # Errors
/// 数据库查询失败时返回 `DbError`。
pub fn get_storage_stats_impl(
    conn: &Connection,
    db_path: Option<&std::path::Path>,
) -> Result<StorageStatsDto, DbError> {
    let live_count = db::count_live(conn)?;
    let file_size_bytes = db_path
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(StorageStatsDto {
        live_count,
        file_size_bytes,
    })
}

/// cleanup_history 的纯函数实现，可在测试中直接调用。
///
/// 两步清理：先软删超出 `KEEP_RECENT_COUNT` 的旧条目，再物理删除所有墓碑行。
///
/// # Errors
/// 数据库操作失败时返回 `DbError`。
pub fn cleanup_history_impl(conn: &Connection) -> Result<CleanupResultDto, DbError> {
    let soft_deleted = db::cleanup_keep_recent(conn, KEEP_RECENT_COUNT)?;
    let purged = db::gc_purge_deleted(conn)?;
    Ok(CleanupResultDto {
        soft_deleted,
        purged,
    })
}

/// paste_to_front 的纯函数实现，可在测试中直接调用。
///
/// 降级实现：从 DB 按 id 查文本内容 → 写回系统剪贴板 → 返回 "write_back_only"。
///
/// 图片条目（kind="image"）当前返回错误，不支持写回（arboard 图片格式转换留后续）。
///
/// 注意：真实自动粘贴需要 Accessibility 授权 + CGEvent ⌘V 注入，
/// 当前故意跳过，待后续版本在 onboarding 授权流程完整后补充。
///
/// # Errors
/// - id 不存在时返回错误
/// - 条目为图片类型时返回错误
/// - 剪贴板写入失败时返回 `DbError::Other`
pub fn paste_to_front_impl(conn: &Connection, id: &str) -> Result<PasteResultDto, DbError> {
    if id.trim().is_empty() {
        return Err(DbError::Other("id 不能为空".to_string()));
    }

    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT content, kind FROM clip_items WHERE id = ?1 AND is_deleted = 0",
            rusqlite::params![id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(DbError::Sqlite)?;

    let (content, kind) = row.ok_or_else(|| DbError::Other("条目不存在或已删除".to_string()))?;

    if kind == "image" {
        return Err(DbError::Other(
            "图片条目暂不支持写回剪贴板".to_string(),
        ));
    }

    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| DbError::Other(format!("剪贴板初始化失败：{e}")))?;
    clipboard
        .set_text(&content)
        .map_err(|e| DbError::Other(format!("写回剪贴板失败：{e}")))?;

    Ok(PasteResultDto {
        outcome: "write_back_only".to_string(),
    })
}

/// Tauri 命令：取存储统计（活跃条目数 + 库文件大小）。
#[tauri::command]
pub fn get_storage_stats(
    state: State<'_, AppDb>,
    app: tauri::AppHandle,
) -> Result<StorageStatsDto, String> {
    let db_path = app
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("quickquick.db"));

    with_db(&state, |conn| {
        get_storage_stats_impl(conn, db_path.as_deref()).map_err(|e| e.to_string())
    })
}

/// Tauri 命令：清理历史（容量裁剪 + GC）。
#[tauri::command]
pub fn cleanup_history(state: State<'_, AppDb>) -> Result<CleanupResultDto, String> {
    with_db(&state, |conn| {
        cleanup_history_impl(conn).map_err(|e| e.to_string())
    })
}

/// Tauri 命令：打开 macOS 辅助功能系统设置深链。
///
/// 使用 `std::process::Command("open")` 打开深链 URL（项目未依赖 tauri-plugin-opener，
/// Cargo.toml 中无此依赖，改用系统 `open` 命令直接打开）。
#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    std::process::Command::new("open")
        .arg(ACCESSIBILITY_DEEPLINK)
        .spawn()
        .map_err(|e| format!("无法打开辅助功能设置：{e}"))?;
    Ok(())
}

/// Tauri 命令：将指定条目写回系统剪贴板（降级实现）。
///
/// 返回 `{ outcome: "write_back_only" }`，不模拟 ⌘V 注入。
#[tauri::command]
pub fn paste_to_front(state: State<'_, AppDb>, id: String) -> Result<PasteResultDto, String> {
    with_db(&state, |conn| {
        paste_to_front_impl(conn, &id).map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ingest;

    /// 构造最小合法内存 SQLite，初始化 clip_items 表。
    fn make_test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("内存库开启失败");
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
             );",
        )
        .expect("建测试表失败");
        conn
    }

    /// T1：get_storage_stats_impl — 空库返回 live_count=0，file_size_bytes=0（无路径）
    #[test]
    fn get_storage_stats_impl_empty_db_returns_zero() {
        let conn = make_test_conn();
        let stats = get_storage_stats_impl(&conn, None).expect("不应失败");
        assert_eq!(stats.live_count, 0, "空库活跃数应为 0");
        assert_eq!(stats.file_size_bytes, 0, "无路径时文件大小应为 0");
    }

    /// T2：get_storage_stats_impl — 写入 2 条后 live_count=2
    #[test]
    fn get_storage_stats_impl_counts_live_items() {
        let conn = make_test_conn();
        let item_a = crate::clipboard::CapturedItem {
            text: "hello".to_string(),
            html: None,
        };
        let item_b = crate::clipboard::CapturedItem {
            text: "world".to_string(),
            html: None,
        };
        ingest(&conn, &item_a).expect("入库 A 失败");
        ingest(&conn, &item_b).expect("入库 B 失败");

        let stats = get_storage_stats_impl(&conn, None).expect("不应失败");
        assert_eq!(stats.live_count, 2, "写入 2 条后 live_count 应为 2");
    }

    /// T3：cleanup_history_impl — 无超限时 soft_deleted=0，purged=0
    #[test]
    fn cleanup_history_impl_no_excess_returns_zeros() {
        let conn = make_test_conn();
        let item = crate::clipboard::CapturedItem {
            text: "only one".to_string(),
            html: None,
        };
        ingest(&conn, &item).expect("入库失败");

        let result = cleanup_history_impl(&conn).expect("不应失败");
        assert_eq!(result.soft_deleted, 0, "未超限时 soft_deleted 应为 0");
        assert_eq!(result.purged, 0, "无墓碑时 purged 应为 0");
    }

    /// T4：paste_to_front_impl — 空 id 应返回 Err
    #[test]
    fn paste_to_front_impl_empty_id_returns_err() {
        let conn = make_test_conn();
        let result = paste_to_front_impl(&conn, "");
        assert!(result.is_err(), "空 id 应返回错误");
    }

    /// T5：paste_to_front_impl — 不存在的 id 应返回 Err
    #[test]
    fn paste_to_front_impl_nonexistent_id_returns_err() {
        let conn = make_test_conn();
        let result = paste_to_front_impl(&conn, "nonexistent-uuid");
        assert!(result.is_err(), "不存在的 id 应返回错误");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("不存在") || msg.contains("条目"),
            "错误信息应说明条目不存在，实际：{msg}"
        );
    }
}
