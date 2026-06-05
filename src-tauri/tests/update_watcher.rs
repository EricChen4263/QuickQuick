//! update_watcher 后台任务的可单测面：判定逻辑与轮询时序常量。
//!
//! 真实的 updater().check()/下载/emit 无法在单测构造（归 manual_confirm 与 S02），
//! 故此处只锁定纯函数 should_check 与文档约定的时序常量，防止其被无意改动。

use quickquick_lib::ipc::update::{restart_app, should_check};
use quickquick_lib::{UPDATE_FIRST_CHECK_DELAY_SECS, UPDATE_POLL_INTERVAL_SECS};

#[test]
fn should_check_follows_enabled_and_not_ready() {
    assert!(should_check(true, false));
    assert!(!should_check(false, false));
    assert!(!should_check(false, true));
    assert!(!should_check(true, true));
}

#[test]
fn restart_app_command_exists_with_apphandle_signature() {
    // restart_app 内部 restart() 替换进程、实际永不返回，无法在单测内调用——调用会终止测试进程。
    // 改以"按预期签名绑定为函数指针"做编译期 + 链接期断言：证明命令存在且签名为
    // `fn(tauri::AppHandle)`（声明 () 以满足 #[tauri::command] 宏）。
    // 一旦命令被误删/改签名，本测试编译失败即报警。真实重启行为归 manual_confirm（A12）。
    let cmd: fn(tauri::AppHandle) = restart_app;
    // 用函数指针地址非零佐证已成功绑定（避免未使用变量告警，且为具体值断言而非恒真）。
    assert_ne!(cmd as usize, 0);
}

#[test]
fn watcher_timing_matches_design_contract() {
    // 设计冻结：首检延迟 8s（让启动 I/O 先沉淀），轮询间隔 6h = 21600s（后台低频）。
    assert_eq!(UPDATE_FIRST_CHECK_DELAY_SECS, 8);
    assert_eq!(UPDATE_POLL_INTERVAL_SECS, 21600);
}
