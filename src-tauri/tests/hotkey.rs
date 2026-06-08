//! 集成测试：热键配置默认值、改键持久化与冲突检测
//!
//! 对应验收项：V0-F2-A01、V0-F2-A02

use quickquick_lib::hotkey::{HotkeyAction, HotkeyConfig, HotkeyError, HotkeyRegistrar};
use tempfile::tempdir;

/// fake registrar：总是成功注册
struct AlwaysOkRegistrar;

impl HotkeyRegistrar for AlwaysOkRegistrar {
    fn register(&self, _accelerator: &str) -> Result<(), HotkeyError> {
        Ok(())
    }
}

/// fake registrar：对特定键返回 AlreadyInUse，其余成功
struct ConflictRegistrar {
    /// 冲突的加速键字符串
    conflicting_key: String,
}

impl HotkeyRegistrar for ConflictRegistrar {
    fn register(&self, accelerator: &str) -> Result<(), HotkeyError> {
        // 仅当加速键与冲突键完全相同时拒绝
        if accelerator == self.conflicting_key {
            Err(HotkeyError::AlreadyInUse)
        } else {
            Ok(())
        }
    }
}

/// V0-F2-A01：断言默认热键正确；rebind 后配置更新；save/load 往返一致
#[test]
fn hotkey_defaults_and_rebind() {
    // Arrange
    let mut config = HotkeyConfig::default();

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

    // Act：rebind History 到新键（用总是成功的 fake registrar）
    let registrar = AlwaysOkRegistrar;
    let result = config.rebind(HotkeyAction::History, "CmdOrCtrl+Shift+H", &registrar);

    // Assert：rebind 成功且配置已更新
    assert!(result.is_ok(), "rebind 应成功：{:?}", result);
    assert_eq!(
        config.get_accelerator(HotkeyAction::History),
        "CmdOrCtrl+Shift+H",
        "rebind 后历史热键应更新"
    );

    // Act：持久化往返
    let dir = tempdir().expect("创建临时目录失败");
    let config_path = dir.path().join("hotkey.json");

    config.save(&config_path).expect("save 失败");
    let loaded = HotkeyConfig::load(&config_path).expect("load 失败");

    // Assert：load 回来的值与 save 前一致
    assert_eq!(
        loaded.get_accelerator(HotkeyAction::History),
        "CmdOrCtrl+Shift+H",
        "load 回的 History 键应与 save 前一致"
    );
    assert_eq!(
        loaded.get_accelerator(HotkeyAction::Translate),
        "CmdOrCtrl+Shift+T",
        "load 回的 Translate 键应未变"
    );
}

/// V0-F2-A01（补）：rebind Translate 只改 Translate 字段，History 字段保持默认不变
#[test]
fn hotkey_rebind_translate_isolates_field() {
    // Arrange
    let mut config = HotkeyConfig::default();
    let registrar = AlwaysOkRegistrar;

    // Act：将 Translate 改绑到新键
    let result = config.rebind(HotkeyAction::Translate, "CmdOrCtrl+Shift+Y", &registrar);

    // Assert：rebind 成功，Translate 已更新
    assert!(result.is_ok(), "Translate rebind 应成功：{:?}", result);
    assert_eq!(
        config.get_accelerator(HotkeyAction::Translate),
        "CmdOrCtrl+Shift+Y",
        "rebind 后 Translate 热键应更新"
    );

    // Assert：History 字段保持默认，未被串改
    assert_eq!(
        config.get_accelerator(HotkeyAction::History),
        "CmdOrCtrl+Shift+C",
        "rebind Translate 不应影响 History 字段"
    );
}

/// V0-F2-A02：冲突键 rebind → 返回错误且错误含"已被占用"；原配置不变；不 panic
#[test]
fn hotkey_conflict_rejected() {
    // Arrange
    let mut config = HotkeyConfig::default();
    let original_history = config.get_accelerator(HotkeyAction::History).to_owned();

    let registrar = ConflictRegistrar {
        conflicting_key: "CmdOrCtrl+Shift+X".to_string(),
    };

    // Act：尝试绑定到冲突键
    let result = config.rebind(HotkeyAction::History, "CmdOrCtrl+Shift+X", &registrar);

    // Assert：应返回错误
    assert!(result.is_err(), "冲突键 rebind 应失败");

    // Assert：错误信息含"已被占用"
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("已被占用"),
        "错误信息应含'已被占用'，实际: {err_msg}"
    );

    // Assert：原配置未被改动
    assert_eq!(
        config.get_accelerator(HotkeyAction::History),
        original_history,
        "冲突时原配置不应改动"
    );
}
