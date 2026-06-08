//! 集成测试：设置 IPC 命令实现层（impl 函数）
//!
//! 覆盖验收项 V4-F1-A03：设置 IPC 命令往返
//! - 热键 set 后 get 读回新值一致；set 冲突键 → 返回 Err
//! - 排除名单 set [a,b] 后 get 读回 [a,b]；空列表往返
//! - selected_provider set 合法 id 读回一致；set 非法 id → Err
//! - get_translate_providers 返回非空且含 "lingva"、不含已移除的 "mymemory"
//!
//! 测试约定：函数名含子串 `ipc_settings` 确保 verify 命中。

use quickquick_lib::hotkey::{HotkeyAction, HotkeyError, HotkeyRegistrar};
use quickquick_lib::ipc::settings::{
    get_exclude_list_impl, get_hotkeys_impl, get_selected_provider_impl,
    get_translate_providers_impl, set_exclude_list_impl, set_hotkey_impl, set_hotkey_runtime_impl,
    set_selected_provider_impl, RuntimeHotkeyRegistrar,
};
use std::path::PathBuf;
use std::sync::Mutex;

/// fake registrar：总是成功注册（用于测试正常改键路径）
struct AlwaysOkRegistrar;

impl HotkeyRegistrar for AlwaysOkRegistrar {
    fn register(&self, _accelerator: &str) -> Result<(), HotkeyError> {
        Ok(())
    }
}

/// fake registrar：对特定键返回 AlreadyInUse（用于测试冲突拒绝路径）
struct ConflictRegistrar {
    conflicting_key: String,
}

impl HotkeyRegistrar for ConflictRegistrar {
    fn register(&self, accelerator: &str) -> Result<(), HotkeyError> {
        if accelerator == self.conflicting_key {
            Err(HotkeyError::AlreadyInUse)
        } else {
            Ok(())
        }
    }
}

/// fake runtime registrar：记录命令层真实注册/注销顺序，避免启动 Tauri GUI。
#[derive(Default)]
struct RecordingRuntimeRegistrar {
    events: Mutex<Vec<String>>,
    failing_key: Option<String>,
}

impl RecordingRuntimeRegistrar {
    fn new(failing_key: Option<&str>) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            failing_key: failing_key.map(str::to_string),
        }
    }

    fn events(&self) -> Vec<String> {
        self.events.lock().expect("events lock 应可用").clone()
    }
}

impl RuntimeHotkeyRegistrar for RecordingRuntimeRegistrar {
    fn register_action_shortcut(
        &self,
        action: HotkeyAction,
        accelerator: &str,
    ) -> Result<(), String> {
        self.events
            .lock()
            .expect("events lock 应可用")
            .push(format!("register:{action:?}:{accelerator}"));
        if self.failing_key.as_deref() == Some(accelerator) {
            Err(format!("runtime register failed: {accelerator}"))
        } else {
            Ok(())
        }
    }

    fn unregister(&self, accelerator: &str) -> Result<(), String> {
        self.events
            .lock()
            .expect("events lock 应可用")
            .push(format!("unregister:{accelerator}"));
        Ok(())
    }
}

/// 生成唯一临时文件路径（不预创建文件，load_or_default 能处理不存在的情形）
fn tmp_hotkey_path() -> PathBuf {
    std::env::temp_dir().join(format!("quickquick_test_hotkey_{}.json", uuid_suffix()))
}

fn tmp_settings_path() -> PathBuf {
    std::env::temp_dir().join(format!("quickquick_test_settings_{}.json", uuid_suffix()))
}

/// 利用时间戳 + thread id 构造测试用唯一后缀，避免并行测试文件冲突
fn uuid_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let tid = format!("{:?}", std::thread::current().id());
    format!("{ts}_{}", tid.replace(['(', ')', ' '], "_"))
}

/// V4-F1-A03：set_hotkey 改键后 get_hotkeys 读回新值
#[test]
fn ipc_settings_set_hotkey_then_get_returns_new_value() {
    let hotkey_path = tmp_hotkey_path();
    let registrar = AlwaysOkRegistrar;

    // Arrange：改 history 键
    set_hotkey_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+H",
        &hotkey_path,
        &registrar,
    )
    .expect("set_hotkey 应成功");

    // Act：读回
    let dto = get_hotkeys_impl(&hotkey_path).expect("get_hotkeys 应成功");

    // Assert：history 已更新，translate 仍是默认值
    assert_eq!(dto.history, "CmdOrCtrl+Shift+H", "history 键应已更新");
    assert_eq!(dto.translate, "CmdOrCtrl+Shift+T", "translate 键应保持默认");
    assert_eq!(dto.main, "CmdOrCtrl+Shift+M", "main 键应保持默认");
}

/// V4-F1-A03：set_hotkey 改 main 后 get_hotkeys 读回新值
#[test]
fn ipc_settings_set_main_hotkey_then_get_returns_new_value() {
    let hotkey_path = tmp_hotkey_path();
    let registrar = AlwaysOkRegistrar;

    // Act：改 main 键
    set_hotkey_impl(
        HotkeyAction::Main,
        "CmdOrCtrl+Shift+Q",
        &hotkey_path,
        &registrar,
    )
    .expect("set_hotkey main 应成功");

    // Assert
    let dto = get_hotkeys_impl(&hotkey_path).expect("get_hotkeys 应成功");
    assert_eq!(dto.history, "CmdOrCtrl+Shift+C", "history 键应保持默认");
    assert_eq!(dto.translate, "CmdOrCtrl+Shift+T", "translate 键应保持默认");
    assert_eq!(dto.main, "CmdOrCtrl+Shift+Q", "main 键应已更新");
}

/// V4-F1-A03：set_hotkey 两动作设同一 accelerator → 冲突拒绝
#[test]
fn ipc_settings_set_hotkey_conflict_rejected() {
    let hotkey_path = tmp_hotkey_path();

    // 先把 history 设为某键（用 AlwaysOk）
    set_hotkey_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+X",
        &hotkey_path,
        &AlwaysOkRegistrar,
    )
    .expect("先行改键应成功");

    // 再把 translate 也设为同一键，用冲突 registrar 模拟系统已占用
    let registrar = ConflictRegistrar {
        conflicting_key: "CmdOrCtrl+Shift+X".to_string(),
    };
    let result = set_hotkey_impl(
        HotkeyAction::Translate,
        "CmdOrCtrl+Shift+X",
        &hotkey_path,
        &registrar,
    );

    // Assert：返回 Err，且错误信息含"已被占用"语义
    assert!(result.is_err(), "冲突键应返回 Err");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("已被占用") || err_msg.contains("冲突"),
        "错误信息应含冲突语义，实际: {err_msg}"
    );
}

#[test]
fn ipc_settings_runtime_register_failure_keeps_config_and_old_shortcut() {
    let hotkey_path = tmp_hotkey_path();
    set_hotkey_impl(
        HotkeyAction::Main,
        "CmdOrCtrl+Shift+O",
        &hotkey_path,
        &AlwaysOkRegistrar,
    )
    .expect("先行写入旧 main 热键应成功");
    let runtime = RecordingRuntimeRegistrar::new(Some("CmdOrCtrl+Shift+N"));

    let result = set_hotkey_runtime_impl(
        HotkeyAction::Main,
        "CmdOrCtrl+Shift+N",
        &hotkey_path,
        &runtime,
    );

    assert!(result.is_err(), "运行时注册失败应返回 Err");
    let dto = get_hotkeys_impl(&hotkey_path).expect("失败后配置仍应可读");
    assert_eq!(dto.main, "CmdOrCtrl+Shift+O", "不应保存失败的新 main 热键");
    assert_eq!(
        runtime.events(),
        vec!["register:Main:CmdOrCtrl+Shift+N"],
        "注册失败时不应注销旧热键"
    );
}

#[test]
fn ipc_settings_runtime_same_accelerator_is_noop() {
    let hotkey_path = tmp_hotkey_path();
    set_hotkey_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+O",
        &hotkey_path,
        &AlwaysOkRegistrar,
    )
    .expect("先行写入旧 history 热键应成功");
    let runtime = RecordingRuntimeRegistrar::default();

    set_hotkey_runtime_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+O",
        &hotkey_path,
        &runtime,
    )
    .expect("保存同一个热键应 no-op 成功");

    let dto = get_hotkeys_impl(&hotkey_path).expect("no-op 后配置仍应可读");
    assert_eq!(
        dto.history, "CmdOrCtrl+Shift+O",
        "原 history 热键应保持不变"
    );
    assert!(
        runtime.events().is_empty(),
        "no-op 不应调用运行时 register/unregister"
    );
}

/// V4-F1-A03：set_exclude_list [a,b] 后 get_exclude_list 读回 [a,b]
#[test]
fn ipc_settings_exclude_list_roundtrip() {
    let settings_path = tmp_settings_path();

    let apps = vec![
        "com.1password.1password".to_string(),
        "com.apple.keychainaccess".to_string(),
    ];

    // Act：写入
    set_exclude_list_impl(apps.clone(), &settings_path, None).expect("set_exclude_list 应成功");

    // Act：读回
    let loaded = get_exclude_list_impl(&settings_path).expect("get_exclude_list 应成功");

    // Assert：内容一致（排序无关）
    let mut sorted_apps = apps.clone();
    let mut sorted_loaded = loaded.clone();
    sorted_apps.sort();
    sorted_loaded.sort();
    assert_eq!(sorted_apps, sorted_loaded, "排除名单往返应一致");
}

/// V4-F1-A03：空列表往返正确
#[test]
fn ipc_settings_exclude_list_empty_roundtrip() {
    let settings_path = tmp_settings_path();

    set_exclude_list_impl(vec![], &settings_path, None).expect("空列表 set 应成功");

    let loaded = get_exclude_list_impl(&settings_path).expect("get 应成功");

    assert!(loaded.is_empty(), "空列表往返应仍为空");
}

/// V4-F1-A03：set_selected_provider 合法 id 读回一致
#[test]
fn ipc_settings_selected_provider_valid_id_roundtrip() {
    let settings_path = tmp_settings_path();

    set_selected_provider_impl("lingva", &settings_path).expect("合法 id 应成功");

    let loaded = get_selected_provider_impl(&settings_path).expect("get 应成功");

    assert_eq!(loaded, "lingva", "读回的 provider id 应一致");
}

/// V4-F1-A03：set_selected_provider 非法 id → Err
#[test]
fn ipc_settings_selected_provider_invalid_id_rejected() {
    let settings_path = tmp_settings_path();

    let result = set_selected_provider_impl("nonexistent_provider", &settings_path);

    assert!(result.is_err(), "非法 provider id 应返回 Err");
}

/// TV1-F1-A03：get_translate_providers 返回非空、含 "lingva" 且不含已移除的 "mymemory"
#[test]
fn ipc_settings_get_translate_providers_contains_lingva_not_mymemory() {
    let providers = get_translate_providers_impl();

    assert!(!providers.is_empty(), "provider 列表不应为空");
    assert!(
        providers.iter().any(|p| p.id == "lingva"),
        "provider 列表应含 lingva"
    );
    assert!(
        !providers.iter().any(|p| p.id == "mymemory"),
        "provider 列表不应再含 mymemory"
    );
}

/// TV1-F1-A03：持久化的旧值 mymemory（已移除）被 get 迁移回退为默认源 google_free（设计文档§六）
#[test]
fn ipc_settings_get_selected_provider_migrates_legacy_mymemory_to_default() {
    let settings_path = tmp_settings_path();
    std::fs::write(&settings_path, r#"{"selected_provider":"mymemory"}"#)
        .expect("写入旧 settings 应成功");

    let loaded = get_selected_provider_impl(&settings_path).expect("get 应成功");

    assert_eq!(
        loaded, "google_free",
        "旧值 mymemory 应迁移回退为默认源 google_free"
    );
}
