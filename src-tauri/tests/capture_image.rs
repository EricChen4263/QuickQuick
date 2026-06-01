//! 集成测试：图片剪贴板捕获层（V5-F1-S02 捕获层）
//!
//! 验收项：
//! - poll_text_only        — 纯文本快照 → [CapturedClip::Text]
//! - poll_image_only       — 纯图快照   → [CapturedClip::Image]
//! - poll_text_and_image   — 图文快照   → [Text, Image]（顺序确定）
//! - poll_self_marker      — has_self_marker → []
//! - poll_privacy_skip     — policy 命中 → []
//! - poll_no_change        — 计数未递增 → []
//! - rgba_to_png_valid     — 合法 2×2 RGBA → 可解码为 PNG
//! - rgba_to_png_bad_len   — 字节长度与尺寸不符 → None
//! - capture_and_ingest_text_image — 图文快照 → clip_items 1文本+1图片，clip_images 1行
//! - capture_and_ingest_image_only — 纯图快照 → 图片正确入库

use quickquick_lib::clipboard::{
    poll_once_with_policy, CapturedClip, ClipboardBackend, ClipboardSnapshot, RawImageData,
};
use quickquick_lib::keyprovider::{KeyError, KeyProvider};
use quickquick_lib::pipeline;
use quickquick_lib::privacy::{CapturePolicy, ExcludeList};
use tempfile::tempdir;

/// 最小合法 2×2 RGBA（16 字节，供多个测试共用）
fn make_2x2_rgba() -> Vec<u8> {
    vec![
        255, 0, 0, 255, // 像素 (0,0) 红
        0, 255, 0, 255, // 像素 (1,0) 绿
        0, 0, 255, 255, // 像素 (0,1) 蓝
        255, 255, 0, 255, // 像素 (1,1) 黄
    ]
}

/// 可编程的假剪贴板后端（支持注入图片）。
struct FakeBackend {
    count: u64,
    snapshot: ClipboardSnapshot,
}

impl FakeBackend {
    fn new_text(count: u64, text: &str) -> Self {
        Self {
            count,
            snapshot: ClipboardSnapshot {
                text: Some(text.to_owned()),
                html: None,
                image: None,
                has_self_marker: false,
                is_concealed: false,
                source_app: None,
            },
        }
    }

    fn new_image(count: u64, width: usize, height: usize, bytes: Vec<u8>) -> Self {
        Self {
            count,
            snapshot: ClipboardSnapshot {
                text: None,
                html: None,
                image: Some(RawImageData {
                    width,
                    height,
                    bytes,
                }),
                has_self_marker: false,
                is_concealed: false,
                source_app: None,
            },
        }
    }

    fn new_text_and_image(
        count: u64,
        text: &str,
        width: usize,
        height: usize,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            count,
            snapshot: ClipboardSnapshot {
                text: Some(text.to_owned()),
                html: None,
                image: Some(RawImageData {
                    width,
                    height,
                    bytes,
                }),
                has_self_marker: false,
                is_concealed: false,
                source_app: None,
            },
        }
    }

    fn new_self_marker(count: u64) -> Self {
        Self {
            count,
            snapshot: ClipboardSnapshot {
                text: Some("ignored".to_owned()),
                html: None,
                image: None,
                has_self_marker: true,
                is_concealed: false,
                source_app: None,
            },
        }
    }
}

impl ClipboardBackend for FakeBackend {
    fn change_count(&self) -> u64 {
        self.count
    }

    fn read(&self) -> ClipboardSnapshot {
        self.snapshot.clone()
    }
}

/// 固定密钥 KeyProvider（不触碰真实钥匙串）
struct FixedKeyProvider;

impl KeyProvider for FixedKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], KeyError> {
        Ok([42u8; 32])
    }
}

// poll_once_with_policy 系列

/// 纯文本快照 → Vec 长度 1，唯一元素为 CapturedClip::Text
#[test]
fn poll_text_only() {
    let backend = FakeBackend::new_text(2, "hello");
    let mut last_seen = 1u64;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy {
        paused: false,
        exclude: &exclude,
    };

    let result = poll_once_with_policy(&backend, &mut last_seen, &policy);

    assert_eq!(result.len(), 1, "纯文本应返回长度 1 的 Vec");
    assert!(
        matches!(&result[0], CapturedClip::Text(item) if item.text == "hello"),
        "第一项应为 CapturedClip::Text(\"hello\")"
    );
    assert_eq!(last_seen, 2);
}

/// 纯图快照（text=None, image=Some） → Vec 长度 1，唯一元素为 CapturedClip::Image
#[test]
fn poll_image_only() {
    let rgba = make_2x2_rgba();
    let backend = FakeBackend::new_image(3, 2, 2, rgba);
    let mut last_seen = 2u64;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy {
        paused: false,
        exclude: &exclude,
    };

    let result = poll_once_with_policy(&backend, &mut last_seen, &policy);

    assert_eq!(result.len(), 1, "纯图应返回长度 1 的 Vec");
    assert!(
        matches!(&result[0], CapturedClip::Image { width: 2, height: 2, png_bytes } if !png_bytes.is_empty()),
        "第一项应为 Image {{ width:2, height:2, png_bytes:non-empty }}"
    );
    assert_eq!(last_seen, 3);
}

/// 图文快照（both）→ Vec 长度 2，顺序：[Text, Image]
#[test]
fn poll_text_and_image() {
    let rgba = make_2x2_rgba();
    let backend = FakeBackend::new_text_and_image(4, "caption", 2, 2, rgba);
    let mut last_seen = 3u64;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy {
        paused: false,
        exclude: &exclude,
    };

    let result = poll_once_with_policy(&backend, &mut last_seen, &policy);

    assert_eq!(result.len(), 2, "图文应返回长度 2 的 Vec");
    assert!(
        matches!(&result[0], CapturedClip::Text(item) if item.text == "caption"),
        "第 0 项应为 Text"
    );
    assert!(
        matches!(&result[1], CapturedClip::Image { .. }),
        "第 1 项应为 Image"
    );
    assert_eq!(last_seen, 4);
}

/// has_self_marker 命中 → 返回空 Vec，last_seen 仍推进
#[test]
fn poll_self_marker_returns_empty() {
    let backend = FakeBackend::new_self_marker(5);
    let mut last_seen = 4u64;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy {
        paused: false,
        exclude: &exclude,
    };

    let result = poll_once_with_policy(&backend, &mut last_seen, &policy);

    assert!(result.is_empty(), "has_self_marker 时应返回空 Vec");
    assert_eq!(last_seen, 5, "last_seen 应推进到 5");
}

/// privacy::should_skip 命中（paused=true）→ 返回空 Vec，last_seen 推进
#[test]
fn poll_privacy_skip_returns_empty() {
    let backend = FakeBackend::new_text(6, "private");
    let mut last_seen = 5u64;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy {
        paused: true,
        exclude: &exclude,
    };

    let result = poll_once_with_policy(&backend, &mut last_seen, &policy);

    assert!(result.is_empty(), "paused=true 时应返回空 Vec");
    assert_eq!(last_seen, 6, "last_seen 应推进到 6");
}

/// 计数未递增 → 返回空 Vec，last_seen 不变
#[test]
fn poll_no_change_returns_empty() {
    let backend = FakeBackend::new_text(5, "same");
    let mut last_seen = 5u64;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy {
        paused: false,
        exclude: &exclude,
    };

    let result = poll_once_with_policy(&backend, &mut last_seen, &policy);

    assert!(result.is_empty(), "计数未递增应返回空 Vec");
    assert_eq!(last_seen, 5, "无变化时 last_seen 不应改变");
}

// rgba_to_png 系列

/// 合法 2×2 RGBA → 能被 image::load_from_memory 解码的 PNG 字节
#[test]
fn rgba_to_png_valid_encodes_decodable_png() {
    use quickquick_lib::clipboard::rgba_to_png_for_test;

    let rgba = make_2x2_rgba();
    let png = rgba_to_png_for_test(2, 2, &rgba);

    assert!(png.is_some(), "合法 2×2 RGBA 应能编码为 PNG");
    let png_bytes = png.unwrap();
    let decoded = image::load_from_memory(&png_bytes);
    assert!(decoded.is_ok(), "编码出的 PNG 应可被 image crate 解码");
    let img = decoded.unwrap();
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
}

/// 字节长度与尺寸不符 → None
#[test]
fn rgba_to_png_bad_length_returns_none() {
    use quickquick_lib::clipboard::rgba_to_png_for_test;

    let bad_bytes = vec![1u8; 7]; // 2×2×4=16，但只给 7 字节
    let result = rgba_to_png_for_test(2, 2, &bad_bytes);

    assert!(result.is_none(), "字节长度不符时应返回 None");
}

// capture_and_ingest 系列

/// 图文快照 → clip_items 含 1 文本行 + 1 图片行，clip_images 有 1 行
#[test]
fn capture_and_ingest_text_and_image() {
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let provider = FixedKeyProvider;
    let conn = pipeline::open_app_db(&provider, &db_path).expect("open_app_db 应成功");

    let rgba = make_2x2_rgba();
    let backend = FakeBackend::new_text_and_image(1, "caption text", 2, 2, rgba);
    let mut last_seen: u64 = 0;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy {
        paused: false,
        exclude: &exclude,
    };

    let outcomes = pipeline::capture_and_ingest(&backend, &mut last_seen, &conn, &policy)
        .expect("capture_and_ingest 应成功");

    assert_eq!(outcomes.len(), 2, "图文快照应产生 2 个 IngestOutcome");

    // 验证 clip_items 包含 1 个文本行 + 1 个图片行
    let text_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clip_items WHERE kind = 'text'",
            [],
            |row| row.get(0),
        )
        .expect("查询文本行应成功");
    let image_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clip_items WHERE kind = 'image'",
            [],
            |row| row.get(0),
        )
        .expect("查询图片行应成功");
    let clip_images_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM clip_images", [], |row| row.get(0))
        .expect("查询 clip_images 应成功");

    assert_eq!(text_count, 1, "应有 1 条文本 clip_item");
    assert_eq!(image_count, 1, "应有 1 条图片 clip_item");
    assert_eq!(clip_images_count, 1, "clip_images 应有 1 行");
}

/// 原子性回滚：文本成功+图片失败 → 整体回滚，clip_items 无残留。
///
/// 构造缺 clip_images 表的 in-memory 连接，使 ingest_image_as_clip 因
/// "no such table: clip_images" 失败；图文快照时文本已成功写入，
/// 原子化后文本也应被回滚，clip_items 最终为 0 行。
#[test]
fn capture_and_ingest_rolls_back_on_partial_failure() {
    use rusqlite::Connection;

    // 建只有 clip_items 表、没有 clip_images 表的 in-memory 连接。
    // schema 与 ensure_schema 中完全一致（含 is_deleted/text_hash 等列），
    // 使 db::ingest 能正常写入文本行；db::ingest_image_as_clip 则因
    // "no such table: clip_images" 而失败，触发原子性回滚路径。
    let conn = Connection::open_in_memory().expect("in-memory 连接应成功");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS clip_items (
            id                TEXT PRIMARY KEY NOT NULL,
            content           TEXT,
            kind              TEXT    NOT NULL DEFAULT 'text',
            created_utc       INTEGER NOT NULL,
            last_modified_utc INTEGER NOT NULL,
            is_deleted        INTEGER NOT NULL DEFAULT 0,
            deleted_at_utc    INTEGER,
            text_hash         TEXT,
            is_favorite       INTEGER NOT NULL DEFAULT 0
        );",
    )
    .expect("建 clip_items 表应成功");

    let rgba = make_2x2_rgba();
    let backend = FakeBackend::new_text_and_image(1, "should_rollback", 2, 2, rgba);
    let mut last_seen: u64 = 0;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy {
        paused: false,
        exclude: &exclude,
    };

    let result = pipeline::capture_and_ingest(&backend, &mut last_seen, &conn, &policy);

    assert!(result.is_err(), "图片失败时整体应返回 Err");

    let remaining: i64 = conn
        .query_row("SELECT COUNT(*) FROM clip_items", [], |row| row.get(0))
        .expect("查询 clip_items 应成功");
    assert_eq!(
        remaining, 0,
        "整体回滚后 clip_items 应为 0 行（文本不应残留）"
    );
}

/// 纯图快照 → 图片正确入库（kind='image'，clip_images 1行）
#[test]
fn capture_and_ingest_image_only() {
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let provider = FixedKeyProvider;
    let conn = pipeline::open_app_db(&provider, &db_path).expect("open_app_db 应成功");

    let rgba = make_2x2_rgba();
    let backend = FakeBackend::new_image(1, 2, 2, rgba);
    let mut last_seen: u64 = 0;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy {
        paused: false,
        exclude: &exclude,
    };

    let outcomes = pipeline::capture_and_ingest(&backend, &mut last_seen, &conn, &policy)
        .expect("capture_and_ingest 应成功");

    assert_eq!(outcomes.len(), 1, "纯图快照应产生 1 个 IngestOutcome");

    let image_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clip_items WHERE kind = 'image'",
            [],
            |row| row.get(0),
        )
        .expect("查询图片行应成功");
    let clip_images_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM clip_images", [], |row| row.get(0))
        .expect("查询 clip_images 应成功");

    assert_eq!(image_count, 1, "应有 1 条图片 clip_item");
    assert_eq!(clip_images_count, 1, "clip_images 应有 1 行");
}
