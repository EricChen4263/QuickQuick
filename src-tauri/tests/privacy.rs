//! 集成测试：隐私门控（V1-F1-S03）
//!
//! 验收项：
//! - V1-F1-A06 concealed_skipped   — concealed/transient 平台标记自动跳过，不做内容启发式
//! - V1-F1-A07 app_exclude_list    — 名单内 app 来源的复制不记录

use quickquick_lib::clipboard::ClipboardSnapshot;
use quickquick_lib::privacy::{should_skip, CapturePolicy, ExcludeList, SkipReason};

/// A06-1：is_concealed=true 的快照 → should_skip 返回 Some(Concealed)。
///
/// 注意：仅检测平台标记，不分析内容（AAA / 非恒真）。
#[test]
fn concealed_skipped() {
    // Arrange：构造一个带 concealed 标记的快照（内容无关）
    let snapshot = ClipboardSnapshot {
        text: Some("some normal looking text".to_owned()),
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: true,
        source_app: None,
    };
    let exclude = ExcludeList::new_with_apps([]);
    let policy = CapturePolicy {
        paused: false,
        skip_sensitive: true,
        exclude: &exclude,
    };

    // Act
    let reason = should_skip(&snapshot, &policy);

    // Assert：标记驱动跳过，原因为 Concealed
    assert_eq!(
        reason,
        Some(SkipReason::Concealed),
        "is_concealed=true 时应返回 Concealed，不论内容"
    );
}

/// A06-2：内容看起来像密码但 is_concealed=false → 不跳过（证明不做内容启发式识别）。
#[test]
fn concealed_no_heuristic() {
    // Arrange：内容含密码特征字符串，但平台标记为 false
    let snapshot = ClipboardSnapshot {
        text: Some("P@ssw0rd!Secret123".to_owned()),
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: None,
    };
    let exclude = ExcludeList::new_with_apps([]);
    let policy = CapturePolicy {
        paused: false,
        skip_sensitive: true,
        exclude: &exclude,
    };

    // Act
    let reason = should_skip(&snapshot, &policy);

    // Assert：不看内容，不跳过
    assert_eq!(
        reason, None,
        "is_concealed=false 时即使内容像密码也不应跳过（不做启发式识别）"
    );
}

/// A07-1：source_app 在排除名单内 → should_skip 返回 Some(Excluded)。
#[test]
fn app_exclude_list() {
    // Arrange：构造含目标 app 标识的快照
    let snapshot = ClipboardSnapshot {
        text: Some("secret content".to_owned()),
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: Some("com.foo.secret".to_owned()),
    };
    let exclude = ExcludeList::new_with_apps(["com.foo.secret"]);
    let policy = CapturePolicy {
        paused: false,
        skip_sensitive: true,
        exclude: &exclude,
    };

    // Act
    let reason = should_skip(&snapshot, &policy);

    // Assert：名单命中，返回 Excluded
    assert_eq!(
        reason,
        Some(SkipReason::Excluded),
        "source_app 在排除名单内时应返回 Excluded"
    );
}

/// A07-2：source_app 不在排除名单内 → 不跳过。
#[test]
fn app_not_in_exclude_list() {
    // Arrange：app 不在名单
    let snapshot = ClipboardSnapshot {
        text: Some("normal content".to_owned()),
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: Some("com.apple.textedit".to_owned()),
    };
    let exclude = ExcludeList::new_with_apps(["com.foo.secret"]);
    let policy = CapturePolicy {
        paused: false,
        skip_sensitive: true,
        exclude: &exclude,
    };

    // Act
    let reason = should_skip(&snapshot, &policy);

    // Assert：不在名单，不跳过
    assert_eq!(reason, None, "source_app 不在排除名单内时不应跳过");
}

/// A07-3：source_app 为 None（来源未知）且不在名单 → 不跳过。
#[test]
fn app_exclude_none_source() {
    // Arrange：无来源信息的快照
    let snapshot = ClipboardSnapshot {
        text: Some("content".to_owned()),
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: None,
    };
    let exclude = ExcludeList::new_with_apps(["com.foo.secret"]);
    let policy = CapturePolicy {
        paused: false,
        skip_sensitive: true,
        exclude: &exclude,
    };

    // Act
    let reason = should_skip(&snapshot, &policy);

    // Assert：来源未知时不应因名单而跳过
    assert_eq!(reason, None, "source_app=None 时不应因排除名单跳过");
}
