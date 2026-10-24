//! macOS Accessibility 引导与优雅降级（V3-F3-S09）
//!
//! 设计对齐：设计文档§二 Accessibility 主动引导+优雅降级
//!
//! 背景说明：
//! - 全局热键（tauri-plugin-global-shortcut）**不需要** Accessibility 授权——
//!   macOS 允许任何应用注册全局快捷键。
//! - 模拟粘贴（CGEvent Cmd+V）和读取选中内容**需要** Accessibility 授权——
//!   否则 CGEvent 注入会被 OS 静默丢弃。
//! - 因此：未授权时应用仍可响应热键打开面板，但粘贴须降级为仅写回剪贴板。
//!
//! 核心抽象：
//! - `AccessibilityProbe` trait — 抽象 AXIsProcessTrusted 检测，headless 可测
//! - `OnboardingAction`        — 引导决策枚举：直接进行 vs 显示说明卡片+深链
//! - `PasteCapability`         — 粘贴能力枚举：完整粘贴 vs 仅写回剪贴板
//! - `onboarding_action`       — 纯函数：依据探针结果决定引导动作
//! - `paste_capability`        — 纯函数：依据探针结果返回当前粘贴能力
//! - `perform_paste_or_degrade`— 执行粘贴：已授权走完整路径，未授权仅写回不崩
//! - `OnboardingState`         — 首次引导向导的状态机（欢迎→热键→授权→自启→完成）
//! - `get_onboarding_status`   — 命令：把上面两个纯判定暴露给前端，供向导决定落到哪一步

use serde::Serialize;

use crate::clipboard::CapturedItem;
use crate::paste::{write_then_paste, PasteBackend};

/// macOS 辅助功能深链常量，跳转到系统设置 › 隐私与安全性 › 辅助功能。
///
/// 未授权时在说明卡片中提供此链接，引导用户手动授权。
pub const ACCESSIBILITY_DEEPLINK: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/// 抽象 OS Accessibility 授权检测，使引导逻辑与平台解耦、可测。
///
/// 实现者：
/// - 生产：调用 macOS `AXIsProcessTrusted()` C API
/// - 测试：`FakeAccessibilityProbe`（在测试文件中内联定义）
pub trait AccessibilityProbe {
    /// 返回当前进程是否已获得 Accessibility 授权。
    ///
    /// 对应 macOS `AXIsProcessTrusted()`：true = 已授权，false = 未授权。
    fn is_trusted(&self) -> bool;
}

/// 引导决策枚举：决定是否需要显示说明卡片并提供深链。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnboardingAction {
    /// 已授权，直接进行正常粘贴流程。
    Proceed,
    /// 未授权，应显示说明卡片并提供 `ACCESSIBILITY_DEEPLINK` 引导用户授权。
    ShowCardAndDeepLink,
}

/// 粘贴能力枚举：当前进程能做到的最高粘贴级别。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteCapability {
    /// 已授权：可写回剪贴板 + 模拟 Cmd+V（完整粘贴）。
    FullPaste,
    /// 未授权：仅可写回剪贴板，不发送 Cmd+V（CGEvent 注入无效）。
    WriteBackOnly,
}

/// 降级粘贴结果，区分实际执行路径以供调用方判断是否需要给用户提示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteOutcome {
    /// 走完整粘贴路径（write + send_paste）。
    FullPasteDone,
    /// 走降级路径，仅写回剪贴板，未发送 Cmd+V。
    WriteBackOnlyDone,
}

/// 依据探针结果决定引导动作（纯函数）。
///
/// - `is_trusted()` 为 true  → `Proceed`
/// - `is_trusted()` 为 false → `ShowCardAndDeepLink`
pub fn onboarding_action(probe: &impl AccessibilityProbe) -> OnboardingAction {
    if probe.is_trusted() {
        OnboardingAction::Proceed
    } else {
        OnboardingAction::ShowCardAndDeepLink
    }
}

/// 依据探针结果返回当前粘贴能力（纯函数）。
///
/// - `is_trusted()` 为 true  → `FullPaste`
/// - `is_trusted()` 为 false → `WriteBackOnly`
pub fn paste_capability(probe: &impl AccessibilityProbe) -> PasteCapability {
    if probe.is_trusted() {
        PasteCapability::FullPaste
    } else {
        PasteCapability::WriteBackOnly
    }
}

/// 执行粘贴或优雅降级（不崩溃）。
///
/// - 已授权：调用 `write_then_paste`（写回 + 等待 changeCount + 模拟 Cmd+V）
/// - 未授权：仅调用 `write_with_marker` 写回剪贴板，**不发送 send_paste**，
///   返回 `WriteBackOnlyDone` 降级标识，不 panic
///
/// 未授权时 CGEvent 注入会被 OS 静默丢弃，故主动跳过 send_paste，
/// 避免产生用户困惑（剪贴板已更新但粘贴未生效）。
pub fn perform_paste_or_degrade(
    probe: &impl AccessibilityProbe,
    backend: &mut dyn PasteBackend,
    item: &CapturedItem,
) -> Result<PasteOutcome, crate::paste::PasteError> {
    // 分支判据只认 `paste_capability`：同一个「能不能粘贴」问题只有一处答案，
    // 免得探针判定散成两份、将来只改一处就分裂出两套行为。
    match paste_capability(probe) {
        PasteCapability::FullPaste => {
            write_then_paste(backend, item)?;
            Ok(PasteOutcome::FullPasteDone)
        }
        PasteCapability::WriteBackOnly => {
            backend.write_with_marker(item);
            Ok(PasteOutcome::WriteBackOnlyDone)
        }
    }
}

// ---------------------------------------------------------------------------
// 首次引导向导状态机
// ---------------------------------------------------------------------------

/// 引导向导的四个步骤，枚举顺序即用户看到的前进方向；`Done` 是终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingStep {
    /// 欢迎：说明 QuickQuick 做什么。
    Welcome,
    /// 热键介绍：告诉用户怎么唤出面板。
    HotkeyIntro,
    /// 辅助功能授权：提供深链，未授权时粘贴降级为仅写回。
    AccessibilityAuth,
    /// 自启确认：复用既有 `get_launch_on_login` / `set_launch_on_login`。
    AutostartConfirm,
    /// 终态：引导此后不再出现。
    Done,
}

impl OnboardingStep {
    /// 步骤码：与前端 `GuideStep` 字面量一一对应，是跨语言契约的一部分。
    pub fn code(self) -> &'static str {
        match self {
            Self::Welcome => "welcome",
            Self::HotkeyIntro => "hotkey",
            Self::AccessibilityAuth => "accessibility",
            Self::AutostartConfirm => "autostart",
            Self::Done => "done",
        }
    }

    /// 序列中的下一步；`Done` 是吸收态，再前进仍是它自己。
    fn next(self) -> Self {
        match self {
            Self::Welcome => Self::HotkeyIntro,
            Self::HotkeyIntro => Self::AccessibilityAuth,
            Self::AccessibilityAuth => Self::AutostartConfirm,
            Self::AutostartConfirm | Self::Done => Self::Done,
        }
    }
}

/// 向导状态：当前步骤 + 最近一次观测到的辅助功能授权状态。
///
/// 两个字段一起才能决定下一步：授权已为真时，进到授权步骤这一步必须被跳过，
/// 否则已授权的用户会再被问一遍「去系统设置授权」（这正是本次要修的表现）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardingState {
    step: OnboardingStep,
    accessibility_trusted: bool,
}

impl OnboardingState {
    /// 首次进入向导：停在欢迎页，并记下当前授权观测值。
    pub fn new(accessibility_trusted: bool) -> Self {
        Self {
            step: OnboardingStep::Welcome,
            accessibility_trusted,
        }
    }

    /// 当前步骤。
    pub fn step(self) -> OnboardingStep {
        self.step
    }

    /// 最近一次观测到的授权状态。
    pub fn accessibility_trusted(self) -> bool {
        self.accessibility_trusted
    }

    /// 引导是否应渲染：终态不再出现。
    pub fn is_visible(self) -> bool {
        self.step != OnboardingStep::Done
    }

    /// 换到指定步骤，其余字段不变。
    fn at(self, step: OnboardingStep) -> Self {
        Self { step, ..self }
    }
}

/// 前进一步（纯函数）：已授权时跳过辅助功能授权步骤。
///
/// 跳过发生在**前进到该步骤的瞬间**，而不是进入向导时一次性算好：
/// 用户可能在向导开着的时候去系统设置点了允许，此时前进必须跟着新观测值走。
pub fn advance(state: OnboardingState) -> OnboardingState {
    let mut step = state.step.next();
    if step == OnboardingStep::AccessibilityAuth && state.accessibility_trusted {
        step = step.next();
    }
    state.at(step)
}

/// 跳过当前步骤（「稍后」/「跳过」语义，纯函数）。
///
/// 欢迎页与终态不可跳过——否则一进来就能把向导跳没，等于没有引导。
pub fn skip_step(state: OnboardingState) -> OnboardingState {
    match state.step {
        OnboardingStep::Welcome | OnboardingStep::Done => state,
        _ => advance(state),
    }
}

/// 观测辅助功能授权状态（纯函数）。
///
/// 关键分支：从未授权变为已授权、且正停在授权步骤时，该步骤**立即收起**并前进到
/// 自启确认。用户刚在系统设置里点完「允许」，不需要再回面板点一次「下一步」；
/// 收起的是「授权请求」这一步，而不是整个向导——后面还有自启确认要问。
pub fn observe_trust(state: OnboardingState, trusted: bool) -> OnboardingState {
    let updated = OnboardingState {
        accessibility_trusted: trusted,
        ..state
    };
    if trusted && state.step == OnboardingStep::AccessibilityAuth {
        return updated.at(OnboardingStep::AutostartConfirm);
    }
    updated
}

/// 结束引导（点「完成」或关闭按钮）：进入终态，此后不再出现。
pub fn finish(state: OnboardingState) -> OnboardingState {
    state.at(OnboardingStep::Done)
}

// ---------------------------------------------------------------------------
// 前端契约 DTO 与命令
// ---------------------------------------------------------------------------

/// 引导状态 DTO。
///
/// 字段名与取值字符串是和 `src/ipc/ipc-client.ts` 的 `OnboardingStatus` 之间的契约：
/// 两侧各有一条测试锚定这些字面量，改一侧必须同时改另一侧。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OnboardingStatusDto {
    /// `"proceed"`（已授权）或 `"show_accessibility_card"`（未授权）。
    pub action: &'static str,
    /// `"full_paste"` 或 `"write_back_only"`，与 `PasteResultDto.outcome` 共用取值。
    pub capability: &'static str,
}

/// 把引导决策映射为对前端稳定的取值。
pub fn action_code(action: &OnboardingAction) -> &'static str {
    match action {
        OnboardingAction::Proceed => "proceed",
        OnboardingAction::ShowCardAndDeepLink => "show_accessibility_card",
    }
}

/// 把粘贴能力映射为对前端稳定的取值。
pub fn capability_code(capability: &PasteCapability) -> &'static str {
    match capability {
        PasteCapability::FullPaste => "full_paste",
        PasteCapability::WriteBackOnly => "write_back_only",
    }
}

/// 组合引导状态（纯函数）：一次探针判定同时给出引导动作与粘贴能力。
///
/// 两者读的是同一次探针结果，不在两处各自判定——授权状态可能在两次读取之间变化。
pub fn onboarding_status(probe: &impl AccessibilityProbe) -> OnboardingStatusDto {
    OnboardingStatusDto {
        action: action_code(&onboarding_action(probe)),
        capability: capability_code(&paste_capability(probe)),
    }
}

/// 生产 Accessibility 探针：macOS 走 `AXIsProcessTrusted`，其它平台恒未授权。
#[cfg(target_os = "macos")]
fn platform_probe() -> impl AccessibilityProbe {
    crate::macos_paste::MacOsAccessibilityProbe
}

/// 生产 Accessibility 探针：非 macOS 平台恒未授权（与降级粘贴路径一致）。
#[cfg(not(target_os = "macos"))]
fn platform_probe() -> impl AccessibilityProbe {
    crate::macos_paste::FallbackAccessibilityProbe
}

/// Tauri 命令：读取首次引导所需的辅助功能授权状态（纯只读，无副作用）。
///
/// 前端在面板挂载与窗口重新获得焦点时各调一次：用户去系统设置点完「允许」回到面板，
/// 这次调用就会把 `action` 从 `show_accessibility_card` 翻成 `proceed`，向导随即收起授权步骤。
///
/// 判定身份是**可执行文件**：`AXIsProcessTrusted` 认的是当前进程的可执行文件路径，
/// 开发构建与打包构建是两个不同的可执行文件，两次 TCC 授权互不复用，故发布前的
/// 真机确认必须在两种构建下分别做。
#[tauri::command]
pub fn get_onboarding_status() -> OnboardingStatusDto {
    onboarding_status(&platform_probe())
}
