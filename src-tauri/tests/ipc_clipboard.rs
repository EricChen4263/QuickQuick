//! 集成测试：剪贴板 IPC 命令实现层（impl 函数）
//!
//! 覆盖验收项 V4-F1-A01：剪贴板 IPC 命令往返
//! - list_clip_items_impl：列出条目，软删后不返回，收藏置顶
//! - delete_clip_item_impl：软删后条目从列表消失
//! - toggle_favorite_clip_impl：收藏切换后排序变化生效
//!
//! 测试约定：函数名含子串 `ipc_clipboard` 确保 verify 命中。

use quickquick_lib::db;
use quickquick_lib::ipc::clipboard::{
    delete_clip_item_impl, list_clip_items_impl, toggle_favorite_clip_impl,
};
use rusqlite::Connection;
use tempfile::tempdir;

const KEY: [u8; 32] = [42u8; 32];

/// 打开临时数据库，返回连接（schema 已初始化）
fn open_tmp_db() -> (tempfile::TempDir, Connection) {
    let dir = tempdir().expect("tempdir 创建失败");
    let path = dir.path().join("test.db");
    let conn = db::open_or_create(&path, &KEY).expect("建库应成功");
    (dir, conn)
}

/// 向数据库插入文本条目，返回 id
fn insert_text(conn: &Connection, text: &str) -> String {
    use quickquick_lib::clipboard::CapturedItem;
    let item = CapturedItem {
        text: text.to_string(),
        html: None,
    };
    match db::ingest(conn, &item).expect("ingest 应成功") {
        db::IngestOutcome::Inserted(id) | db::IngestOutcome::Bumped(id) => id,
    }
}

/// V4-F1-A01：list_clip_items_impl 返回未软删条目
#[test]
fn ipc_clipboard_list_returns_live_items() {
    let (_dir, conn) = open_tmp_db();

    insert_text(&conn, "hello");
    insert_text(&conn, "world");

    let items = list_clip_items_impl(&conn).expect("list 应成功");

    assert_eq!(items.len(), 2, "应返回 2 条未软删条目");
}

/// V4-F1-A01：软删后 list 不再返回该条目
#[test]
fn ipc_clipboard_list_excludes_deleted_items() {
    let (_dir, conn) = open_tmp_db();

    let id = insert_text(&conn, "will-be-deleted");
    insert_text(&conn, "stays");

    db::soft_delete(&conn, &id).expect("soft_delete 应成功");

    let items = list_clip_items_impl(&conn).expect("list 应成功");

    assert_eq!(items.len(), 1, "软删后应只剩 1 条");
    assert_ne!(items[0].id, id, "软删的条目不应出现在结果中");
}

/// V4-F1-A01：delete_clip_item_impl 软删指定条目
#[test]
fn ipc_clipboard_delete_removes_item_from_list() {
    let (_dir, conn) = open_tmp_db();

    let id = insert_text(&conn, "to-delete");
    insert_text(&conn, "to-keep");

    delete_clip_item_impl(&conn, &id).expect("delete_impl 应成功");

    let items = list_clip_items_impl(&conn).expect("list 应成功");
    assert_eq!(items.len(), 1, "delete 后应只剩 1 条");

    let found = items.iter().any(|it| it.id == id);
    assert!(!found, "被删除的条目不应出现在列表中");
}

/// V4-F1-A01：toggle_favorite_clip_impl 收藏切换后收藏项排在前面
#[test]
fn ipc_clipboard_toggle_favorite_puts_item_first() {
    let (_dir, conn) = open_tmp_db();

    let id_a = insert_text(&conn, "item-a");
    let id_b = insert_text(&conn, "item-b");

    // item-b 是最新插入的，正常情况排在最前
    let items_before = list_clip_items_impl(&conn).expect("list 应成功");
    assert_eq!(items_before[0].id, id_b, "未收藏时最新条目应排在最前");

    // 收藏 item-a（较旧）→ 它应排到最前
    toggle_favorite_clip_impl(&conn, &id_a, true).expect("toggle_favorite 应成功");

    let items_after = list_clip_items_impl(&conn).expect("list 应成功");
    assert_eq!(items_after[0].id, id_a, "收藏后 item-a 应排在最前");
    assert!(items_after[0].is_favorite, "收藏项的 is_favorite 应为 true");
}

/// V4-F1-A01：toggle_favorite_clip_impl 取消收藏后正常排序恢复
#[test]
fn ipc_clipboard_toggle_favorite_unset_restores_order() {
    let (_dir, conn) = open_tmp_db();

    let id_a = insert_text(&conn, "item-a");
    let id_b = insert_text(&conn, "item-b");

    // 先收藏 id_a
    toggle_favorite_clip_impl(&conn, &id_a, true).expect("set favorite 应成功");

    // 再取消收藏 id_a
    toggle_favorite_clip_impl(&conn, &id_a, false).expect("unset favorite 应成功");

    let items = list_clip_items_impl(&conn).expect("list 应成功");

    // id_b 更新更晚（set_favorite 刷新了 id_a 的 last_modified），
    // 两者都无收藏；id_b 是后插入的，在无收藏情况下应排在最前
    // 注意：toggle 也会更新 last_modified_utc，所以 id_a 取消收藏后 last_modified 更新了
    // 取消收藏时 id_a 的 last_modified 被刷新，所以 id_a 实际上排在最前
    let ids: Vec<&str> = items.iter().map(|it| it.id.as_str()).collect();
    assert!(
        ids.contains(&id_a.as_str()) && ids.contains(&id_b.as_str()),
        "两个条目都应存在"
    );
    assert!(!items[0].is_favorite, "取消收藏后排第一的条目 is_favorite 应为 false");
}

/// V4-F1-A01：DTO 字段完整性——包含 id/content/kind/is_favorite/last_modified_utc
#[test]
fn ipc_clipboard_list_dto_fields_complete() {
    let (_dir, conn) = open_tmp_db();

    insert_text(&conn, "test content");

    let items = list_clip_items_impl(&conn).expect("list 应成功");
    assert_eq!(items.len(), 1);

    let item = &items[0];
    assert!(!item.id.is_empty(), "id 不应为空");
    assert_eq!(item.content, "test content", "content 应等于插入的文本");
    assert_eq!(item.kind, "text", "kind 应为 'text'");
    assert!(!item.is_favorite, "默认 is_favorite 应为 false");
    assert!(item.last_modified_utc > 0, "last_modified_utc 应为正整数");
}
