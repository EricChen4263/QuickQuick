//! IPC 命令模块
//!
//! 定义 Tauri 托管状态类型 `AppDb` 与所有命令子模块。
//! 命令注册到 `invoke_handler` 由 S04 启动管道负责，本模块只声明类型与命令函数。
//!
//! 子模块：
//! - `clipboard`：剪贴板取数/管理命令（list/delete/toggle_favorite）
//! - `translate`：翻译命令（translate_text / list_translate_history）
//! - `settings`：设置命令（get/set_hotkeys / exclude_list / translate_providers）

pub mod clipboard;
pub mod settings;
pub mod translate;

/// Tauri 托管状态：持有可选数据库连接（Mutex 包裹保证跨命令线程安全）。
///
/// 持有 `Option<Connection>` 而非裸 `Connection`，使 `setup_app_db` 无论开库成功与否
/// 都能调用 `app.manage(AppDb(...))`，避免 Tauri dispatch 层因状态未注册而 panic。
/// 开库成功放 `Some(conn)`，失败放 `None`；命令层通过 `with_db` 统一处理 None 情况。
///
/// 开库与 `app.manage(...)` 注册由 S04 启动管道负责，本模块只声明类型。
pub struct AppDb(pub std::sync::Mutex<Option<rusqlite::Connection>>);

/// 取出受管连接并执行闭包；数据库不可用时返回统一错误，不 panic。
///
/// 设计意图：把"lock → Option 解包 → 调闭包"三步封装为一处，
/// 5 个依赖 AppDb 的命令通过此函数统一处理 None 场景，避免重复 match。
///
/// # Errors
/// - Mutex 中毒（极罕见）：返回锁错误描述
/// - 连接为 None（开库失败）：返回"数据库不可用，请检查钥匙串授权或重启应用"
/// - 闭包自身失败：透传闭包返回的 `Err`
pub fn with_db<T>(
    db: &AppDb,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String> {
    let guard = db.0.lock().map_err(|e| format!("锁获取失败: {e}"))?;
    let conn = guard
        .as_ref()
        .ok_or_else(|| "数据库不可用，请检查钥匙串授权或重启应用".to_string())?;
    f(conn)
}
