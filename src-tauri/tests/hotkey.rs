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

/// 逐键写入：只改目标动作的键，其他动作与未知键原样保留。
#[test]
fn save_accelerator_only_touches_target_key() {
    // Arrange
    let dir = tempdir().expect("创建临时目录失败");
    let path = dir.path().join("hotkey.json");
    std::fs::write(
        &path,
        r#"{
  "history_accelerator": "CmdOrCtrl+Shift+H",
  "translate_accelerator": "CmdOrCtrl+Shift+Y",
  "main_accelerator": "CmdOrCtrl+Shift+M",
  "future_field": [1, 2, 3]
}"#,
    )
    .expect("写临时配置失败");

    // Act
    HotkeyConfig::save_accelerator(&path, HotkeyAction::Translate, "CmdOrCtrl+Shift+L")
        .expect("逐键写入应成功");

    // Assert
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("读回失败"))
            .expect("回写必须是合法 JSON");
    let map = written.as_object().expect("根必须是对象");
    assert_eq!(
        map["translate_accelerator"],
        serde_json::Value::from("CmdOrCtrl+Shift+L")
    );
    assert_eq!(
        map["history_accelerator"],
        serde_json::Value::from("CmdOrCtrl+Shift+H"),
        "其他动作的热键不得被改动"
    );
    assert_eq!(
        map["main_accelerator"],
        serde_json::Value::from("CmdOrCtrl+Shift+M")
    );
    assert_eq!(
        map["future_field"],
        serde_json::json!([1, 2, 3]),
        "未知键必须原样保留"
    );
}

/// 语法损坏的 hotkey.json：备份留痕后拒写，原文件逐字节不变。
#[test]
fn save_accelerator_rejects_broken_file_and_keeps_it_intact() {
    // Arrange
    let dir = tempdir().expect("创建临时目录失败");
    let path = dir.path().join("hotkey.json");
    let original = b"{ broken hotkey json ";
    std::fs::write(&path, original).expect("写临时配置失败");

    // Act
    let err = HotkeyConfig::save_accelerator(&path, HotkeyAction::History, "CmdOrCtrl+Shift+K")
        .expect_err("损坏文件必须拒写");

    // Assert
    let message = err.to_string();
    assert!(
        message.contains("hotkey.json") && message.contains("已拒绝写入"),
        "错误文案应点名文件并说明拒写，实际：{message}"
    );
    assert_eq!(
        std::fs::read(&path).expect("读回失败"),
        original,
        "拒写后原文件必须逐字节不变"
    );
    let backups: Vec<_> = std::fs::read_dir(dir.path())
        .expect("读目录失败")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("hotkey.json.") && name.ends_with(".bak"))
        })
        .collect();
    assert_eq!(
        backups.len(),
        1,
        "应留一份带时间戳的备份，实际：{backups:?}"
    );
}
