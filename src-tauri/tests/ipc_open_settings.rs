//! 集成测试：open_accessibility_settings 的子进程退出码处理。
//!
//! 验证 run_open_status_impl 同步等待子进程并按退出码返回 Ok/Err
//! （回归保护：spawn→status 修复，防止常驻进程下子进程僵尸泄漏）。

use quickquick_lib::ipc::system::run_open_status_impl;

/// 子进程退出码 0（success）应返回 Ok。
#[test]
fn success_exit_returns_ok() {
    let result = run_open_status_impl("true", "ignored-arg");
    assert!(result.is_ok(), "退出码 0 应返回 Ok，实际：{result:?}");
}

/// 子进程非零退出码应返回 Err，且错误信息说明打开失败。
#[test]
fn failure_exit_returns_err() {
    let result = run_open_status_impl("false", "ignored-arg");
    assert!(result.is_err(), "非零退出码应返回 Err");
    let msg = result.unwrap_err();
    assert!(msg.contains("失败"), "错误信息应说明打开失败，实际：{msg}");
}

/// 命令不存在（spawn 失败）应返回 Err，错误信息说明无法打开。
#[test]
fn missing_command_returns_err() {
    let result = run_open_status_impl("quickquick-nonexistent-binary-xyz", "ignored-arg");
    assert!(result.is_err(), "命令不存在应返回 Err");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("无法打开辅助功能设置"),
        "spawn 失败信息应说明无法打开，实际：{msg}"
    );
}
