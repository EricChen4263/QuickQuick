//! 最近一个非 QuickQuick 前台 app 的追踪与显式激活（方案 B）
//!
//! 背景（修复主窗口"粘贴到前台"落空）：
//! 主窗口路径下 QuickQuick 长时间是前台 app，`app.hide()` 隐式让 macOS 把焦点
//! 还给"上一个 app"时，那个"上一个 app"已陈旧/错误 → Cmd+V 落空。
//! 方案 B 显式记录最近一个非自身前台 app 的 pid，粘贴时主动激活它。
//!
//! 本模块职责（headless 可测的纯逻辑 + 跨线程托管状态）：
//! - `LastExternalApp` — `Mutex<Option<i32>>` 托管状态，存 pid（i32）而非 ObjC 对象，
//!   规避跨线程 Send 问题；由观察者写、粘贴命令读。
//! - `should_record_pid` — 纯函数：判断某候选 pid 是否应被记录（排除自身/无效 pid）。
//! - `activation_decision` — 纯函数：依据已记录 pid 决定走"显式激活"还是"降级隐式路径"。
//!
//! 真实 NSWorkspace 通知注册与 NSRunningApplication 激活留 lib.rs / system.rs 运行期实现，
//! 只能 GUI 实测；本模块抽出的纯决策逻辑可在无 GUI 环境单测。

use std::sync::Mutex;

/// 最近一个非 QuickQuick 前台 app 的 pid 托管状态。
///
/// 存 pid（i32）而非 `NSRunningApplication`：ObjC 对象非 `Send`，无法跨
/// 观察者主线程 → 粘贴命令线程传递；pid 是普通整数，跨线程安全。
/// `None` 表示尚未记录到任何外部 app（启动初期或刚清空）。
pub struct LastExternalApp(Mutex<Option<i32>>);

impl LastExternalApp {
    /// 创建空状态（尚未记录到任何外部 app）。
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }

    /// 记录一个外部 app 的 pid。
    ///
    /// lock poison 时静默跳过（不 panic）：记录前台 pid 是尽力而为的优化，
    /// 偶发丢失只会让粘贴退回隐式降级路径，不影响正确性。
    pub fn set(&self, pid: i32) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(pid);
        }
    }

    /// 读取当前记录的外部 app pid；未记录或 lock poison 时返回 `None`。
    pub fn get(&self) -> Option<i32> {
        self.0.lock().ok().and_then(|guard| *guard)
    }
}

impl Default for LastExternalApp {
    fn default() -> Self {
        Self::new()
    }
}

/// 判断某候选前台 app 的 pid 是否应被记录为"最近外部 app"。
///
/// 排除三类不该记录的 pid：
/// - 等于 QuickQuick 自身 pid：自己激活自己粘贴无意义（且会激活回面板）。
/// - 0：macOS pid 从 1 起，0 是无效/内核占位值。
/// - 负数：无效 pid（防御性，NSRunningApplication.processIdentifier 理论不为负）。
pub fn should_record_pid(candidate_pid: i32, self_pid: i32) -> bool {
    candidate_pid > 0 && candidate_pid != self_pid
}

/// 粘贴时的激活决策：依据已记录的 pid 决定走显式激活还是降级隐式路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationDecision {
    /// 有有效 pid：粘贴前主动激活该 pid 对应的目标 app。
    ActivatePid(i32),
    /// 无可用 pid（尚未记录到）：跳过显式激活，回退原有 `app.hide()` 隐式路径。
    FallbackHide,
}

/// 依据已记录的外部 app pid 决定粘贴时的激活策略（纯函数）。
///
/// - `Some(pid)` 且 `pid > 0` → `ActivatePid(pid)`（显式激活目标）
/// - `None` 或非正 pid       → `FallbackHide`（降级隐式路径，不破坏 popover 流程）
///
/// 注意：此处只决策"是否尝试激活"；目标 app 可能已退出（runningApplication 返回 nil）
/// 的二次降级由运行期激活函数处理，本纯函数不感知 app 存活状态。
pub fn activation_decision(recorded_pid: Option<i32>) -> ActivationDecision {
    match recorded_pid {
        Some(pid) if pid > 0 => ActivationDecision::ActivatePid(pid),
        _ => ActivationDecision::FallbackHide,
    }
}
