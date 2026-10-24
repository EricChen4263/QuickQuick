//! 集成测试：macOS Accessibility 引导与优雅降级（V3-F3-S09）
//!
//! 验收项：
//! - V3-F3-A11 accessibility_onboarding_degrade
//!   — 未授权时弹说明卡片+深链，未授权优雅降级为 WriteBackOnly（不发粘贴，不崩）

use quickquick_lib::clipboard::CapturedItem;
use quickquick_lib::onboarding::{
    advance, finish, observe_trust, onboarding_action, onboarding_status, paste_capability,
    perform_paste_or_degrade, skip_step, AccessibilityProbe, OnboardingAction, OnboardingState,
    OnboardingStep, PasteCapability, ACCESSIBILITY_DEEPLINK,
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

    fn write_image(&mut self, _width: usize, _height: usize, _rgba: &[u8]) -> Result<(), String> {
        // onboarding 测试不涉及图片写入，返回 Ok 满足 trait
        Ok(())
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
    assert!(
        result.is_ok(),
        "降级路径不应 panic 或返回错误，实际: {:?}",
        result
    );

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

// ---------------------------------------------------------------------------
// 首次引导向导状态机（P5-4）
// ---------------------------------------------------------------------------

/// 首次进入向导停在欢迎页，并记录当前授权观测值。
#[test]
fn onboarding_guide_starts_at_welcome_with_trust_observation() {
    // Arrange & Act
    let untrusted_state = OnboardingState::new(false);
    let trusted_state = OnboardingState::new(true);

    // Assert：未授权与已授权都从欢迎页开始（授权步骤在前进时才可能被跳过）
    assert_eq!(untrusted_state.step(), OnboardingStep::Welcome);
    assert_eq!(trusted_state.step(), OnboardingStep::Welcome);
    assert!(!untrusted_state.accessibility_trusted());
    assert!(trusted_state.accessibility_trusted());
    assert!(untrusted_state.is_visible(), "欢迎页必须渲染");
}

/// 未授权时四步顺序推进：欢迎 → 热键 → 授权 → 自启 → 完成，终态不再出现。
#[test]
fn onboarding_guide_advance_walks_four_steps_in_order_when_untrusted() {
    // Arrange
    let mut state = OnboardingState::new(false);

    // Act & Assert：逐步前进，每一步都钉住具体步骤码
    state = advance(state);
    assert_eq!(state.step(), OnboardingStep::HotkeyIntro);
    state = advance(state);
    assert_eq!(state.step(), OnboardingStep::AccessibilityAuth);
    state = advance(state);
    assert_eq!(state.step(), OnboardingStep::AutostartConfirm);

    // Act：最后一步前进 → 终态
    state = advance(state);

    // Assert：完成后引导不再渲染
    assert_eq!(state.step(), OnboardingStep::Done);
    assert!(!state.is_visible(), "走到终态后引导不应再渲染");
}

/// 已授权时前进跳过授权步骤（授权过的用户不该再看一遍授权请求）。
#[test]
fn onboarding_guide_advance_skips_accessibility_step_when_trusted() {
    // Arrange
    let mut state = OnboardingState::new(true);

    // Act：欢迎 → 热键
    state = advance(state);
    assert_eq!(state.step(), OnboardingStep::HotkeyIntro);

    // Act：热键 →（跳过授权）→ 自启确认
    state = advance(state);

    // Assert：直接落在自启确认，授权步骤被跳过
    assert_eq!(
        state.step(),
        OnboardingStep::AutostartConfirm,
        "已授权时前进必须跳过辅助功能授权步骤"
    );
    assert!(state.accessibility_trusted());
}

/// 授权后自动收起：停在授权步骤时观测到授权变为真，该步骤立即收起并前进到自启确认。
///
/// 这条钉住「授权完成后引导自动恢复」的核心分支——删掉 observe_trust 里的自动前进，
/// 本测试必须变红。
#[test]
fn onboarding_guide_observe_trust_collapses_accessibility_step() {
    // Arrange：未授权，走到授权步骤
    let mut state = OnboardingState::new(false);
    state = advance(state);
    state = advance(state);
    assert_eq!(state.step(), OnboardingStep::AccessibilityAuth);

    // Act：授权状态从未授权变为已授权
    let collapsed = observe_trust(state, true);

    // Assert：授权步骤已收起，直接到达自启确认；授权观测值同步更新
    assert_eq!(
        collapsed.step(),
        OnboardingStep::AutostartConfirm,
        "授权变为真后，授权步骤必须自动收起并前进"
    );
    assert!(collapsed.accessibility_trusted(), "授权观测值必须同步更新");
    assert!(collapsed.is_visible(), "自动收起的是授权步骤，不是整个向导");
}

/// 授权观测的负向对照：仍是未授权时不得前进（否则「授权后收起」无从与前进一步区分）。
#[test]
fn onboarding_guide_observe_untrusted_keeps_accessibility_step() {
    // Arrange
    let mut state = OnboardingState::new(false);
    state = advance(state);
    state = advance(state);

    // Act：重复观测到未授权
    let observed = observe_trust(state, false);

    // Assert：仍停在授权步骤
    assert_eq!(observed.step(), OnboardingStep::AccessibilityAuth);
    assert!(!observed.accessibility_trusted());
}

/// 授权观测发生在更早步骤时，后续前进同样跳过授权步骤（不是只有「当场收起」一种路径）。
#[test]
fn onboarding_guide_trust_observed_on_welcome_is_honored_by_later_advance() {
    // Arrange：停在欢迎页，此时观测到授权为真
    let state = observe_trust(OnboardingState::new(false), true);
    assert_eq!(state.step(), OnboardingStep::Welcome);

    // Act：欢迎 → 热键 → 下一步
    let at_hotkey = advance(state);
    assert_eq!(at_hotkey.step(), OnboardingStep::HotkeyIntro);
    let next = advance(at_hotkey);

    // Assert：跳过授权步骤
    assert_eq!(next.step(), OnboardingStep::AutostartConfirm);
}

/// 跳过：欢迎页不可跳过（不能一进来就把向导跳没），中间步骤与前进同路径。
#[test]
fn onboarding_guide_skip_step_guards_welcome_and_walks_the_rest() {
    // Arrange：欢迎页
    let at_welcome = OnboardingState::new(false);

    // Act & Assert：欢迎页跳过是空操作
    assert_eq!(
        skip_step(at_welcome),
        at_welcome,
        "欢迎页不应被跳过（返回原状态）"
    );

    // Act：热键页跳过 → 授权页；授权页跳过 → 自启确认
    let at_hotkey = advance(at_welcome);
    assert_eq!(
        skip_step(at_hotkey).step(),
        OnboardingStep::AccessibilityAuth
    );
    let at_accessibility = skip_step(at_hotkey);
    assert_eq!(
        skip_step(at_accessibility).step(),
        OnboardingStep::AutostartConfirm,
        "授权步骤「稍后」应前进到自启确认"
    );
}

/// 结束（完成 / 关闭）后引导不再出现，且终态是吸收态。
#[test]
fn onboarding_guide_finish_is_terminal_and_invisible() {
    // Arrange
    let mut state = OnboardingState::new(false);
    state = advance(state);

    // Act
    let finished = finish(state);

    // Assert：终态不再渲染，继续前进也不会复活
    assert_eq!(finished.step(), OnboardingStep::Done);
    assert!(!finished.is_visible(), "结束后引导不应再渲染");
    assert_eq!(
        advance(finished),
        finished,
        "终态下继续前进必须保持终态（此后不再出现）"
    );
    assert_eq!(skip_step(finished), finished, "终态跳过同样是空操作");
}

/// 步骤码是跨语言契约：前端 `GuideStep` 用的就是这几个字面量。
#[test]
fn onboarding_guide_step_codes_are_frontend_contract() {
    assert_eq!(OnboardingStep::Welcome.code(), "welcome");
    assert_eq!(OnboardingStep::HotkeyIntro.code(), "hotkey");
    assert_eq!(OnboardingStep::AccessibilityAuth.code(), "accessibility");
    assert_eq!(OnboardingStep::AutostartConfirm.code(), "autostart");
    assert_eq!(OnboardingStep::Done.code(), "done");
}

/// 引导状态 DTO 的字段取值是前端契约，字符串必须逐字对齐 ipc-client.ts。
#[test]
fn onboarding_status_dto_codes_are_frontend_contract() {
    // Arrange：两种探针（已授权 / 未授权）
    let trusted_probe = FakeAccessibilityProbe::trusted();
    let untrusted_probe = FakeAccessibilityProbe::untrusted();

    // Act
    let trusted_status = onboarding_status(&trusted_probe);
    let untrusted_status = onboarding_status(&untrusted_probe);

    // Assert：已授权 → 直接进行 + 完整粘贴
    assert_eq!(trusted_status.action, "proceed");
    assert_eq!(trusted_status.capability, "full_paste");

    // Assert：未授权 → 展示授权卡片 + 仅写回
    assert_eq!(untrusted_status.action, "show_accessibility_card");
    assert_eq!(untrusted_status.capability, "write_back_only");
}
