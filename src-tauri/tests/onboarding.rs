//! 集成测试：macOS Accessibility 引导与优雅降级（V3-F3-S09）
//!
//! 验收项：
//! - V3-F3-A11 accessibility_onboarding_degrade
//!   — 未授权时弹说明卡片+深链，未授权优雅降级为 WriteBackOnly（不发粘贴，不崩）

use quickquick_lib::clipboard::CapturedItem;
use quickquick_lib::onboarding::{
    AccessibilityProbe, OnboardingAction, PasteCapability, ACCESSIBILITY_DEEPLINK,
    onboarding_action, paste_capability, perform_paste_or_degrade,
};
use quickquick_lib::paste::PasteBackend;

/// 可控的假 Accessibility 探针，用于 headless 测试。
struct FakeAccessibilityProbe {
    trusted: bool,
}

impl FakeAccessibilityProbe {
    fn trusted() -> Self {
        Self { trusted: true }
    }

    fn untrusted() -> Self {
        Self { trusted: false }
    }
}

impl AccessibilityProbe for FakeAccessibilityProbe {
    fn is_trusted(&self) -> bool {
        self.trusted
    }
}

/// 可控的假粘贴后端，记录 write_with_marker 和 send_paste 调用次数。
struct FakePasteBackend {
    count: u64,
    clipboard_text: Option<String>,
    write_call_count: usize,
    send_paste_call_count: usize,
}

impl FakePasteBackend {
    fn new() -> Self {
        Self {
            count: 0,
            clipboard_text: None,
            write_call_count: 0,
            send_paste_call_count: 0,
        }
    }
}

impl PasteBackend for FakePasteBackend {
    fn change_count(&self) -> u64 {
        self.count
    }

    fn write_with_marker(&mut self, item: &CapturedItem) {
        self.clipboard_text = Some(item.text.clone());
        self.count += 1;
        self.write_call_count += 1;
    }

    fn current_text(&self) -> Option<String> {
        self.clipboard_text.clone()
    }

    fn send_paste(&mut self) {
        self.send_paste_call_count += 1;
    }
}

/// A11（accessibility_onboarding_degrade）未授权分支——
/// 未授权探针应返回 ShowCardAndDeepLink，深链常量指向辅助功能设置。
#[test]
fn accessibility_onboarding_degrade_untrusted_shows_card_and_deeplink() {
    // Arrange
    let probe = FakeAccessibilityProbe::untrusted();

    // Act
    let action = onboarding_action(&probe);

    // Assert：未授权应触发引导卡片+深链
    assert_eq!(
        action,
        OnboardingAction::ShowCardAndDeepLink,
        "未授权时 onboarding_action 应返回 ShowCardAndDeepLink"
    );

    // Assert：深链常量精确等于设计规定值
    assert_eq!(
        ACCESSIBILITY_DEEPLINK,
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        "深链常量应精确等于设计规定值"
    );
}

/// A11（accessibility_onboarding_degrade）未授权降级——
/// 未授权时 paste_capability 返回 WriteBackOnly，
/// perform_paste_or_degrade 仅写回剪贴板，不调用 send_paste，不 panic。
#[test]
fn accessibility_onboarding_degrade_untrusted_write_back_only_no_paste() {
    // Arrange
    let probe = FakeAccessibilityProbe::untrusted();
    let mut backend = FakePasteBackend::new();
    let item = CapturedItem {
        text: "降级写回测试内容".to_owned(),
        html: None,
    };

    // Act：验证能力检测为 WriteBackOnly
    let capability = paste_capability(&probe);
    assert_eq!(
        capability,
        PasteCapability::WriteBackOnly,
        "未授权时 paste_capability 应返回 WriteBackOnly"
    );

    // Act：perform_paste_or_degrade 在未授权时仅写回
    let result = perform_paste_or_degrade(&probe, &mut backend, &item);

    // Assert：函数返回降级标识（Ok 且未发送粘贴）
    assert!(result.is_ok(), "降级路径不应 panic 或返回错误，实际: {:?}", result);

    // Assert：剪贴板内容已写入（write_with_marker 被调用）
    assert_eq!(
        backend.clipboard_text,
        Some("降级写回测试内容".to_owned()),
        "降级路径应写回剪贴板内容"
    );
    assert_eq!(
        backend.write_call_count, 1,
        "降级路径应恰好调用一次 write_with_marker"
    );

    // Assert：send_paste 未被调用（关键：不模拟 Cmd+V）
    assert_eq!(
        backend.send_paste_call_count, 0,
        "未授权降级路径不应调用 send_paste（不模拟 Cmd+V）"
    );
}

/// A11（accessibility_onboarding_degrade）已授权分支——
/// 已授权探针应返回 Proceed，paste_capability 为 FullPaste，
/// perform_paste_or_degrade 写回剪贴板且返回成功。
#[test]
fn accessibility_onboarding_degrade_trusted_full_paste() {
    // Arrange
    let probe = FakeAccessibilityProbe::trusted();
    let mut backend = FakePasteBackend::new();
    let item = CapturedItem {
        text: "完整粘贴测试内容".to_owned(),
        html: None,
    };

    // Act：引导决策
    let action = onboarding_action(&probe);

    // Assert：已授权应直接进行
    assert_eq!(
        action,
        OnboardingAction::Proceed,
        "已授权时 onboarding_action 应返回 Proceed"
    );

    // Act：能力检测
    let capability = paste_capability(&probe);

    // Assert：能力为完整粘贴
    assert_eq!(
        capability,
        PasteCapability::FullPaste,
        "已授权时 paste_capability 应返回 FullPaste"
    );

    // Act：执行完整粘贴
    let result = perform_paste_or_degrade(&probe, &mut backend, &item);

    // Assert：完整路径成功且已写回剪贴板
    assert!(result.is_ok(), "完整粘贴路径应成功，实际: {:?}", result);
    assert_eq!(
        backend.write_call_count, 1,
        "完整粘贴应调用 write_with_marker"
    );
}

/// A11（accessibility_onboarding_degrade）已授权 send_paste 路径——
/// perform_paste_or_degrade 在已授权时应调用 send_paste（模拟 Cmd+V）。
/// 独立测试确保此关键断言不被前置断言失败屏蔽。
#[test]
fn accessibility_onboarding_degrade_trusted_perform_calls_send_paste() {
    // Arrange
    let probe = FakeAccessibilityProbe::trusted();
    let mut backend = FakePasteBackend::new();
    let item = CapturedItem {
        text: "send_paste 路径验证内容".to_owned(),
        html: None,
    };

    // Act
    let result = perform_paste_or_degrade(&probe, &mut backend, &item);

    // Assert：send_paste 被调用（完整路径模拟 Cmd+V）
    assert!(result.is_ok(), "完整粘贴路径应成功，实际: {:?}", result);
    assert_eq!(
        backend.send_paste_call_count, 1,
        "已授权完整粘贴应调用 send_paste"
    );
}
