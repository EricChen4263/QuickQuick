//! 富文本还原（粘贴 + 复制后端）集成测试（RT1-F1-S04）。
//!
//! 走真实加密库 open_or_create，验证取数+组装两个半纯函数在加密整库上成立：
//! - fetch_paste_item：粘贴链路取数，富文本条目带回 html、纯文本 html==None。
//! - fetch_clip_for_copy：复制命令取数，富文本 text+html 齐、纯文本 html==None。
//!
//! arboard set().html 实写系统剪贴板属 GUI 副作用，不在自动化覆盖（归 RT1-M01 manual_confirm）。

use quickquick_lib::clipboard::CapturedItem;
use quickquick_lib::db;
use quickquick_lib::ipc::system::{fetch_clip_for_copy, fetch_paste_item};

/// 构造临时加密库路径（测试结束随 TempDir 清理）。
fn temp_db() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("clips.db");
    (dir, path)
}

/// 入库一条条目并返回其 id。
fn ingest_item(conn: &rusqlite::Connection, text: &str, html: Option<&str>) -> String {
    let item = CapturedItem {
        text: text.to_string(),
        html: html.map(|h| h.to_string()),
    };
    match db::ingest(conn, &item).expect("ingest 失败") {
        db::IngestOutcome::Inserted(id) => id,
        db::IngestOutcome::Bumped(_) => panic!("首次应 Inserted"),
    }
}

#[test]
fn fetch_paste_item_includes_html() {
    let (_dir, path) = temp_db();
    let conn = db::open_or_create(&path, &[7u8; 32]).expect("开库失败");
    let rich_id = ingest_item(&conn, "hello", Some("<b>hello</b>"));
    let plain_id = ingest_item(&conn, "plain text", None);

    let rich = fetch_paste_item(&conn, &rich_id).expect("取富文本条目失败");
    assert_eq!(rich.text, "hello", "纯文本字段应保留");
    assert_eq!(
        rich.html.as_deref(),
        Some("<b>hello</b>"),
        "富文本条目应带回原 html"
    );

    let plain = fetch_paste_item(&conn, &plain_id).expect("取纯文本条目失败");
    assert_eq!(plain.html, None, "纯文本条目 html 应为 None");
}

#[test]
fn copy_clip_assembles_text_and_html() {
    let (_dir, path) = temp_db();
    let conn = db::open_or_create(&path, &[9u8; 32]).expect("开库失败");
    let rich_id = ingest_item(&conn, "world", Some("<i>world</i>"));
    let plain_id = ingest_item(&conn, "just text", None);

    let rich = fetch_clip_for_copy(&conn, &rich_id).expect("取富文本条目失败");
    assert_eq!(rich.text, "world", "纯文本字段应保留");
    assert_eq!(
        rich.html.as_deref(),
        Some("<i>world</i>"),
        "富文本条目应带回原 html"
    );

    let plain = fetch_clip_for_copy(&conn, &plain_id).expect("取纯文本条目失败");
    assert_eq!(plain.text, "just text", "纯文本字段应保留");
    assert_eq!(plain.html, None, "纯文本条目 html 应为 None");
}
