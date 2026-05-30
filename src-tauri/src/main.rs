// Tauri 应用二进制入口（瘦入口，实现在 lib.rs）
// Windows 平台禁止弹出控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    quickquick_lib::run();
}
