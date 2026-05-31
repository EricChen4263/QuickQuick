//! 集成测试：剪贴板 IPC 命令参数校验（V4-F1-A14 剪贴板部分）
//!
//! 覆盖验收项 V4-F1-A14：命令参数校验
//! - 空 id 返回 Err 而非 panic
//! - 全空白 id 返回 Err 而非 panic
//! - 合法 id 正常执行（不因校验误杀）
//!
//! 测试约定：函数名含子串 `ipc_input_validation` 确保 verify 命中。

use quickquick_lib::db;
use quickquick_lib::ipc::clipboard::{
    delete_clip_item_impl, toggle_favorite_clip_impl,
};
use rusqlite::Connection;
use tempfile::tempdir;

const KEY: [u8; 32] = [55u8; 32];

fn open_tmp_db() -> (tempfile::TempDir, Connection) {
    let dir = tempdir().expect("tempdir 创建失败");
    let path = dir.path().join("test_validation.db");
    let conn = db::open_or_create(&path, &KEY).expect("建库应成功");
    (dir, conn)
}

/// V4-F1-A14：delete_clip_item_impl 传空串 → 返回 Err，不 panic
#[test]
fn ipc_input_validation_delete_empty_id_returns_err() {
    let (_dir, conn) = open_tmp_db();

    let result = delete_clip_item_impl(&conn, "");

    assert!(result.is_err(), "空 id 应返回 Err");
}

/// V4-F1-A14：delete_clip_item_impl 传全空白 id → 返回 Err，不 panic
#[test]
fn ipc_input_validation_delete_whitespace_id_returns_err() {
    let (_dir, conn) = open_tmp_db();

    let result = delete_clip_item_impl(&conn, "   ");

    assert!(result.is_err(), "全空白 id 应返回 Err");
}

/// V4-F1-A14：toggle_favorite_clip_impl 传空串 → 返回 Err，不 panic
#[test]
fn ipc_input_validation_toggle_favorite_empty_id_returns_err() {
    let (_dir, conn) = open_tmp_db();

    let result = toggle_favorite_clip_impl(&conn, "", true);

    assert!(result.is_err(), "空 id 应返回 Err");
}

/// V4-F1-A14：toggle_favorite_clip_impl 传全空白 id → 返回 Err，不 panic
#[test]
fn ipc_input_validation_toggle_favorite_whitespace_id_returns_err() {
    let (_dir, conn) = open_tmp_db();

    let result = toggle_favorite_clip_impl(&conn, "\t\n ", false);

    assert!(result.is_err(), "全空白 id 应返回 Err");
}

/// V4-F1-A14：合法 id（即使在库中不存在）不触发校验错误
///
/// id 格式合法时应通过校验层，执行 SQL（影响行数为 0 但不报错）。
#[test]
fn ipc_input_validation_delete_valid_nonexistent_id_passes_validation() {
    let (_dir, conn) = open_tmp_db();

    let result = delete_clip_item_impl(&conn, "550e8400-e29b-41d4-a716-446655440000");

    // 合法 id 不应因校验失败返回 Err（id 不存在但 SQL 不报错）
    assert!(
        result.is_ok(),
        "合法 UUID 格式 id 应通过校验（即使不存在于库中），实际: {:?}",
        result.err()
    );
}

/// V4-F1-A14：toggle_favorite 合法 id 通过校验
#[test]
fn ipc_input_validation_toggle_favorite_valid_id_passes_validation() {
    let (_dir, conn) = open_tmp_db();

    let result = toggle_favorite_clip_impl(&conn, "550e8400-e29b-41d4-a716-446655440000", true);

    assert!(
        result.is_ok(),
        "合法 UUID 格式 id 应通过校验，实际: {:?}",
        result.err()
    );
}
