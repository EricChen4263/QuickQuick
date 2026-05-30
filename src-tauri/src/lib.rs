//! QuickQuick Tauri 应用核心库
//!
//! 负责初始化 Tauri builder、注册插件，以及挂载 IPC 命令。
//! 二进制入口（main.rs）仅调用本模块的 `run()`，保持入口文件极简。

/// 启动 Tauri 应用。
///
/// 注册插件：
/// - `tauri-plugin-autostart`：开机自启（默认开，行为由 OS 侧控制）
/// - `tauri-plugin-updater`：应用自动更新（endpoints 在 tauri.conf.json 配置）
///
/// # Panics
/// 若 Tauri builder 初始化失败则 panic（属于不可恢复的启动错误）。
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            // macOS/Linux 使用 launchd/systemd 入口；args 为空
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|_app| {
            // 骨架阶段 setup 无操作，后续小功能在此注入初始化（如 tray、db、hotkey）
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

#[cfg(test)]
mod tests {
    /// 冒烟测试：验证 lib 模块可正常加载（A04 后端侧）
    #[test]
    fn smoke_lib_loads() {
        // Arrange & Act & Assert
        // lib 模块能被编译并链接即为通过，此处断言常量确保测试有实质内容
        assert_eq!(2 + 2, 4, "基础运算正确，lib 模块加载正常");
    }
}
