//! 富文本存储层集成测试（走真实加密库 open_or_create，端到端验证 schema + ingest + 查询）。
//!
//! 与 db.rs 内联单测互补：内联单测用内存库快测迁移幂等/补写逻辑；
//! 本文件用真实 SQLCipher 文件库，证明 ensure_schema 建表已含 html_content、
//! ingest→list 的富文本 roundtrip 在加密整库上同样成立（防"内存库过、真库不过"假绿）。

use quickquick_lib::clipboard::CapturedItem;
use quickquick_lib::db;

/// 构造临时加密库路径（测试结束随 TempDir 清理）。
fn temp_db() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("clips.db");
    (dir, path)
}

#[test]
fn fresh_db_persists_richtext_roundtrip_through_encrypted_store() {
    let (_dir, path) = temp_db();
    let key = [7u8; 32];
    let conn = db::open_or_create(&path, &key).expect("开库失败");

    let html = "<b>bold</b>".to_string();
    let item = CapturedItem {
        text: "bold".to_string(),
        html: Some(html.clone()),
    };
    let outcome = db::ingest(&conn, &item).expect("ingest 失败");
    let id = match outcome {
        db::IngestOutcome::Inserted(id) => id,
        db::IngestOutcome::Bumped(_) => panic!("首次应 Inserted"),
    };

    let rows = db::list_items_full(&conn).expect("list 失败");
    let row = rows.iter().find(|r| r.id == id).expect("应查到该行");
    assert_eq!(row.kind, "richtext", "带 html 条目 kind 应为 richtext");
    assert_eq!(
        row.html_content.as_deref(),
        Some(html.as_str()),
        "加密库读回 html_content 应一致"
    );
}

#[test]
fn plaintext_dedup_unchanged_on_encrypted_store() {
    let (_dir, path) = temp_db();
    let key = [9u8; 32];
    let conn = db::open_or_create(&path, &key).expect("开库失败");

    let item = CapturedItem {
        text: "dup".to_string(),
        html: None,
    };
    db::ingest(&conn, &item).expect("首次 ingest 失败");
    let second = db::ingest(&conn, &item).expect("二次 ingest 失败");
    assert!(
        matches!(second, db::IngestOutcome::Bumped(_)),
        "相同纯文本二次应 Bumped"
    );
    assert_eq!(db::count_live(&conn).expect("count 失败"), 1, "去重后应仅 1 行");
}
