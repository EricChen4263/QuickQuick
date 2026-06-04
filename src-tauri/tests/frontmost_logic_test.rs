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
    activation_decision, should_record_pid, ActivationDecision, LastExternalApp,
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
