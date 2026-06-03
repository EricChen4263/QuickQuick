//! single-instance 插件接线的编译期约束测试。
//!
//! 多实例互斥行为本身依赖真实多进程、无法在 MockRuntime 下单测；
//! 本文件覆盖唯一可纯逻辑/编译期验证的部分：
//! `tauri-plugin-single-instance` 依赖已就绪，且其 `init` 接受
//! 我们在 `run()` 中实际使用的回调签名 `(app, argv, cwd)`。
//! 若依赖缺失或回调签名漂移（如插件升级改了参数列表），此测试编译失败（红）。
//!
//! `show_and_focus_window` 为 `pub(crate)`、集成测试不可见，故其可见性约束
//! 由 lib crate 自身编译保证（`run()` 内回调引用它）——本测试不重复覆盖。

use tauri::Wry;

/// `init` 应接受 `Fn(&AppHandle, Vec<String>, String)` 形态的回调，
/// 并返回一个可注册进 Tauri builder 的 `TauriPlugin`。
///
/// 这里以与 `run()` 中相同形态的回调（仅取 app、忽略 argv/cwd）调用 `init`，
/// 并把返回值绑定到 `TauriPlugin<Wry>` 类型，编译期锁定签名与返回类型。
#[test]
fn single_instance_init_accepts_app_argv_cwd_callback() {
    // Arrange & Act: 用与 run() 同形态的回调构造插件，绑定到具体插件类型
    let plugin: tauri::plugin::TauriPlugin<Wry> =
        tauri_plugin_single_instance::init(|_app, _argv, _cwd| {});

    // Assert: 插件名非空，确认确实构造出了可注册的插件实例
    use tauri::plugin::Plugin;
    assert!(
        !Plugin::<Wry>::name(&plugin).is_empty(),
        "single-instance 插件应有非空名称"
    );
}
