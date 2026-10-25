//! 自启动偏好配置集成测试
//!
//! 验收项 V0-F1-A05：autostart 插件接入——自启动开关配置项存在、可读写、默认开。
//! 测试策略：只测偏好配置模型（AutostartConfig），不触发真实 OS LaunchAgent 注册。

use quickquick_lib::autostart::AutostartConfig;
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
