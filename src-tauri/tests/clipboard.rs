//! 集成测试：剪贴板捕获引擎（V1-F1-S01）
//!
//! 验收项：
//! - V1-F1-A01 capture_dual_field   — 双字段同存
//! - V1-F1-A02 poll_changecount_triggers_capture — 轮询 changeCount 驱动
//! - V1-F1-A03 self_write_marker_skipped         — 防自污染跳过

use quickquick_lib::clipboard::{ClipboardBackend, ClipboardSnapshot, poll_once};

// ── FakeBackend ───────────────────────────────────────────────────────────────

/// 可编程的假剪贴板后端，驱动 poll_once 逻辑测试（无 OS 依赖）。
struct FakeBackend {
    count: u64,
    snapshot: ClipboardSnapshot,
}

impl FakeBackend {
    fn new(count: u64, text: Option<&str>, html: Option<&str>, has_self_marker: bool) -> Self {
        Self {
            count,
            snapshot: ClipboardSnapshot {
                text: text.map(str::to_owned),
                html: html.map(str::to_owned),
                has_self_marker,
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

// ── V1-F1-A01 capture_dual_field ─────────────────────────────────────────────

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

// ── V1-F1-A02 poll_changecount_triggers_capture ───────────────────────────────

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

// ── V1-F1-A03 self_write_marker_skipped ──────────────────────────────────────

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

// ── I-01 防御：OS 计数重置（降序）─────────────────────────────────────────────

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
