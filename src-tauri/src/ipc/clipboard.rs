//! 剪贴板 IPC 命令层
//!
//! 模式：每个命令 = 薄的 `#[tauri::command]` 包装 + 可单测的纯函数 impl。
//! 单测只测 impl 函数（传 `&Connection`），命令层把 `DbError` 映射为 `String`。
//!
//! 命令清单（前端通过 invoke 对应的命令名调用）：
//! - `list_clip_items`         — 返回全部未软删条目（收藏优先）
//! - `delete_clip_item`        — 软删指定 id 的条目
//! - `toggle_favorite_clip`    — 设置或取消指定条目的收藏状态

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::db::{self, DbError};
use crate::ipc::AppDb;

/// 前端展示用 DTO：剪贴板条目
///
/// 字段用 camelCase 序列化，与前端 TypeScript 接口对齐。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItemDto {
    pub id: String,
    pub content: String,
    pub kind: String,
    pub is_favorite: bool,
    pub last_modified_utc: i64,
}

/// list_clip_items 的纯函数实现，可在测试中直接调用。
///
/// 返回所有未软删条目，按收藏优先、组内最近修改时间降序排列。
///
/// # Errors
/// 数据库查询失败时返回 `DbError`。
pub fn list_clip_items_impl(conn: &Connection) -> Result<Vec<ClipItemDto>, DbError> {
    let rows = db::list_items_full(conn)?;
    let dtos = rows
        .into_iter()
        .map(|r| ClipItemDto {
            id: r.id,
            content: r.content,
            kind: r.kind,
            is_favorite: r.is_favorite,
            last_modified_utc: r.last_modified_utc,
        })
        .collect();
    Ok(dtos)
}

/// delete_clip_item 的纯函数实现，可在测试中直接调用。
///
/// 对 id 做前置校验：空串或全空白直接返回 Err，不执行 SQL。
/// 这样避免无效 id 静默通过，也保证不会因空 id 触发非预期行为。
///
/// # Errors
/// - id 为空或全空白时返回 `DbError::Other`
/// - 数据库操作失败时返回 `DbError::Sqlite`
pub fn delete_clip_item_impl(conn: &Connection, id: &str) -> Result<(), DbError> {
    validate_id(id)?;
    db::soft_delete(conn, id)
}

/// toggle_favorite_clip 的纯函数实现，可在测试中直接调用。
///
/// 对 id 做前置校验：空串或全空白直接返回 Err，不执行 SQL。
///
/// # Errors
/// - id 为空或全空白时返回 `DbError::Other`
/// - 数据库操作失败时返回 `DbError::Sqlite`
pub fn toggle_favorite_clip_impl(
    conn: &Connection,
    id: &str,
    favorite: bool,
) -> Result<(), DbError> {
    validate_id(id)?;
    db::set_favorite(conn, id, favorite)
}

/// 校验 id 参数：空串或全空白视为非法输入。
///
/// 为何在命令层校验而非 db 层：db 层函数语义是「执行 SQL」，
/// 合法/非法输入判定属于 API 契约，应在边界层（IPC 命令层）强制。
fn validate_id(id: &str) -> Result<(), DbError> {
    if id.trim().is_empty() {
        return Err(DbError::Other("id 不能为空或全空白".to_string()));
    }
    Ok(())
}

/// Tauri 命令：列出所有未软删的剪贴板条目（收藏优先）。
#[tauri::command]
pub fn list_clip_items(state: State<'_, AppDb>) -> Result<Vec<ClipItemDto>, String> {
    let conn = state.0.lock().map_err(|e| format!("锁获取失败: {e}"))?;
    list_clip_items_impl(&conn).map_err(|e| e.to_string())
}

/// Tauri 命令：软删指定剪贴板条目。
#[tauri::command]
pub fn delete_clip_item(state: State<'_, AppDb>, id: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| format!("锁获取失败: {e}"))?;
    delete_clip_item_impl(&conn, &id).map_err(|e| e.to_string())
}

/// Tauri 命令：设置或取消剪贴板条目的收藏状态。
#[tauri::command]
pub fn toggle_favorite_clip(
    state: State<'_, AppDb>,
    id: String,
    favorite: bool,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| format!("锁获取失败: {e}"))?;
    toggle_favorite_clip_impl(&conn, &id, favorite).map_err(|e| e.to_string())
}
