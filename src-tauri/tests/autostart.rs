//! 自启动偏好配置集成测试
//!
//! 验收项 V0-F1-A05：autostart 插件接入——自启动开关配置项存在、可读写、默认开。
//! 测试策略：只测偏好配置模型（AutostartConfig），不触发真实 OS LaunchAgent 注册。

use quickquick_lib::autostart::{set_enabled_with_apply, AutostartConfig};
use tempfile::NamedTempFile;

/// 验证自启动偏好默认为开（V0-F1-A05）
#[test]
fn autostart_default_on() {
    // Arrange & Act
    let config = AutostartConfig::default();

    // Assert：默认值必须为 true，与设计文档§二"默认开"严格对齐
    assert!(config.enabled, "自启动偏好默认值应为 true（默认开）");
}

/// 验证自启动偏好可持久化读写（改为 false → save → load → 仍为 false）
#[test]
fn autostart_persist_read_write() {
    // Arrange
    let tmp = NamedTempFile::new().expect("创建临时文件失败");
    let path = tmp.path();

    let mut config = AutostartConfig::default();
    assert!(config.enabled, "初始应为开");

    // Act：关闭自启动并持久化
    config.enabled = false;
    config.save(path).expect("save 不应失败");

    // Act：重新从文件加载
    let loaded = AutostartConfig::load(path).expect("load 不应失败");

    // Assert：加载回来的值应与写入一致
    assert!(!loaded.enabled, "持久化后 load 回来的 enabled 应为 false");
}

/// 验证文件不存在时 load_or_default 回退到默认值（enabled=true）
///
/// 场景：首次启动，配置文件尚未创建，setup 层调用 load_or_default 应得到默认开，
/// 而不是报错中断启动。
#[test]
fn autostart_load_or_default_when_file_not_exist() {
    // Arrange：tempdir 下不存在的文件路径（跨平台安全）
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("not_exist_autostart.json");

    // Act
    let config = AutostartConfig::load_or_default(&path);

    // Assert：文件不存在时 load_or_default 应回退 enabled=true
    assert!(
        config.enabled,
        "文件不存在时 load_or_default 应回退 enabled=true（默认开）"
    );
}

/// 逐键写入：只改 enabled，未知键与其他字段原样保留（对齐 settings.json 的逐键语义）。
#[test]
fn save_enabled_keeps_unknown_keys() {
    // Arrange：文件里既有 enabled，也有未来版本 / 别的工具写的键
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("autostart.json");
    std::fs::write(
        &path,
        r#"{
  "enabled": true,
  "launch_agent_label": "com.example.quickquick",
  "future_field": {"kept": true}
}"#,
    )
    .expect("写临时配置失败");

    // Act
    AutostartConfig::save_enabled(&path, false).expect("逐键写入应成功");

    // Assert：值已改，未知键一个不少
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("读回失败"))
            .expect("回写必须是合法 JSON");
    let map = written.as_object().expect("根必须是对象");
    assert_eq!(map["enabled"], serde_json::Value::Bool(false));
    assert_eq!(
        map["launch_agent_label"],
        serde_json::Value::from("com.example.quickquick")
    );
    assert_eq!(
        map["future_field"],
        serde_json::json!({"kept": true}),
        "未知键必须原样保留"
    );
}

/// 语法损坏的 autostart.json：**备份留痕后拒写**，原文件逐字节不变。
///
/// 与 settings.json 同一策略：无法解析的文件里可能有用户的真实配置，
/// 覆盖成「只含 enabled 的新文件」等于把它们删掉。
#[test]
fn save_enabled_rejects_broken_file_and_keeps_it_intact() {
    // Arrange
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("autostart.json");
    let original = b"{ not json at all ";
    std::fs::write(&path, original).expect("写临时配置失败");

    // Act
    let err = AutostartConfig::save_enabled(&path, false).expect_err("损坏文件必须拒写");

    // Assert：错误可读、原文件不动、留一份备份
    let message = err.to_string();
    assert!(
        message.contains("autostart.json") && message.contains("已拒绝写入"),
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
                .is_some_and(|name| name.starts_with("autostart.json.") && name.ends_with(".bak"))
        })
        .collect();
    assert_eq!(
        backups.len(),
        1,
        "应留一份带时间戳的备份，实际：{backups:?}"
    );
    assert_eq!(
        std::fs::read(&backups[0]).expect("读备份失败"),
        original,
        "备份必须是原文件完整副本"
    );
}

/// OS 登录项注册失败：配置回滚到写入前的字节，并返回中文 Err（界面走悲观更新、值不变）。
#[test]
fn set_enabled_rolls_back_config_when_os_apply_fails() {
    // Arrange：先有一份正常配置（false + 未知键），模拟用户在关闭状态下手动开启
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("autostart.json");
    let original = br#"{
  "enabled": false,
  "future_field": {"kept": true}
}"#;
    std::fs::write(&path, original).expect("写初始配置失败");

    // Act：写 true 成功，但 OS 注册失败
    let err = set_enabled_with_apply(&path, true, |_| Err("LaunchAgent 写入被拒绝".to_string()))
        .expect_err("OS 注册失败必须返回 Err");

    // Assert：错误文案可读、指向回滚结果
    let message = err.to_string();
    assert!(
        message.contains("系统登录项设置失败") && message.contains("已恢复为原设置"),
        "错误文案应说明失败并已回滚，实际：{message}"
    );
    assert!(
        message.contains("LaunchAgent 写入被拒绝"),
        "错误文案应带上底层原因，实际：{message}"
    );

    // Assert：配置文件回到写入前的值（enabled=false，未知键仍在）
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("读回失败"))
            .expect("回滚后应是合法 JSON");
    let map = written.as_object().expect("根必须是对象");
    assert_eq!(
        map["enabled"],
        serde_json::Value::Bool(false),
        "必须回滚到写入前的值"
    );
    assert_eq!(
        map["future_field"],
        serde_json::json!({"kept": true}),
        "回滚不得丢未知键"
    );
    // 界面以 get_launch_on_login 为准 → 回滚后读到的仍是原值，前端悲观更新保持不动
    assert!(
        !AutostartConfig::load_or_default(&path).enabled,
        "回滚后读到的值必须是原来的 false"
    );
}

/// OS 应用成功：配置保留新值，不产生回滚。
#[test]
fn set_enabled_keeps_new_value_when_os_apply_succeeds() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("autostart.json");
    std::fs::write(&path, br#"{"enabled": false}"#).expect("写初始配置失败");

    set_enabled_with_apply(&path, true, |enabled| {
        assert!(enabled, "应把新值交给 apply");
        Ok(())
    })
    .expect("OS 应用成功应返回 Ok");

    assert!(
        AutostartConfig::load_or_default(&path).enabled,
        "成功后配置应保留新值"
    );
}

/// 配置文件原本不存在 + OS 应用失败：回滚应把文件删掉，而不是凭空留一份配置。
#[test]
fn set_enabled_removes_config_when_apply_fails_and_file_was_absent() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("autostart.json");

    set_enabled_with_apply(&path, true, |_| Err("boom".to_string()))
        .expect_err("OS 应用失败必须返回 Err");

    assert!(
        !path.exists(),
        "写入前文件不存在时，回滚应删除文件而不是留一份凭空生成的配置"
    );
}

/// 假登录项实现：记录调用并返回预设结果，用来覆盖「条目已不存在」等平台差异。
struct FakeLauncher {
    is_enabled: Result<bool, String>,
    enable: Result<(), String>,
    disable: Result<(), String>,
    calls: std::cell::RefCell<Vec<&'static str>>,
}

impl FakeLauncher {
    fn new(
        is_enabled: Result<bool, String>,
        enable: Result<(), String>,
        disable: Result<(), String>,
    ) -> Self {
        Self {
            is_enabled,
            enable,
            disable,
            calls: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<&'static str> {
        self.calls.borrow().clone()
    }
}

impl quickquick_lib::autostart::AutostartLauncher for FakeLauncher {
    fn is_enabled(&self) -> Result<bool, String> {
        self.calls.borrow_mut().push("is_enabled");
        self.is_enabled.clone()
    }

    fn enable(&self) -> Result<(), String> {
        self.calls.borrow_mut().push("enable");
        self.enable.clone()
    }

    fn disable(&self) -> Result<(), String> {
        self.calls.borrow_mut().push("disable");
        self.disable.clone()
    }
}

/// 关闭自启时条目已不存在：视为成功（幂等），且配置写入 false。
///
/// Windows 上删除不存在的注册表项会报错，若不做这一步判断，用户会看到「关闭失败」却永远关不掉。
#[test]
fn disable_is_noop_success_when_entry_already_absent() {
    use quickquick_lib::autostart::apply_with;

    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("autostart.json");
    std::fs::write(&path, br#"{"enabled": true}"#).expect("写初始配置失败");

    // 条目已不存在；disable 若被调用会返回错误（用它来证明根本没调用）
    let launcher = FakeLauncher::new(Ok(false), Ok(()), Err("条目不存在，删除失败".into()));

    set_enabled_with_apply(&path, false, |enabled| apply_with(&launcher, enabled))
        .expect("条目已不存在时应幂等成功");

    assert_eq!(
        launcher.calls(),
        vec!["is_enabled"],
        "条目不存在就不该再调 disable"
    );
    assert!(
        !AutostartConfig::load_or_default(&path).enabled,
        "配置应写入 false"
    );
}

/// 条目仍在但删除失败：必须返回 Err（否则界面显示已关闭、系统仍会自启）。
#[test]
fn disable_returns_err_when_entry_still_exists_and_delete_fails() {
    use quickquick_lib::autostart::apply_with;

    let launcher = FakeLauncher::new(Ok(true), Ok(()), Err("拒绝访问".into()));

    let err = apply_with(&launcher, false).expect_err("条目仍在且删除失败必须报错");

    assert!(
        err.contains("关闭自启动失败") && err.contains("拒绝访问"),
        "错误应说明关闭失败并带底层原因，实际：{err}"
    );
    assert_eq!(launcher.calls(), vec!["is_enabled", "disable"]);
}

/// 读取当前状态失败时保守处理：仍尝试删除，并把读取失败写进错误。
#[test]
fn disable_still_tries_when_state_read_fails() {
    use quickquick_lib::autostart::apply_with;

    let launcher = FakeLauncher::new(Err("读注册表失败".into()), Ok(()), Ok(()));

    apply_with(&launcher, false).expect("读取失败但删除成功仍算成功");

    assert_eq!(launcher.calls(), vec!["is_enabled", "disable"]);
}

/// 启用失败直接返回 Err（启用没有「已达成」的省事路径）。
#[test]
fn enable_returns_err_when_registration_fails() {
    use quickquick_lib::autostart::apply_with;

    let launcher = FakeLauncher::new(Ok(false), Err("写注册表失败".into()), Ok(()));

    let err = apply_with(&launcher, true).expect_err("启用失败必须报错");

    assert!(
        err.contains("启用自启动失败") && err.contains("写注册表失败"),
        "错误应说明启用失败并带底层原因，实际：{err}"
    );
    assert_eq!(launcher.calls(), vec!["enable"], "启用路径不该先查状态");
}

/// 二阶失败：OS 注册失败、回滚写也失败 → 错误文案必须如实说明磁盘当前是新值、下次启动生效。
///
/// 只报「恢复原设置也失败」会让用户以为设置没变；实际磁盘上已经是新值，
/// 下次启动会按新值生效——不说清楚，用户会以为「没变化所以不用管」。
#[cfg(unix)]
#[test]
fn rollback_failure_message_states_disk_holds_new_value_and_takes_effect_next_launch() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("autostart.json");
    std::fs::write(&path, br#"{"enabled": false}"#).expect("写初始配置失败");

    // apply 里把**目录**设成只读再返回失败 → 新值已写入，回滚写（临时文件 + rename）必然失败。
    // 不能只设文件只读：原子写走 rename 覆盖，POSIX 上覆盖只读文件是允许的（权限在目录上）。
    let dir_path = dir.path().to_path_buf();
    let err = set_enabled_with_apply(&path, true, |_| {
        std::fs::set_permissions(&dir_path, std::fs::Permissions::from_mode(0o555))
            .expect("设置目录只读失败");
        Err("LaunchAgent 写入被拒绝".to_string())
    })
    .expect_err("回滚失败也必须返回 Err");

    // 恢复权限以便读取与清理
    std::fs::set_permissions(&dir_path, std::fs::Permissions::from_mode(0o755))
        .expect("恢复目录权限失败");

    let message = err.to_string();
    assert!(
        message.contains("系统登录项设置失败") && message.contains("LaunchAgent 写入被拒绝"),
        "错误应带上底层原因，实际：{message}"
    );
    assert!(
        message.contains("下次启动生效"),
        "必须告知磁盘当前是新值、将于下次启动生效，实际：{message}"
    );
    assert!(
        message.contains("配置文件当前为新值") && message.contains("请检查文件权限"),
        "必须告知磁盘真值并给出可操作建议，实际：{message}"
    );
    // 磁盘确实是新值（这正是文案要如实说明的那件事）
    assert!(
        AutostartConfig::load_or_default(&path).enabled,
        "回滚失败时磁盘上应仍是新值 true"
    );
}

/// 读取状态失败 + 删除也失败：错误串必须同时包含删除原因与读取原因。
///
/// 只报「关闭失败」会让排查断在半路：到底是「状态查不到」还是「删不掉」，
/// 两条原因都要留在文案里。
#[test]
fn disable_error_includes_both_delete_and_state_read_reasons() {
    use quickquick_lib::autostart::apply_with;

    let launcher = FakeLauncher::new(Err("读注册表失败".into()), Ok(()), Err("拒绝访问".into()));

    let err = apply_with(&launcher, false).expect_err("删除失败必须报错");

    assert!(
        err.contains("关闭自启动失败"),
        "错误应说明关闭失败，实际：{err}"
    );
    assert!(
        err.contains("拒绝访问"),
        "错误应包含删除失败的底层原因，实际：{err}"
    );
    assert!(
        err.contains("读注册表失败"),
        "错误应同时包含读取状态失败的原因，实际：{err}"
    );
    assert_eq!(launcher.calls(), vec!["is_enabled", "disable"]);
}

/// 回滚失败 + 原本不存在：删除回滚也失败时，文案同样要说明「下次启动生效」，且磁盘是新值。
#[cfg(unix)]
#[test]
fn rollback_failure_when_file_was_absent_reports_next_launch_and_keeps_new_value() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let path = dir.path().join("autostart.json");
    assert!(!path.exists(), "前置条件：配置文件原本不存在");

    // apply 里把目录设成只读 → 新值已写入，回滚的「删除文件」也会失败
    let dir_path = dir.path().to_path_buf();
    let err = set_enabled_with_apply(&path, true, |_| {
        std::fs::set_permissions(&dir_path, std::fs::Permissions::from_mode(0o555))
            .expect("设置目录只读失败");
        Err("LaunchAgent 写入被拒绝".to_string())
    })
    .expect_err("回滚失败也必须返回 Err");

    std::fs::set_permissions(&dir_path, std::fs::Permissions::from_mode(0o755))
        .expect("恢复目录权限失败");

    let message = err.to_string();
    assert!(
        message.contains("将于下次启动生效") || message.contains("下次启动生效"),
        "文案必须说明新值将于下次启动生效，实际：{message}"
    );
    assert!(
        message.contains("配置文件当前为新值") && message.contains("请检查文件权限"),
        "文案必须说明磁盘真值并给出可操作建议，实际：{message}"
    );
    assert!(
        path.exists() && AutostartConfig::load_or_default(&path).enabled,
        "回滚失败时磁盘上应仍是新值 true"
    );
}
