//! IPC 命令模块
//!
//! 定义 Tauri 托管状态类型 `AppDb` 与所有命令子模块。
//! 命令注册到 `invoke_handler` 由 S04 启动管道负责，本模块只声明类型与命令函数。
//!
//! 子模块：
//! - `clipboard`：剪贴板取数/管理命令（list/delete/toggle_favorite）

pub mod clipboard;
pub mod translate;

/// Tauri 托管状态：持有数据库连接（Mutex 包裹保证跨命令线程安全）。
///
/// 命令函数通过 `tauri::State<'_, AppDb>` 取得连接，lock 后调 impl 函数。
/// 开库与 `app.manage(...)` 注册由 S04 启动管道负责，本模块只声明类型。
pub struct AppDb(pub std::sync::Mutex<rusqlite::Connection>);
