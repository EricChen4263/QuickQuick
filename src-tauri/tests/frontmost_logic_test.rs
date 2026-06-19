//! frontmost 模块纯逻辑的集成测试（headless 可测部分）。
//!
//! 覆盖方案 B 中可在无 GUI 环境验证的纯决策逻辑：
//! - `should_record_pid`   — pid 过滤（排除自身/0/负数）
//! - `LastExternalApp`     — 托管状态 get/set 语义
//! - `activation_decision` — 粘贴时"显式激活 vs 降级"的两分支决策
//!
//! 真实 NSWorkspace 通知触发、NSRunningApplication 激活、Cmd+V 落点
//! 只能 GUI 实测，不在此文件覆盖。

use quickquick_lib::frontmost::{
    activation_decision, should_hide_after_focus_recheck, should_record_hwnd, should_record_pid,
    ActivationDecision, LastExternalApp,
};

/// should_record_pid：候选 pid 等于自身 pid 时不记录（自己激活自己无意义）。
#[test]
fn should_record_pid_rejects_own_pid() {
    assert!(
        !should_record_pid(1234, 1234),
        "候选 pid 等于自身 pid 时不应记录"
    );
}

/// should_record_pid：候选 pid 是其他正常 app 时应记录。
#[test]
fn should_record_pid_accepts_external_pid() {
    assert!(
        should_record_pid(5678, 1234),
        "外部 app 的正常 pid 应被记录"
    );
}

/// should_record_pid：pid 为 0（无效占位）时不记录。
#[test]
fn should_record_pid_rejects_zero() {
    assert!(!should_record_pid(0, 1234), "pid=0 是无效值，不应记录");
}

/// should_record_pid：负数 pid（防御性）不记录。
#[test]
fn should_record_pid_rejects_negative() {
    assert!(!should_record_pid(-1, 1234), "负数 pid 无效，不应记录");
}

/// LastExternalApp：新建状态 get 返回 None。
#[test]
fn last_external_app_new_is_none() {
    let state = LastExternalApp::new();
    assert_eq!(state.get(), None, "新建状态应无记录");
}

/// LastExternalApp：set 后 get 返回该 pid。
#[test]
fn last_external_app_set_then_get_returns_pid() {
    let state = LastExternalApp::new();
    state.set(4321);
    assert_eq!(state.get(), Some(4321), "set 后 get 应返回该 pid");
}

/// LastExternalApp：后写覆盖前写（始终保留最近一个）。
#[test]
fn last_external_app_set_overwrites_previous() {
    let state = LastExternalApp::new();
    state.set(100);
    state.set(200);
    assert_eq!(state.get(), Some(200), "后写应覆盖前写，保留最近 pid");
}

/// activation_decision：有有效 pid → 显式激活该 pid。
#[test]
fn activation_decision_with_pid_activates() {
    assert_eq!(
        activation_decision(Some(9999)),
        ActivationDecision::ActivatePid(9999),
        "有有效 pid 应走显式激活"
    );
}

/// activation_decision：None → 降级隐式路径。
#[test]
fn activation_decision_none_falls_back() {
    assert_eq!(
        activation_decision(None),
        ActivationDecision::FallbackHide,
        "无记录 pid 应降级隐式路径"
    );
}

/// activation_decision：非正 pid（防御）→ 降级隐式路径。
#[test]
fn activation_decision_nonpositive_falls_back() {
    assert_eq!(
        activation_decision(Some(0)),
        ActivationDecision::FallbackHide,
        "pid=0 应降级而非激活无效目标"
    );
}

/// should_record_hwnd（Windows 前台捕获）：有效 hwnd 且属于外部进程时应记录。
#[test]
fn should_record_hwnd_accepts_external_window() {
    // hwnd 非 0 且其所属 pid 不等于自身 pid → 记录
    assert!(
        should_record_hwnd(0x1234, 5678, 1234),
        "外部进程的有效窗口句柄应被记录"
    );
}

/// should_record_hwnd：hwnd 为 0（无前台窗口）时不记录。
#[test]
fn should_record_hwnd_rejects_zero_hwnd() {
    assert!(
        !should_record_hwnd(0, 5678, 1234),
        "hwnd=0 表示无前台窗口，不应记录"
    );
}

/// should_record_hwnd：窗口属于自身进程时不记录（自己激活自己粘贴无意义）。
#[test]
fn should_record_hwnd_rejects_own_process() {
    assert!(
        !should_record_hwnd(0x1234, 1234, 1234),
        "属于自身进程的窗口不应记录"
    );
}

/// should_hide_after_focus_recheck（Windows 失焦延迟复检）：复检仍失焦 → 应隐藏。
#[test]
fn should_hide_after_focus_recheck_hides_when_still_unfocused() {
    assert!(
        should_hide_after_focus_recheck(false),
        "延迟复检后仍未聚焦应执行隐藏"
    );
}

/// should_hide_after_focus_recheck：复检已重新聚焦 → 不隐藏（滤掉瞬时假失焦）。
#[test]
fn should_hide_after_focus_recheck_skips_when_refocused() {
    assert!(
        !should_hide_after_focus_recheck(true),
        "延迟复检后已重新聚焦应跳过隐藏（瞬时假失焦）"
    );
}

/// 方案 C 关窗还焦的复合契约：state 未记录 pid 时，命令读 state.get() 喂给 activation_decision，
/// 应得 FallbackHide（退化为纯 app.hide 隐式还焦，不 panic、不尝试激活无效目标）。
#[test]
fn hide_and_return_focus_falls_back_when_no_pid_recorded() {
    // Arrange：模拟新启动尚未记录到任何外部 app 的状态。
    let state = LastExternalApp::new();

    // Act：复现命令体内 last_external.get() → activation_decision 的取值链。
    let decision = activation_decision(state.get());

    // Assert
    assert_eq!(
        decision,
        ActivationDecision::FallbackHide,
        "未记录 pid 时关窗还焦应降级隐式路径"
    );
}

/// 方案 C 关窗还焦的复合契约：state 已记录有效 pid 时，命令应据此显式激活该目标 app。
#[test]
fn hide_and_return_focus_activates_recorded_pid() {
    // Arrange：模拟已观察到外部前台 app（pid=7788）的状态。
    let state = LastExternalApp::new();
    state.set(7788);

    // Act
    let decision = activation_decision(state.get());

    // Assert
    assert_eq!(
        decision,
        ActivationDecision::ActivatePid(7788),
        "已记录 pid 时关窗还焦应显式激活该目标"
    );
}
