//! 集成测试：剪贴板捕获引擎（V1-F1-S01）+ 去重/置顶（V1-F1-S02）
//!
//! 验收项：
//! - V1-F1-A01 capture_dual_field   — 双字段同存
//! - V1-F1-A02 poll_changecount_triggers_capture — 轮询 changeCount 驱动
//! - V1-F1-A03 self_write_marker_skipped         — 防自污染跳过
//! - V1-F1-A04 dedup_and_bump       — 内容去重+置顶刷新
//! - V1-F1-A05 bump_no_new_record   — 置顶刷新不产生新记录

use quickquick_lib::clipboard::{CapturedItem, ClipboardBackend, ClipboardSnapshot, poll_once, poll_once_with_policy};
use quickquick_lib::db;
use quickquick_lib::privacy::{CapturePolicy, ExcludeList};
use tempfile::tempdir;

/// 可编程的假剪贴板后端，驱动 poll_once 逻辑测试（无 OS 依赖）。
struct FakeBackend {
    count: u64,
    snapshot: ClipboardSnapshot,
}

impl FakeBackend {
    /// 构造假后端。`is_concealed` 与 `source_app` 填默认安全值（false/None），
    /// 保持 S01/S02 测试行为不变。
    fn new(count: u64, text: Option<&str>, html: Option<&str>, has_self_marker: bool) -> Self {
        Self {
            count,
            snapshot: ClipboardSnapshot {
                text: text.map(str::to_owned),
                html: html.map(str::to_owned),
                has_self_marker,
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

/// A01：快照含 text + html 时，poll_once 返回双字段都在的 CapturedItem；
///      纯文本键（text）作为显示/搜索/判重基础字段。
#[test]
fn capture_dual_field() {
    // Arrange
    let backend = FakeBackend::new(2, Some("hello world"), Some("<b>hello world</b>"), false);
    let mut last_seen = 1u64;

    // Act
    let result = poll_once(&backend, &mut last_seen);

    // Assert
    let item = result.expect("应返回 CapturedItem");
    assert_eq!(item.text, "hello world", "text 字段应保存纯文本键");
    assert_eq!(
        item.html,
        Some("<b>hello world</b>".to_owned()),
        "html 字段应保存富文本"
    );
    assert_eq!(last_seen, 2, "last_seen_count 应更新为新计数");
}

/// A02：changeCount 不变 → None；递增一次 → Some 并更新 last_seen_count；
///      再次调用同值 → None（一递增只捕获一次）。
#[test]
fn poll_changecount_triggers_capture() {
    // Arrange：count 与 last_seen 相同，无变化
    let backend_same = FakeBackend::new(5, Some("text"), None, false);
    let mut last_seen = 5u64;

    // Act + Assert：无变化时返回 None
    let no_change = poll_once(&backend_same, &mut last_seen);
    assert!(no_change.is_none(), "changeCount 未变时应返回 None");
    assert_eq!(last_seen, 5, "无变化时 last_seen_count 不应改变");

    // Arrange：count 递增
    let backend_changed = FakeBackend::new(6, Some("new text"), None, false);

    // Act：递增时应捕获
    let captured = poll_once(&backend_changed, &mut last_seen);
    assert!(captured.is_some(), "changeCount 递增时应返回 Some");
    assert_eq!(last_seen, 6, "捕获后 last_seen_count 应更新为 6");

    // Act：再次调用同 count（模拟下次轮询 count 仍为 6）
    let no_dup = poll_once(&backend_changed, &mut last_seen);
    assert!(no_dup.is_none(), "相同 count 不应重复捕获");
}

/// A03：快照带本工具私有标记时，poll_once 跳过不记（返回 None），
///      但 last_seen_count 仍推进（避免反复触发）。
#[test]
fn self_write_marker_skipped() {
    // Arrange：count 递增但快照带自写标记
    let backend = FakeBackend::new(10, Some("sensitive"), None, true);
    let mut last_seen = 9u64;

    // Act
    let result = poll_once(&backend, &mut last_seen);

    // Assert：跳过不记
    assert!(result.is_none(), "带自写标记时应跳过，返回 None");
    // last_seen_count 仍推进，防止下次轮询（count 不变）再次触发读取
    assert_eq!(last_seen, 10, "即使跳过，last_seen_count 也应更新为新计数");
}

/// I-01 防御测试：OS 计数从大降到小（如 Windows 进程重启致 GetClipboardSequenceNumber
/// 归零），poll_once 应返回 None 且将基线下调为当前值；随后计数正向递增时正常捕获一次，
/// 证明重置后既不重复捕获也不漏捕。
#[test]
fn poll_count_reset_defense() {
    // Arrange：模拟 last_seen=6（旧基线），OS 重置后 count=5（降序）
    let backend_reset = FakeBackend::new(5, Some("after reset"), None, false);
    let mut last_seen = 6u64;

    // Act：降序时应返回 None，且将基线下调为 5
    let result_on_reset = poll_once(&backend_reset, &mut last_seen);

    // Assert：降序不捕获
    assert!(result_on_reset.is_none(), "计数降序（OS 重置）时应返回 None，不误捕");
    assert_eq!(last_seen, 5, "降序时 last_seen_count 应下调为当前值 5");

    // Arrange：紧接着计数正向递增到 6
    let backend_recovered = FakeBackend::new(6, Some("normal capture"), None, false);

    // Act：6 > 5，应正常捕获一次
    let result_after_reset = poll_once(&backend_recovered, &mut last_seen);

    // Assert：正常捕获，证明重置后不漏
    let item = result_after_reset.expect("重置后计数递增应正常捕获");
    assert_eq!(item.text, "normal capture", "重置后恢复的第一次变化应被捕获");
    assert_eq!(last_seen, 6, "捕获后 last_seen_count 应更新为 6");

    // Act：再次调用同 count，证明不重复捕获
    let no_dup = poll_once(&backend_recovered, &mut last_seen);
    assert!(no_dup.is_none(), "相同 count 不应重复捕获");
}

/// A04：ingest 相同文本第二次时，返回 Bumped（不新建行），
///      原条目成为最前（top_id == 原 id），行数仍为 2。
#[test]
fn dedup_and_bump() {
    use quickquick_lib::db::{IngestOutcome, bump_to_top, count_live, ingest, top_id};

    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &[7u8; 32]).expect("建库应成功");

    let item_x = CapturedItem { text: "content-X".to_owned(), html: None };
    let item_y = CapturedItem { text: "content-Y".to_owned(), html: None };

    // Act 1：ingest X → Inserted，库中 1 条
    let outcome_x1 = ingest(&conn, &item_x).expect("ingest X 应成功");
    let id_x = match outcome_x1 {
        IngestOutcome::Inserted(id) => id,
        IngestOutcome::Bumped(_) => panic!("首次 ingest X 应为 Inserted"),
    };
    assert_eq!(count_live(&conn).expect("count_live"), 1, "ingest X 后应有 1 条");

    // Act 2：ingest Y → Inserted，库中 2 条；持有 id_y 用于后续显式置顶（避免 top_id 时序依赖）
    let id_y = match ingest(&conn, &item_y).expect("ingest Y 应成功") {
        IngestOutcome::Inserted(id) => id,
        IngestOutcome::Bumped(_) => panic!("首次 ingest Y 应为 Inserted"),
    };
    assert_eq!(count_live(&conn).expect("count_live"), 2, "ingest Y 后应有 2 条");

    // 将 Y 显式置顶，确保 Y 的 last_modified_utc 晚于 X（与插入顺序无关）
    bump_to_top(&conn, &id_y).expect("bump_to_top Y 应成功");

    // Act 3：再次 ingest X（相同文本）→ 应返回 Bumped，行数仍 2
    let outcome_x2 = ingest(&conn, &item_x).expect("再次 ingest X 应成功");
    let bumped_id = match outcome_x2 {
        IngestOutcome::Bumped(id) => id,
        IngestOutcome::Inserted(_) => panic!("重复 ingest X 应为 Bumped，不应新建行"),
    };

    // Assert：Bumped 返回的 id 就是原来 X 的 id
    assert_eq!(bumped_id, id_x, "Bumped id 应与原 X 的 id 一致");

    // Assert：行数仍为 2（无新行）
    assert_eq!(count_live(&conn).expect("count_live"), 2, "去重后行数应仍为 2");

    // Assert：X 现在是最前（last_modified_utc 最新）
    let top = top_id(&conn).expect("top_id").expect("应有最前条目");
    assert_eq!(top, id_x, "置顶刷新后 X 应成为最前条目");
}

/// A05：显式调 bump_to_top 不产生新记录，X 移到最前，行数仍为 2。
#[test]
fn bump_no_new_record() {
    use quickquick_lib::db::{IngestOutcome, bump_to_top, count_live, ingest, top_id};

    // Arrange：插入 X、Y，共 2 条，Y 更新（置顶 Y）
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &[7u8; 32]).expect("建库应成功");

    let item_x = CapturedItem { text: "bump-X".to_owned(), html: None };
    let item_y = CapturedItem { text: "bump-Y".to_owned(), html: None };

    let id_x = match ingest(&conn, &item_x).expect("ingest X") {
        IngestOutcome::Inserted(id) => id,
        IngestOutcome::Bumped(_) => panic!("首次 ingest X 应为 Inserted"),
    };
    match ingest(&conn, &item_y).expect("ingest Y") {
        IngestOutcome::Inserted(_) => {}
        IngestOutcome::Bumped(_) => panic!("首次 ingest Y 应为 Inserted"),
    }
    assert_eq!(count_live(&conn).expect("count_live 初始"), 2, "初始应有 2 条");

    // Act：显式 bump X 到最前
    bump_to_top(&conn, &id_x).expect("bump_to_top X 应成功");

    // Assert：行数仍为 2（没有新记录产生）
    assert_eq!(count_live(&conn).expect("count_live 后"), 2, "bump 后行数应仍为 2，无新记录");

    // Assert：X 现在是最前
    let top = top_id(&conn).expect("top_id").expect("应有最前条目");
    assert_eq!(top, id_x, "bump_to_top 后 X 应成为最前条目");
}

/// A08-1：paused=true 时，poll_once_with_policy 对正常可捕获快照返回 None，
///        但 last_seen_count 仍推进（防止下次重复触发读取）。
#[test]
fn pause_stops_capture() {
    // Arrange：正常快照，count 递增，策略为暂停
    let backend = FakeBackend::new(3, Some("normal text"), None, false);
    let mut last_seen = 2u64;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy { paused: true, exclude: &exclude };

    // Act
    let result = poll_once_with_policy(&backend, &mut last_seen, &policy);

    // Assert：暂停时应跳过
    assert!(result.is_none(), "paused=true 时应返回 None，不捕获");
    assert_eq!(last_seen, 3, "即使跳过，last_seen_count 也应推进到 3");
}

/// A08-2：paused=false 时，同一快照正常捕获，证明开关有效。
#[test]
fn pause_false_captures_normally() {
    // Arrange：相同快照，策略为未暂停
    let backend = FakeBackend::new(3, Some("normal text"), None, false);
    let mut last_seen = 2u64;
    let exclude = ExcludeList::default();
    let policy = CapturePolicy { paused: false, exclude: &exclude };

    // Act
    let result = poll_once_with_policy(&backend, &mut last_seen, &policy);

    // Assert：未暂停时应正常捕获
    let item = result.expect("paused=false 时应返回 CapturedItem");
    assert_eq!(item.text, "normal text", "捕获内容应与快照一致");
    assert_eq!(last_seen, 3, "last_seen_count 应更新为 3");
}
