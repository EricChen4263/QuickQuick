//! 集成测试：回写粘贴引擎（V1-F3-S06）
//!
//! 验收项：
//! - V1-F3-A15 paste_waits_changecount — 回写时序：changeCount 反映写入后再发粘贴
//! - V1-F3-A15b write_and_confirm_ok   — write_and_confirm 正常递增→Ok，未调 send_paste
//! - V1-F3-A15c write_and_confirm_timeout — write_and_confirm 冻结→Err(Timeout)
//! - V1-F3-A17 focus_restore_path     — 焦点恢复路径顺序契约
//! - V1-F3-A18 paste_leaves_selected  — 粘贴后剪贴板留下被选条目 X

use quickquick_lib::clipboard::CapturedItem;
use quickquick_lib::paste::{
    focus_restore_sequence, write_and_confirm, write_then_paste, FocusStep, PasteBackend,
    PasteError,
};

/// 可控的假粘贴后端，记录 send_paste 发送时机。
struct FakePasteBackend {
    /// 当前 change_count，write_with_marker 后自动递增
    count: u64,
    /// 当前剪贴板文本（write_with_marker 写入）
    clipboard_text: Option<String>,
    /// send_paste 被调用时记录的 count 快照
    paste_sent_at_count: Option<u64>,
    /// 是否禁止 change_count 递增（用于模拟超时场景）
    freeze_count: bool,
}

impl FakePasteBackend {
    fn new(initial_count: u64) -> Self {
        Self {
            count: initial_count,
            clipboard_text: None,
            paste_sent_at_count: None,
            freeze_count: false,
        }
    }

    fn with_frozen_count(initial_count: u64) -> Self {
        Self {
            count: initial_count,
            clipboard_text: None,
            paste_sent_at_count: None,
            freeze_count: true,
        }
    }
}

impl PasteBackend for FakePasteBackend {
    fn change_count(&self) -> u64 {
        self.count
    }

    fn write_with_marker(&mut self, item: &CapturedItem) {
        self.clipboard_text = Some(item.text.clone());
        if !self.freeze_count {
            self.count += 1;
        }
    }

    fn current_text(&self) -> Option<String> {
        self.clipboard_text.clone()
    }

    fn send_paste(&mut self) {
        self.paste_sent_at_count = Some(self.count);
    }
}

/// A15（paste_waits_changecount）：
/// 正常路径——write_with_marker 后 change_count 递增，
/// send_paste 应在 change_count 反映写入后才发（paste_sent_at_count > 写前 count）。
#[test]
fn paste_timing_paste_waits_changecount() {
    // Arrange
    let mut backend = FakePasteBackend::new(5);
    let count_before_write = backend.change_count();
    let item = CapturedItem {
        text: "hello paste".to_owned(),
        html: None,
    };

    // Act
    let result = write_then_paste(&mut backend, &item);

    // Assert：写入成功，无超时错误
    assert!(
        result.is_ok(),
        "write_then_paste 应成功，错误: {:?}",
        result
    );

    // Assert：send_paste 在 change_count 递增（反映我方写入）之后才发
    let sent_at = backend.paste_sent_at_count.expect("send_paste 应已被调用");
    assert!(
        sent_at > count_before_write,
        "send_paste 应在 change_count 反映写入后才发（sent_at={sent_at} 应 > 写前={count_before_write}）"
    );
}

/// A15 超时路径——change_count 永不递增时，write_then_paste 应返回 Timeout 错误，
/// 不盲发 send_paste（避免粘贴到旧内容）。
#[test]
fn paste_timing_timeout_when_count_never_increases() {
    // Arrange：freeze_count=true，write_with_marker 不递增计数
    let mut backend = FakePasteBackend::with_frozen_count(10);
    let item = CapturedItem {
        text: "should timeout".to_owned(),
        html: None,
    };

    // Act
    let result = write_then_paste(&mut backend, &item);

    // Assert：应返回 Timeout 错误
    assert!(
        matches!(result, Err(PasteError::Timeout)),
        "change_count 永不递增时应返回 Timeout，实际: {:?}",
        result
    );

    // Assert：超时路径不盲发 send_paste
    assert!(
        backend.paste_sent_at_count.is_none(),
        "超时时不应调用 send_paste"
    );
}

/// A17（focus_restore_path）：
/// focus_restore_sequence 返回的步骤顺序必须精确为设计§八#2 规定的顺序。
#[test]
fn focus_restore_path_sequence_matches_spec() {
    // Arrange & Act
    let steps = focus_restore_sequence();

    // Assert：恰好 5 步，顺序与设计文档完全一致
    assert_eq!(
        steps.len(),
        5,
        "焦点恢复序列应恰好 5 步，实际: {}",
        steps.len()
    );
    assert_eq!(
        steps[0],
        FocusStep::RecordFrontmost,
        "步骤 0 应为 RecordFrontmost"
    );
    assert_eq!(steps[1], FocusStep::HidePanel, "步骤 1 应为 HidePanel");
    assert_eq!(
        steps[2],
        FocusStep::ActivateOriginalApp,
        "步骤 2 应为 ActivateOriginalApp"
    );
    assert_eq!(
        steps[3],
        FocusStep::WaitForeground,
        "步骤 3 应为 WaitForeground"
    );
    assert_eq!(
        steps[4],
        FocusStep::SimulatePaste,
        "步骤 4 应为 SimulatePaste"
    );
}

/// A18（paste_leaves_selected）：
/// write_then_paste(item X) 后，backend.current_text() 应等于 X.text，
/// 剪贴板留下被选条目 X，不做"借用后恢复原剪贴板"。
#[test]
fn paste_leaves_selected_item_on_clipboard() {
    // Arrange
    let mut backend = FakePasteBackend::new(3);
    let item_x = CapturedItem {
        text: "selected-item-X".to_owned(),
        html: None,
    };

    // Act
    let result = write_then_paste(&mut backend, &item_x);

    // Assert：操作成功
    assert!(
        result.is_ok(),
        "write_then_paste 应成功，错误: {:?}",
        result
    );

    // Assert：剪贴板留下 X（不恢复到旧内容）
    assert_eq!(
        backend.current_text(),
        Some("selected-item-X".to_owned()),
        "粘贴后剪贴板应留下被选条目 X 的文本"
    );
}

/// A15b（write_and_confirm_ok）：
/// 正常路径——write_and_confirm 写入并等到 changeCount 递增后返回 Ok，
/// 且不调用 send_paste（职责边界：只确认写入，不发粘贴）。
#[test]
fn write_and_confirm_normal_returns_ok_without_send_paste() {
    let mut backend = FakePasteBackend::new(0);
    let item = CapturedItem {
        text: "confirm-test".to_owned(),
        html: None,
    };

    let result = write_and_confirm(&mut backend, &item);

    assert!(result.is_ok(), "正常递增时 write_and_confirm 应返回 Ok，实际: {result:?}");
    assert!(
        backend.paste_sent_at_count.is_none(),
        "write_and_confirm 不应调用 send_paste（只负责写入确认）"
    );
    assert_eq!(
        backend.current_text(),
        Some("confirm-test".to_owned()),
        "write_and_confirm 后剪贴板文本应为写入内容"
    );
}

/// A15c（write_and_confirm_timeout）：
/// 冻结计数路径——changeCount 永不递增时 write_and_confirm 应返回 Err(Timeout)，
/// 且不调用 send_paste。
#[test]
fn write_and_confirm_frozen_count_returns_timeout() {
    let mut backend = FakePasteBackend::with_frozen_count(7);
    let item = CapturedItem {
        text: "should-timeout".to_owned(),
        html: None,
    };

    let result = write_and_confirm(&mut backend, &item);

    assert!(
        matches!(result, Err(PasteError::Timeout)),
        "计数冻结时 write_and_confirm 应返回 Timeout，实际: {result:?}"
    );
    assert!(
        backend.paste_sent_at_count.is_none(),
        "超时时不应调用 send_paste"
    );
}
