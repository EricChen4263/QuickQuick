//! 集成测试：设置 IPC 命令实现层（impl 函数）
//!
//! 覆盖验收项 V4-F1-A03：设置 IPC 命令往返
//! - 热键 set 后 get 读回新值一致；set 冲突键 → 返回 Err
//! - 排除名单 set [a,b] 后 get 读回 [a,b]；空列表往返
//! - selected_provider set 合法 id 读回一致；set 非法 id → Err
//! - get_translate_providers 返回非空且含 "lingva"、不含已移除的 "mymemory"
//!
//! 测试约定：函数名含子串 `ipc_settings` 确保 verify 命中。

use quickquick_lib::hotkey::HotkeyAction;
use quickquick_lib::ipc::settings::{
    get_exclude_list_impl, get_hotkeys_impl, get_selected_provider_impl,
    get_translate_providers_impl, set_exclude_list_impl, set_hotkey_runtime_impl,
    set_selected_provider_impl, RuntimeHotkeyRegistrar,
};
use std::path::PathBuf;
use std::sync::Mutex;

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

/// fake runtime registrar：注册一律返回「热键已被占用」的中文文案，模拟 OS 占用冲突。
struct OccupiedConflictRegistrar;

impl RuntimeHotkeyRegistrar for OccupiedConflictRegistrar {
    fn register_action_shortcut(
        &self,
        _action: HotkeyAction,
        _accelerator: &str,
    ) -> Result<(), String> {
        Err("热键已被占用，无法绑定".to_string())
    }

    fn unregister(&self, _accelerator: &str) -> Result<(), String> {
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

/// V4-F1-A03：set_hotkey 改 history 键（生产路径：注册新键 → 落盘 → 注销旧键）后读回新值
#[test]
fn ipc_settings_set_history_hotkey_runtime_then_get_returns_new_value() {
    let hotkey_path = tmp_hotkey_path();
    let runtime = RecordingRuntimeRegistrar::default();

    // Arrange：改 history 键
    set_hotkey_runtime_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+H",
        &hotkey_path,
        &runtime,
    )
    .expect("set_hotkey 应成功");

    // Act：读回
    let dto = get_hotkeys_impl(&hotkey_path).expect("get_hotkeys 应成功");

    // Assert：history 已持久化，translate / main 仍是默认值
    assert_eq!(dto.history, "CmdOrCtrl+Shift+H", "history 键应已更新");
    assert_eq!(dto.translate, "CmdOrCtrl+Shift+T", "translate 键应保持默认");
    assert_eq!(dto.main, "CmdOrCtrl+Shift+M", "main 键应保持默认");

    // Assert：生产路径顺序是「先注册新键、落盘成功后才注销旧键」
    assert_eq!(
        runtime.events(),
        vec![
            "register:History:CmdOrCtrl+Shift+H".to_string(),
            "unregister:CmdOrCtrl+Shift+C".to_string(),
        ],
        "应先注册新键、再注销旧键"
    );

    // Assert：重复设置同一键走同键早退，不再触发运行时注册/注销
    set_hotkey_runtime_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+H",
        &hotkey_path,
        &runtime,
    )
    .expect("重复设置同一键应早退成功");
    assert_eq!(
        runtime.events().len(),
        2,
        "同键早退不应新增注册/注销事件：{:?}",
        runtime.events()
    );
}

/// V4-F1-A03：set_hotkey 改 main 后 get_hotkeys 读回新值
#[test]
fn ipc_settings_set_main_hotkey_runtime_then_get_returns_new_value() {
    let hotkey_path = tmp_hotkey_path();
    let runtime = RecordingRuntimeRegistrar::default();

    // Act：改 main 键
    set_hotkey_runtime_impl(
        HotkeyAction::Main,
        "CmdOrCtrl+Shift+Q",
        &hotkey_path,
        &runtime,
    )
    .expect("set_hotkey main 应成功");

    // Assert：main 已更新，另两个动作保持默认
    let dto = get_hotkeys_impl(&hotkey_path).expect("get_hotkeys 应成功");
    assert_eq!(dto.history, "CmdOrCtrl+Shift+C", "history 键应保持默认");
    assert_eq!(dto.translate, "CmdOrCtrl+Shift+T", "translate 键应保持默认");
    assert_eq!(dto.main, "CmdOrCtrl+Shift+Q", "main 键应已更新");
}

/// V4-F1-A03：set_hotkey 改 translate 键后读回新值，且不串改其它动作
#[test]
fn ipc_settings_set_translate_hotkey_runtime_then_get_returns_new_value() {
    let hotkey_path = tmp_hotkey_path();
    let runtime = RecordingRuntimeRegistrar::default();

    // Act：改 translate 键
    set_hotkey_runtime_impl(
        HotkeyAction::Translate,
        "CmdOrCtrl+Shift+Y",
        &hotkey_path,
        &runtime,
    )
    .expect("set_hotkey translate 应成功");

    // Assert：translate 已更新，history / main 保持默认（字段互不串改）
    let dto = get_hotkeys_impl(&hotkey_path).expect("get_hotkeys 应成功");
    assert_eq!(dto.history, "CmdOrCtrl+Shift+C", "history 键应保持默认");
    assert_eq!(dto.translate, "CmdOrCtrl+Shift+Y", "translate 键应已更新");
    assert_eq!(dto.main, "CmdOrCtrl+Shift+M", "main 键应保持默认");

    // Assert：注销的是同一个动作的旧键，不误伤其它动作
    assert_eq!(
        runtime.events(),
        vec![
            "register:Translate:CmdOrCtrl+Shift+Y".to_string(),
            "unregister:CmdOrCtrl+Shift+T".to_string(),
        ],
        "应只注册/注销 Translate 动作的热键"
    );
}

/// V4-F1-A03：改键到已被占用的键 → 运行时注册失败原样上抛，配置不变、旧键不注销
///
/// 文案分工（勿静默改文案）：已占用类的**中文措辞**由 `ipc::settings::map_runtime_register_error`
/// 负责，逐字锁定在 `ipc_settings_map_runtime_register_error_*` 单元测试里；本用例只锁
/// 「注册失败的错误被原样上抛、不吞不改写」——这两层各管一段，避免措辞一改就要改一堆集成用例。
#[test]
fn ipc_settings_hotkey_conflict_rejected_runtime() {
    let hotkey_path = tmp_hotkey_path();

    // Arrange：先把 history 设为某键（走生产路径）
    set_hotkey_runtime_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+X",
        &hotkey_path,
        &RecordingRuntimeRegistrar::default(),
    )
    .expect("先行改键应成功");

    // Act：translate 也设为同一键，runtime 侧模拟系统已被占用
    let runtime = RecordingRuntimeRegistrar::new(Some("CmdOrCtrl+Shift+X"));
    let result = set_hotkey_runtime_impl(
        HotkeyAction::Translate,
        "CmdOrCtrl+Shift+X",
        &hotkey_path,
        &runtime,
    );

    // Assert：返回 Err，且注册失败的错误被原样上抛（未吞掉、未改写）
    assert!(result.is_err(), "冲突键应返回 Err");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("runtime register failed: CmdOrCtrl+Shift+X"),
        "运行时注册失败的错误应原样上抛，实际: {err_msg}"
    );

    // Assert：配置未被改动（失败的 translate 键没落盘，history 不受影响）
    let dto = get_hotkeys_impl(&hotkey_path).expect("失败后配置仍应可读");
    assert_eq!(dto.history, "CmdOrCtrl+Shift+X", "history 键应保持不变");
    assert_eq!(
        dto.translate, "CmdOrCtrl+Shift+T",
        "冲突时不应保存新 translate 键"
    );

    // Assert：失败发生在落盘与注销之前——只尝试过注册，没有注销旧键
    assert_eq!(
        runtime.events(),
        vec!["register:Translate:CmdOrCtrl+Shift+X".to_string()],
        "注册失败时不应注销旧热键"
    );
}

/// 冲突文案的落点：注册器给出的中文「已被占用」必须**一字不改**地到达命令层返回值
/// （前端 `HotkeyPanel` 把它原样放进 `role="alert"`，中间任何一层加前缀都会破坏验收口径）。
#[test]
fn ipc_settings_runtime_conflict_message_reaches_caller_verbatim() {
    let hotkey_path = tmp_hotkey_path();

    let result = set_hotkey_runtime_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+H",
        &hotkey_path,
        &OccupiedConflictRegistrar,
    );

    assert_eq!(
        result.unwrap_err(),
        "热键已被占用，无法绑定",
        "中文冲突文案应原样透传到调用方"
    );
}

#[test]
fn ipc_settings_runtime_register_failure_keeps_config_and_old_shortcut() {
    let hotkey_path = tmp_hotkey_path();
    set_hotkey_runtime_impl(
        HotkeyAction::Main,
        "CmdOrCtrl+Shift+O",
        &hotkey_path,
        &RecordingRuntimeRegistrar::default(),
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
    set_hotkey_runtime_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+O",
        &hotkey_path,
        &RecordingRuntimeRegistrar::default(),
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

/// 备份文件列表（`<name>.<ms>.bak`），用于断言「拒写时留痕」。
fn backup_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("读目录失败")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".json.") && name.ends_with(".bak"))
        })
        .collect();
    found.sort();
    found
}

/// 语法坏的 hotkey.json：setter 返回中文 Err（含修复指引）、原文件逐字节不变、留一份备份，
/// 且刚注册的新键被回滚（不留未持久化的临时绑定）。
#[test]
fn ipc_settings_set_hotkey_rejects_broken_config_file() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let hotkey_path = dir.path().join("hotkey.json");
    let original = b"{ broken hotkey json ";
    std::fs::write(&hotkey_path, original).expect("写入损坏配置应成功");
    let runtime = RecordingRuntimeRegistrar::default();

    let err = set_hotkey_runtime_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+H",
        &hotkey_path,
        &runtime,
    )
    .expect_err("损坏的 hotkey.json 必须拒写");

    assert!(
        err.contains("hotkey.json") && err.contains("已拒绝写入"),
        "错误文案应点名文件并给出修复指引，实际：{err}"
    );
    assert_eq!(
        std::fs::read(&hotkey_path).expect("读回失败"),
        original,
        "拒写后原文件必须逐字节不变"
    );
    assert_eq!(backup_files(dir.path()).len(), 1, "应留一份备份");
    assert_eq!(
        runtime.events(),
        vec![
            "register:History:CmdOrCtrl+Shift+H".to_string(),
            "unregister:CmdOrCtrl+Shift+H".to_string(),
        ],
        "落盘失败必须注销刚注册的新键"
    );
}

/// 语法坏的 autostart.json：setter 返回中文 Err、原文件逐字节不变、留一份备份。
#[test]
fn ipc_settings_set_launch_on_login_rejects_broken_config_file() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let autostart_path = dir.path().join("autostart.json");
    let original = b"{ not json ";
    std::fs::write(&autostart_path, original).expect("写入损坏配置应成功");

    // 注入 no-op apply：本用例只关心持久化路径的拒写行为
    let err = quickquick_lib::autostart::set_enabled_with_apply(&autostart_path, false, |_| Ok(()))
        .expect_err("损坏配置必须拒写");

    let message = err.to_string();
    assert!(
        message.contains("autostart.json") && message.contains("已拒绝写入"),
        "错误文案应点名文件并给出修复指引，实际：{message}"
    );
    assert_eq!(
        std::fs::read(&autostart_path).expect("读回失败"),
        original,
        "拒写后原文件必须逐字节不变"
    );
    assert_eq!(backup_files(dir.path()).len(), 1, "应留一份备份");
}

/// 正常文件：setter 逐键写入，未知键与其他字段原样保留。
#[test]
fn ipc_settings_key_level_writes_keep_unknown_keys() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let autostart_path = dir.path().join("autostart.json");
    std::fs::write(
        &autostart_path,
        r#"{"enabled":true,"future_field":{"kept":true}}"#,
    )
    .expect("写入配置应成功");

    quickquick_lib::autostart::set_enabled_with_apply(&autostart_path, false, |_| Ok(()))
        .expect("逐键写入应成功");

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&autostart_path).expect("读回失败"))
            .expect("回写必须是合法 JSON");
    let map = written.as_object().expect("根必须是对象");
    assert_eq!(
        map["enabled"],
        serde_json::Value::Bool(false),
        "enabled 应已改"
    );
    assert_eq!(
        map["future_field"],
        serde_json::json!({"kept": true}),
        "未知键必须原样保留"
    );
}

/// get_hotkeys 读取损坏文件：不再整体失败，而是返回默认键位 + corrupt 修复指引。
#[test]
fn ipc_settings_get_hotkeys_returns_corrupt_hint_instead_of_error() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let hotkey_path = dir.path().join("hotkey.json");
    std::fs::write(&hotkey_path, "{ broken hotkey json ").expect("写入损坏配置应成功");

    let dto = get_hotkeys_impl(&hotkey_path).expect("损坏不应再让读取整体失败");

    assert_eq!(dto.history, "CmdOrCtrl+Shift+C", "损坏时用默认键位");
    assert_eq!(dto.translate, "CmdOrCtrl+Shift+T");
    assert_eq!(dto.main, "CmdOrCtrl+Shift+M");
    let corrupt = dto.corrupt.expect("应附带修复指引");
    assert_eq!(corrupt.file, "hotkey.json");
    assert!(
        corrupt.hint.contains("已备份到") && corrupt.hint.contains(".bak"),
        "指引里应给出备份路径，实际：{}",
        corrupt.hint
    );
    assert!(
        !backup_files(dir.path()).is_empty(),
        "读路径应留下保护性备份"
    );
}

/// 正常文件读取时 corrupt 为 None。
#[test]
fn ipc_settings_get_hotkeys_has_no_corrupt_hint_for_valid_file() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let hotkey_path = dir.path().join("hotkey.json");
    std::fs::write(
        &hotkey_path,
        r#"{"history_accelerator":"CmdOrCtrl+Shift+H"}"#,
    )
    .expect("写配置应成功");

    let dto = get_hotkeys_impl(&hotkey_path).expect("get 应成功");

    assert!(dto.corrupt.is_none(), "正常文件不应带损坏指引");
    assert_eq!(dto.history, "CmdOrCtrl+Shift+H");
    assert_eq!(dto.translate, "CmdOrCtrl+Shift+T", "缺失键取默认值");
}

/// 同键早退不得绕过拒写：文件已损坏时，即使请求等于默认键，也必须返回 Err 并留备份。
///
/// 早退原本是为了「重复设置同一键不触发注册/注销」，但它同时跳过了落盘——
/// 文件已损坏时早退返回成功会让用户以为配置是好的，且损坏文件得不到备份留痕。
#[test]
fn ipc_settings_set_hotkey_same_key_still_rejects_broken_file() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let hotkey_path = dir.path().join("hotkey.json");
    let original = b"{ broken hotkey json ";
    std::fs::write(&hotkey_path, original).expect("写入损坏配置应成功");
    let runtime = RecordingRuntimeRegistrar::default();

    // 请求的就是默认 history 键 → 与容错读出的旧值相同，会走同键早退分支
    let err = set_hotkey_runtime_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+C",
        &hotkey_path,
        &runtime,
    )
    .expect_err("损坏文件即使请求同键也必须拒写");

    assert!(
        err.contains("hotkey.json") && err.contains("已拒绝写入"),
        "错误文案应点名文件并给出修复指引，实际：{err}"
    );
    assert_eq!(
        std::fs::read(&hotkey_path).expect("读回失败"),
        original,
        "拒写后原文件必须逐字节不变"
    );
    assert_eq!(backup_files(dir.path()).len(), 1, "应留下备份");
    assert!(
        runtime.events().is_empty(),
        "早退分支不该触碰运行时注册/注销，实际：{:?}",
        runtime.events()
    );
}

/// 同键但文件正常：早退照旧返回 Ok，不触发注册/注销。
#[test]
fn ipc_settings_set_hotkey_same_key_is_noop_for_valid_file() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let hotkey_path = dir.path().join("hotkey.json");
    std::fs::write(
        &hotkey_path,
        r#"{"history_accelerator":"CmdOrCtrl+Shift+H"}"#,
    )
    .expect("写配置应成功");
    let runtime = RecordingRuntimeRegistrar::default();

    set_hotkey_runtime_impl(
        HotkeyAction::History,
        "CmdOrCtrl+Shift+H",
        &hotkey_path,
        &runtime,
    )
    .expect("同键应早退成功");

    assert!(runtime.events().is_empty(), "同键不该触发注册/注销");
}
