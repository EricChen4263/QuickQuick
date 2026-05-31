//! 配置反序列化守卫
//!
//! 为何不用真实 boot / mock_builder：
//! 本项目 tauri.conf.json 配置了 trayIcon，`mock_builder().build()` 在测试线程
//! 初始化托盘时立即返回 `Tray(NotMainThread)`，先于任何插件配置反序列化执行——
//! 即使 tauri.conf.json 存在非法插件配置块，测试也不会变红（假绿）。
//! mock_builder 路线在本项目不可行，放弃。
//!
//! 本守卫直接操作配置文件，覆盖两条关键路径：
//! 1. JSON 合法性：serde_json 解析，守卫尾逗号等语法错误。
//! 2. autostart 配置形态：tauri-plugin-autostart 期望 unit（()）配置，
//!    若存在 map 块则反序列化失败（复现本次 PluginInitialization 根因）。

use std::path::PathBuf;

/// 返回 tauri.conf.json 的绝对路径（相对于本测试文件所在包的 src-tauri 目录）。
fn conf_path() -> PathBuf {
    // CARGO_MANIFEST_DIR 指向 src-tauri/，tauri.conf.json 与 Cargo.toml 同级
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("tauri.conf.json")
}

/// tauri.conf.json 必须是合法 JSON，serde_json 解析不得报错。
///
/// 守卫场景：删插件块时漏删前一块尾逗号（trailing comma），
/// serde_json 严格模式会 Err。
#[test]
fn conf_json_is_valid() {
    // Arrange
    let path = conf_path();
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 {}: {e}", path.display()));

    // Act
    let result = serde_json::from_str::<serde_json::Value>(&raw);

    // Assert
    assert!(
        result.is_ok(),
        "tauri.conf.json 不是合法 JSON，请检查尾逗号或语法错误: {:?}",
        result.unwrap_err()
    );
}

/// plugins.autostart 必须可被反序列化为 ()（unit），不得是 map/object。
///
/// tauri-plugin-autostart 在框架内部调用 `serde_json::from_value::<()>(conf_value)`，
/// 若该字段是 `{"args": false, ...}` 这样的 map，反序列化为 unit 会 Err，
/// 触发 PluginInitialization 错误（本次 bug 的根因）。
/// 字段缺失等价于 Null，反序列化为 () 成功，视为正常。
#[test]
fn autostart_conf_deserializes_as_unit() {
    // Arrange
    let path = conf_path();
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 {}: {e}", path.display()));
    let conf: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("tauri.conf.json 解析失败: {e}"));

    let autostart_value = conf
        .pointer("/plugins/autostart")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    // Act: 复现 tauri-plugin-autostart 框架内部的反序列化路径
    let result = serde_json::from_value::<()>(autostart_value.clone());

    // Assert
    assert!(
        result.is_ok(),
        "plugins.autostart 无法反序列化为 unit ()，tauri-plugin-autostart 将触发 \
         PluginInitialization 错误。当前值: {autostart_value}，\
         请确保该字段不存在或为 null，不得为 object/map。"
    );
}
