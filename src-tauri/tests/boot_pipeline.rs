//! 集成测试：启动数据管道（V4-F1-S04）
//!
//! 验收项：
//! - boot_pipeline_open_db          — open_app_db 用 fake key provider 成功建库并可 list
//! - boot_pipeline_ingest_visible   — capture_and_ingest 写入后 list_items_full 可见该条
//! - boot_pipeline_dedup_bumped     — 重复同内容时返回 Bumped，不新增行
//! - boot_pipeline_no_change_none   — change_count 未变时返回 Ok(None)，库不增

use quickquick_lib::db;
use quickquick_lib::keyprovider::{KeyError, KeyProvider};
use quickquick_lib::clipboard::{ClipboardBackend, ClipboardSnapshot};
use quickquick_lib::privacy::{CapturePolicy, ExcludeList};
use quickquick_lib::pipeline;
use quickquick_lib::db::IngestOutcome;
use tempfile::tempdir;
use std::sync::{Arc, Mutex};

/// 固定密钥的 fake KeyProvider，不触碰真实钥匙串。
struct FixedKeyProvider {
    key: [u8; 32],
}

impl FixedKeyProvider {
    fn new(key: [u8; 32]) -> Self {
        Self { key }
    }
}

impl KeyProvider for FixedKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], KeyError> {
        Ok(self.key)
    }
}

/// 可控的假剪贴板后端，change_count 和内容可在测试中变更。
struct FakeClipboardBackend {
    count: Arc<Mutex<u64>>,
    text: Arc<Mutex<Option<String>>>,
}

impl FakeClipboardBackend {
    fn new(count: u64, text: Option<&str>) -> Self {
        Self {
            count: Arc::new(Mutex::new(count)),
            text: Arc::new(Mutex::new(text.map(str::to_owned))),
        }
    }

    fn set_count(&self, c: u64) {
        *self.count.lock().unwrap() = c;
    }

    fn set_text(&self, t: Option<&str>) {
        *self.text.lock().unwrap() = t.map(str::to_owned);
    }
}

impl ClipboardBackend for FakeClipboardBackend {
    fn change_count(&self) -> u64 {
        *self.count.lock().unwrap()
    }

    fn read(&self) -> ClipboardSnapshot {
        ClipboardSnapshot {
            text: self.text.lock().unwrap().clone(),
            html: None,
            has_self_marker: false,
            is_concealed: false,
            source_app: None,
        }
    }
}

/// boot_pipeline_open_db：open_app_db 用固定密钥成功建库，list_items_full 返回空列表。
#[test]
fn boot_pipeline_open_db() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let provider = FixedKeyProvider::new([11u8; 32]);

    // Act
    let conn = pipeline::open_app_db(&provider, &db_path)
        .expect("open_app_db 应成功");

    // Assert：可调 list_items_full，返回空列表
    let items = db::list_items_full(&conn).expect("list_items_full 应成功");
    assert!(items.is_empty(), "新建库应无任何条目");
}

/// boot_pipeline_ingest_visible：capture_and_ingest 写入后 list_items_full 可见该条目。
#[test]
fn boot_pipeline_ingest_visible() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let provider = FixedKeyProvider::new([22u8; 32]);
    let conn = pipeline::open_app_db(&provider, &db_path).expect("open_app_db 应成功");

    let backend = FakeClipboardBackend::new(1, Some("hello pipeline"));
    let mut last_seen: u64 = 0;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy { paused: false, exclude: &exclude };

    // Act
    let outcome = pipeline::capture_and_ingest(&backend, &mut last_seen, &conn, &policy)
        .expect("capture_and_ingest 应成功");

    // Assert：返回 Some(Inserted)
    assert!(
        matches!(outcome, Some(IngestOutcome::Inserted(_))),
        "首次捕获应返回 Inserted，实际: {:?}",
        outcome
    );

    // Assert：list_items_full 可见该条内容
    let items = db::list_items_full(&conn).expect("list_items_full 应成功");
    assert_eq!(items.len(), 1, "应有 1 条记录");
    assert_eq!(items[0].content, "hello pipeline", "内容应与捕获文本一致");
}

/// boot_pipeline_dedup_bumped：重复相同内容时 capture_and_ingest 返回 Bumped，不新增行。
#[test]
fn boot_pipeline_dedup_bumped() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let provider = FixedKeyProvider::new([33u8; 32]);
    let conn = pipeline::open_app_db(&provider, &db_path).expect("open_app_db 应成功");

    let backend = FakeClipboardBackend::new(1, Some("dup content"));
    let mut last_seen: u64 = 0;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy { paused: false, exclude: &exclude };

    // Act 1：首次捕获 → Inserted
    let first = pipeline::capture_and_ingest(&backend, &mut last_seen, &conn, &policy)
        .expect("首次 capture_and_ingest 应成功");
    assert!(matches!(first, Some(IngestOutcome::Inserted(_))), "首次应为 Inserted");

    // Act 2：change_count 递增但内容不变 → 再次捕获同内容应返回 Bumped
    backend.set_count(2);
    let second = pipeline::capture_and_ingest(&backend, &mut last_seen, &conn, &policy)
        .expect("第二次 capture_and_ingest 应成功");

    // Assert：返回 Bumped，行数仍为 1
    assert!(
        matches!(second, Some(IngestOutcome::Bumped(_))),
        "重复内容应返回 Bumped，实际: {:?}",
        second
    );
    let items = db::list_items_full(&conn).expect("list_items_full 应成功");
    assert_eq!(items.len(), 1, "去重后应仍为 1 条，不新增行");
}

/// boot_pipeline_no_change_none：change_count 未变时返回 Ok(None)，库条目数不增。
#[test]
fn boot_pipeline_no_change_none() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let provider = FixedKeyProvider::new([44u8; 32]);
    let conn = pipeline::open_app_db(&provider, &db_path).expect("open_app_db 应成功");

    // 先插入一条，确保库不为空作为基准
    let backend = FakeClipboardBackend::new(1, Some("initial content"));
    let mut last_seen: u64 = 0;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy { paused: false, exclude: &exclude };

    pipeline::capture_and_ingest(&backend, &mut last_seen, &conn, &policy)
        .expect("初始 ingest 应成功");

    // 此时 last_seen == 1，backend.count 仍为 1 → 无变化
    // Act：再次轮询，count 未变
    let no_change = pipeline::capture_and_ingest(&backend, &mut last_seen, &conn, &policy)
        .expect("无变化时 capture_and_ingest 应返回 Ok(None)");

    // Assert：返回 None，库中仍只有 1 条
    assert!(no_change.is_none(), "change_count 未变时应返回 Ok(None)");
    let items = db::list_items_full(&conn).expect("list_items_full 应成功");
    assert_eq!(items.len(), 1, "无新捕获时库条目数不应增加");
}
