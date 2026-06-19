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

/// Windows：判断某前台窗口句柄是否应被记录为"最近外部窗口"。
///
/// 唤起 popover/main 时捕获 `GetForegroundWindow()`，过滤掉：
/// - `hwnd == 0`：当前无前台窗口（GetForegroundWindow 返回 NULL）。
/// - 窗口所属进程即 QuickQuick 自身：自己激活自己粘贴无意义（会激活回面板）。
///
/// 纯函数，与 Win32 FFI 解耦，可在 macOS 上单测。
pub fn should_record_hwnd(hwnd: isize, hwnd_pid: u32, self_pid: u32) -> bool {
    hwnd != 0 && hwnd_pid != self_pid
}

/// Windows：失焦延迟复检后是否应执行隐藏（纯函数）。
///
/// 背景：WebView2 内部点击会让主窗口产生瞬时 `Focused(false)`，随即又聚焦回来。
/// 立即隐藏会造成"点一下就消失"的假隐藏。失焦后延迟一小段时间复检 `is_focused()`，
/// 仍为 false 才真正隐藏——滤掉这类瞬时假失焦。
///
/// `is_focused_after_delay`：延迟复检时窗口是否已重新聚焦。
/// 返回 true 表示仍未聚焦、应隐藏；false 表示已回到聚焦、应跳过隐藏。
pub fn should_hide_after_focus_recheck(is_focused_after_delay: bool) -> bool {
    !is_focused_after_delay
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

/// Windows：最近一个非 QuickQuick 前台窗口句柄（HWND）的托管状态。
///
/// 与 `LastExternalApp`（macOS 走 pid）平行：Windows 还焦用 `SetForegroundWindow(hwnd)`，
/// 故记录 HWND 而非 pid。存裸 `isize`（HWND 的整数表示）而非 `HWND` 指针类型——
/// 裸整数跨线程 `Send` 安全，由唤起回调写、粘贴命令线程读。
/// `None` 表示尚未捕获到任何外部窗口（启动初期）。
///
/// 独立于 `LastExternalApp` 新增，不改动其字段/纯函数/单测，避免牵连 macOS 路径。
#[cfg(target_os = "windows")]
pub struct LastExternalHwnd(Mutex<Option<isize>>);

#[cfg(target_os = "windows")]
impl LastExternalHwnd {
    /// 创建空状态（尚未捕获到任何外部窗口）。
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }

    /// 记录一个外部窗口句柄；`hwnd == 0`（无前台窗口）时跳过，不覆盖既有有效值。
    ///
    /// lock poison 时静默跳过（不 panic）：与 `LastExternalApp::set` 一致，
    /// 记录前台句柄是尽力而为的优化，偶发丢失只会让还焦退回隐式路径。
    pub fn set(&self, hwnd: isize) {
        if hwnd == 0 {
            return;
        }
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(hwnd);
        }
    }

    /// 读取当前记录的窗口句柄；未记录或 lock poison 时返回 `None`。
    pub fn get(&self) -> Option<isize> {
        self.0.lock().ok().and_then(|guard| *guard)
    }
}

#[cfg(target_os = "windows")]
impl Default for LastExternalHwnd {
    fn default() -> Self {
        Self::new()
    }
}

/// Windows：捕获当前前台窗口句柄并记入托管状态（唤起 popover/main 前调用）。
///
/// 取 `GetForegroundWindow()` → 经 `GetWindowThreadProcessId` 拿其所属 pid →
/// `should_record_hwnd` 过滤掉空句柄与自身进程 → 写入 `state`。
/// 这是唤起时刻"上一个外部前台窗口"的快照，供后续粘贴还焦时 `SetForegroundWindow`。
///
/// 由 popover.rs / tray.rs 在 `window.show()` 之前各自调用，集中 Win32 FFI 于一处（DRY）。
/// 拿不到 state（理论不发生，setup 已托管）时静默跳过。
#[cfg(target_os = "windows")]
pub fn capture_foreground_window(app: &tauri::AppHandle) {
    use std::sync::Arc;

    use tauri::Manager;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    let Some(state) = app.try_state::<Arc<LastExternalHwnd>>() else {
        return;
    };

    // GetForegroundWindow 返回 NULL（句柄 0）时表示当前无前台窗口；
    // 提前返回，避免对 NULL HWND 调用 GetWindowThreadProcessId（非法参数）。
    let hwnd = unsafe { GetForegroundWindow() } as isize;
    if hwnd == 0 {
        return;
    }

    // GetWindowThreadProcessId 通过 out 参数回填窗口所属进程 pid。
    let mut hwnd_pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd as _, &mut hwnd_pid) };

    if should_record_hwnd(hwnd, hwnd_pid, std::process::id()) {
        state.set(hwnd);
    }
}
