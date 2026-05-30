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
    assert!(
        config.enabled,
        "自启动偏好默认值应为 true（默认开）"
    );
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
    assert!(
        !loaded.enabled,
        "持久化后 load 回来的 enabled 应为 false"
    );
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
