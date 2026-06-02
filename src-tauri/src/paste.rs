//! 回写粘贴引擎（V1-F3-S06）
//!
//! 设计对齐：设计文档§三#3/#4 + §八#2
//!
//! 核心抽象：
//! - `PasteBackend` trait       — 抽象写剪贴板 + 模拟粘贴 + 前台 app，使逻辑层 headless 可测
//! - `FocusStep`                — 焦点恢复步骤枚举，冻结设计§八#2 规定的操作顺序契约
//! - `focus_restore_sequence`   — 返回规定顺序的 Vec<FocusStep>，纯函数可测
//! - `write_and_confirm`        — 只做写入 + 轮询 changeCount 确认，不调 send_paste
//! - `write_then_paste`         — 回写时序核心：write_and_confirm + send_paste
//! - `PasteError`               — 回写失败类型（当前：Timeout）
//!
//! 回写时序保证（A15）：
//! write_with_marker 写回剪贴板后，轮询 change_count 直到其 > 写前值（确认 OS
//! 已接受写入），才调用 send_paste——避免在写入尚未生效时提前粘贴到旧内容。
//! 超时时返回 PasteError::Timeout，不盲发粘贴。
//!
//! 粘贴归属（A18）：
//! 粘贴后剪贴板留下被选条目 X，不做"借用后恢复原剪贴板"。
//! write_with_marker 写入 X，send_paste 发出后剪贴板内容即为 X。

use thiserror::Error;

use crate::clipboard::CapturedItem;

/// 最大轮询次数（超时门限）。
///
/// fake 后端写入即递增，正常路径 1 次即过；
/// 冻结计数的 fake 后端会耗尽轮询次数触发 Timeout。
const MAX_POLL_ATTEMPTS: u32 = 200;

/// 回写粘贴失败类型。
#[derive(Debug, Error, PartialEq)]
pub enum PasteError {
    /// 写入后轮询 change_count 超出最大次数仍未反映写入，放弃粘贴。
    #[error("等待 changeCount 反映写入超时，放弃粘贴")]
    Timeout,
}

/// 抽象 OS 粘贴后端，使回写引擎与平台解耦、可测。
///
/// 实现者：
/// - 生产：macOS NSPasteboard + CGEvent 封装（运行期实现，Phase 1 范围外）
/// - 测试：`FakePasteBackend`（在测试文件中内联定义）
pub trait PasteBackend {
    /// 返回当前剪贴板单调递增变化计数（对应 NSPasteboard.changeCount）。
    fn change_count(&self) -> u64;

    /// 将条目写回剪贴板并附私有标记（防止轮询引擎自捕），写入应使 change_count 递增。
    fn write_with_marker(&mut self, item: &CapturedItem);

    /// 读取当前剪贴板文本，用于验证归属（A18）。
    fn current_text(&self) -> Option<String>;

    /// 向前台应用发送模拟粘贴（macOS: Cmd+V CGEvent）。
    fn send_paste(&mut self);
}

/// 焦点恢复步骤枚举（A17）。
///
/// 冻结设计§八#2 规定的五步操作顺序契约，纯数据可测。
/// 真实 OS 激活留运行期实现；此枚举仅表达"应做什么、按什么顺序"。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusStep {
    /// 步骤 1：记录当前前台应用（唤起面板前的应用）
    RecordFrontmost,
    /// 步骤 2：隐藏 QuickQuick 面板
    HidePanel,
    /// 步骤 3：激活原前台应用（将焦点还给原应用）
    ActivateOriginalApp,
    /// 步骤 4：等待原应用回到前台（确认焦点已交还）
    WaitForeground,
    /// 步骤 5：向原应用发送模拟粘贴（Cmd+V）
    SimulatePaste,
}

/// 返回焦点恢复操作的规定顺序（设计§八#2）。
///
/// 顺序冻结为契约：RecordFrontmost → HidePanel → ActivateOriginalApp
/// → WaitForeground → SimulatePaste。
/// 调用方（运行期）按此序列逐步执行真实 OS 操作。
pub fn focus_restore_sequence() -> Vec<FocusStep> {
    vec![
        FocusStep::RecordFrontmost,
        FocusStep::HidePanel,
        FocusStep::ActivateOriginalApp,
        FocusStep::WaitForeground,
        FocusStep::SimulatePaste,
    ]
}

/// 写入条目并等待 changeCount 反映写入（A15）。
///
/// # 职责边界
/// 只负责写入 + 轮询确认，**不调用 `send_paste`**。
/// 集成方（如 `run_paste_with_backend`）在调用此函数成功后自行决定是否 send_paste。
///
/// 若轮询 `MAX_POLL_ATTEMPTS` 次后 change_count 仍未递增，
/// 返回 `PasteError::Timeout`，调用方不应盲发粘贴。
pub fn write_and_confirm(
    backend: &mut dyn PasteBackend,
    item: &CapturedItem,
) -> Result<(), PasteError> {
    let count_before = backend.change_count();
    backend.write_with_marker(item);
    wait_for_count_increase(backend, count_before)
}

/// 回写时序核心：写入条目 → 等待 changeCount 反映写入 → 发模拟粘贴（A15）。
///
/// # 时序保证
/// 1. 调用 `write_and_confirm`（写入 + 轮询确认）
/// 2. 确认后调用 `send_paste`
///
/// 若 changeCount 未在 `MAX_POLL_ATTEMPTS` 次内递增，
/// 返回 `PasteError::Timeout`，不调用 `send_paste`（不盲发粘贴）。
///
/// 粘贴后剪贴板内容为条目 `item` 本身（设计§三#4，A18）。
pub fn write_then_paste(
    backend: &mut dyn PasteBackend,
    item: &CapturedItem,
) -> Result<(), PasteError> {
    write_and_confirm(backend, item)?;
    backend.send_paste();
    Ok(())
}

/// 轮询直到 change_count > `count_before` 或超时。
///
/// 独立为函数以降低 `write_then_paste` 嵌套层级。
fn wait_for_count_increase(
    backend: &dyn PasteBackend,
    count_before: u64,
) -> Result<(), PasteError> {
    for _ in 0..MAX_POLL_ATTEMPTS {
        if backend.change_count() > count_before {
            return Ok(());
        }
    }
    Err(PasteError::Timeout)
}
