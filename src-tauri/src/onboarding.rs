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

use crate::clipboard::CapturedItem;
use crate::paste::{PasteBackend, write_then_paste};

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
    if probe.is_trusted() {
        write_then_paste(backend, item)?;
        Ok(PasteOutcome::FullPasteDone)
    } else {
        backend.write_with_marker(item);
        Ok(PasteOutcome::WriteBackOnlyDone)
    }
}
