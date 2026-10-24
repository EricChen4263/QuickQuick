//! 集成测试：热键配置默认值与持久化往返
//!
//! 对应验收项：V0-F2-A01
//!
//! 改键与冲突检测的可测逻辑都在生产路径上（运行时注册 → 落盘 → 注销旧键），
//! 因此那些断言位于 `tests/ipc_settings.rs` 针对 `set_hotkey_runtime_impl` 的用例；
//! 本文件只覆盖**不涉及系统注册**的配置读写：默认值、save/load 往返、旧配置兼容。

use quickquick_lib::hotkey::{HotkeyAction, HotkeyConfig};
use tempfile::tempdir;

/// V0-F2-A01：断言默认热键正确；save/load 往返一致
#[test]
fn hotkey_defaults_and_save_load_roundtrip() {
    // Arrange
    let config = HotkeyConfig::default();

    // Assert 默认值
    assert_eq!(
        config.get_accelerator(HotkeyAction::History),
        "CmdOrCtrl+Shift+C",
        "历史热键默认值不符"
    );
    assert_eq!(
        config.get_accelerator(HotkeyAction::Translate),
        "CmdOrCtrl+Shift+T",
        "翻译热键默认值不符"
    );
    assert_eq!(
        config.get_accelerator(HotkeyAction::Main),
        "CmdOrCtrl+Shift+M",
        "主界面热键默认值不符"
    );

    // Act：持久化往返
    let dir = tempdir().expect("创建临时目录失败");
    let config_path = dir.path().join("hotkey.json");

    config.save(&config_path).expect("save 失败");
    let loaded = HotkeyConfig::load(&config_path).expect("load 失败");

    // Assert：load 回来的值与 save 前一致
    for (action, expected) in [
        (HotkeyAction::History, "CmdOrCtrl+Shift+C"),
        (HotkeyAction::Translate, "CmdOrCtrl+Shift+T"),
        (HotkeyAction::Main, "CmdOrCtrl+Shift+M"),
    ] {
        assert_eq!(
            loaded.get_accelerator(action),
            expected,
            "load 回的 {action:?} 键应与 save 前一致"
        );
    }
}

/// V0-F2-A01（补）：非默认热键落盘后 save/load 往返一致，且不影响其它字段
///
/// 对应原 `hotkey_defaults_and_rebind` 里「改键后 save/load 往返」的那一半断言：
/// 改键动作已由 `ipc_settings_*_runtime_then_get_returns_new_value` 在生产路径上覆盖，
/// 本用例独立守住「自定义值经磁盘往返仍不丢、不串字段」。
#[test]
fn hotkey_custom_accelerator_survives_save_load_roundtrip() {
    // Arrange：写一份非默认 history 的配置（等价于改键成功后的落盘结果）
    let dir = tempdir().expect("创建临时目录失败");
    let first_path = dir.path().join("hotkey.json");
    std::fs::write(
        &first_path,
        r#"{
  "history_accelerator": "CmdOrCtrl+Shift+H",
  "translate_accelerator": "CmdOrCtrl+Shift+T",
  "main_accelerator": "CmdOrCtrl+Shift+M"
}"#,
    )
    .expect("写入自定义配置失败");

    // Act：读出后再写到新路径，二次读回
    let loaded = HotkeyConfig::load(&first_path).expect("load 失败");
    let second_path = dir.path().join("hotkey-roundtrip.json");
    loaded.save(&second_path).expect("save 失败");
    let reloaded = HotkeyConfig::load(&second_path).expect("二次 load 失败");

    // Assert：自定义 history 保留，另两个字段未被串改
    for (action, expected) in [
        (HotkeyAction::History, "CmdOrCtrl+Shift+H"),
        (HotkeyAction::Translate, "CmdOrCtrl+Shift+T"),
        (HotkeyAction::Main, "CmdOrCtrl+Shift+M"),
    ] {
        assert_eq!(
            reloaded.get_accelerator(action),
            expected,
            "往返后 {action:?} 键应保持一致"
        );
    }
}

/// 旧 hotkey.json 只有 history/translate 字段时，load 应补齐 main 默认值
#[test]
fn hotkey_load_legacy_config_without_main_uses_default_main() {
    // Arrange
    let dir = tempdir().expect("创建临时目录失败");
    let config_path = dir.path().join("hotkey.json");
    std::fs::write(
        &config_path,
        r#"{
  "history_accelerator": "CmdOrCtrl+Shift+H",
  "translate_accelerator": "CmdOrCtrl+Shift+Y"
}"#,
    )
    .expect("写入旧配置失败");

    // Act
    let loaded = HotkeyConfig::load(&config_path).expect("旧配置 load 应成功");

    // Assert
    assert_eq!(
        loaded.get_accelerator(HotkeyAction::History),
        "CmdOrCtrl+Shift+H",
        "旧配置 history 应保留"
    );
    assert_eq!(
        loaded.get_accelerator(HotkeyAction::Translate),
        "CmdOrCtrl+Shift+Y",
        "旧配置 translate 应保留"
    );
    assert_eq!(
        loaded.get_accelerator(HotkeyAction::Main),
        "CmdOrCtrl+Shift+M",
        "旧配置缺 main 时应补默认值"
    );
}
