//! 回归守卫：macOS 启动激活策略必须为 Accessory（后台不占 Dock）。
//!
//! 为何只测「决策值」而非真实 Dock 行为：Dock 是否显示图标是纯 GUI/全局状态，
//! headless 无法自动化（见设计文档 docs/design/dock-icon-accessory.md 六章 M1）。
//! 可测层只锚定 `macos_startup_activation_policy()` 的返回值——若有人误把
//! Accessory 改回 Regular，此断言失败，构成真实回归锚而非橡皮图章。
//!
//! 该函数与断言仅在 macOS 下有意义，故整文件 cfg(macos) 门控。

#![cfg(target_os = "macos")]

/// 启动激活策略的决策值必须是 Accessory；变异成 Regular 应使此测试失败。
#[test]
fn macos_startup_activation_policy_is_accessory() {
    // Act
    let policy = quickquick_lib::macos_startup_activation_policy();

    // Assert
    assert!(
        matches!(policy, tauri::ActivationPolicy::Accessory),
        "启动激活策略必须为 Accessory（后台不占 Dock），不得为 Regular"
    );
}
