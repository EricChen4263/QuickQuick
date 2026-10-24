//! IPC 命令模块
//!
//! 定义 Tauri 托管状态类型 `AppDb` 与所有命令子模块。
//! 命令注册到 `invoke_handler` 由 S04 启动管道负责，本模块只声明类型与命令函数。
//!
//! 子模块：
//! - `clipboard`：剪贴板取数/管理命令（list/delete/toggle_favorite）
//! - `translate`：翻译命令（translate_text / list_translate_history）
//! - `settings`：设置命令（get/set_hotkeys / exclude_list / translate_providers）
//! - `portable`：备份/恢复命令（export_backup / import_backup / reveal_backup_in_finder）

pub mod clipboard;
pub mod portable;
pub mod settings;
pub mod system;
pub mod translate;
pub mod update;

/// Tauri 托管状态：可选数据库连接 + 启动期开库失败原因（Mutex 包裹保证跨命令线程安全）。
///
/// 持有 `Option<Connection>` 而非裸 `Connection`，使 `setup_app_db` 无论开库成功与否
/// 都能注册状态，避免 Tauri dispatch 层因状态未注册而 panic；命令层通过 `with_db`
/// 统一处理不可用情况。
///
/// `open_error` 存的是启动期失败原因的**用户可读文案**（即 `db::DbError` 的 `Display`）。
/// 为什么必须存进状态里：打包后的 GUI 应用没有用户可见的 stderr，`eprintln` 只服务开发；
/// 用户能看到的唯一出口就是前端收到的 IPC 错误文案，不带上原因等于什么都没说，
/// 而替用户猜一个病因（例如「请检查钥匙串授权」）会把人引向错误的自救路径。
///
/// 开库与注册由 S04 启动管道负责，本模块只声明类型。
pub struct AppDb {
    /// 打开成功的连接；启动期开库失败时为 `None`
    pub conn: std::sync::Mutex<Option<rusqlite::Connection>>,
    /// 启动期开库失败的原因（用户可读文案）；开库成功时为 `None`
    pub open_error: Option<String>,
}

/// Tauri 托管状态：本地 ECDICT 词典 DAO（只读、`Arc` 共享、无需 `Mutex`）。
///
/// `EcdictDb` 内部每次 `lookup` 临时只读开库、无可变状态，故用 `Arc` 直接跨命令
/// 共享即可（与 `AppDb` 需 `Mutex` 包裹可变连接不同）。库文件缺失时 `lookup`
/// 返回错误而非 panic，因此即便资源未打包，状态仍可安全注册。
pub struct EcdictDbState(pub std::sync::Arc<crate::translate::ecdict_db::EcdictDb>);

/// 取出受管连接并执行闭包；数据库不可用时返回**带具体原因**的错误，不 panic。
///
/// 设计意图：把"lock → Option 解包 → 调闭包"三步封装为一处，
/// 5 个依赖 AppDb 的命令通过此函数统一处理不可用场景，避免重复 match。
///
/// # Errors
/// - Mutex 中毒（极罕见）：返回锁错误描述
/// - 连接为 None（启动期开库失败）：返回「数据库不可用：<启动期记录的原因>」
/// - 闭包自身失败：透传闭包返回的 `Err`
pub fn with_db<T>(
    db: &AppDb,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String> {
    let guard = db.conn.lock().map_err(|e| format!("锁获取失败: {e}"))?;
    let conn = guard
        .as_ref()
        .ok_or_else(|| unavailable_message(db.open_error.as_deref()))?;
    f(conn)
}

/// 拼「数据库不可用」的前端文案：有启动期原因就原样带上，没有才退回通用指引。
///
/// 原因来自 `db::DbError` 的 `Display`，本身就是给用户看的中文，并且已各自带着行动指引
/// （版本超前是「请更新应用」，迁移失败是「请联系支持」），所以这里不再追加通用尾巴，
/// 否则会出现两遍「请联系支持」这种噪音。
///
/// 只有状态里没记录原因时才走兜底分支（当前启动流程不会出现：连接与原因总是成对写入，
/// 属防御分支）。兜底文案必须仍然可行动，但**不能**替用户指定某一种病因。
fn unavailable_message(reason: Option<&str>) -> String {
    match reason {
        Some(reason) => format!("数据库不可用：{reason}"),
        None => "数据库不可用：本次启动未能打开本机数据库。请确认应用已是最新版本并重启；\
                 若仍然失败，请联系支持并提供日志。"
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbError;

    /// 没有原因时退回通用指引：不 panic，且仍给出可行动下一步。
    #[test]
    fn with_db_unavailable_without_reason_uses_fallback_guidance() {
        let state = AppDb {
            conn: std::sync::Mutex::new(None),
            open_error: None,
        };

        let msg = with_db(&state, |_conn| Ok(1u8)).expect_err("无连接时必须返回 Err");

        assert!(
            msg.starts_with("数据库不可用："),
            "前端文案前缀应稳定，实际: {msg}"
        );
        assert!(
            msg.contains("重启") && msg.contains("请联系支持"),
            "兜底文案必须给出可行动的下一步，实际: {msg}"
        );
    }

    /// 有启动期原因时，前端文案必须把那句原因带上（用户看不到 stderr）。
    #[test]
    fn with_db_unavailable_with_reason_echoes_the_reason() {
        // 版本超前：必须带出「库比程序新」与两个版本号
        let too_new = DbError::SchemaTooNew {
            found: 2,
            supported: 1,
        }
        .to_string();
        let state = AppDb {
            conn: std::sync::Mutex::new(None),
            open_error: Some(too_new.clone()),
        };

        let msg = with_db(&state, |_conn| Ok(1u8)).expect_err("无连接时必须返回 Err");

        assert!(
            msg.contains(&too_new),
            "文案必须原样带上启动期原因，实际: {msg}"
        );
        assert!(
            msg.contains("更新版本的 QuickQuick") && msg.contains("v2 > v1"),
            "版本超前原因应完整可见，实际: {msg}"
        );

        // 迁移失败：必须带出 from→to 与底层原因
        let failed = DbError::MigrationFailed {
            to_version: 2,
            from_version: 1,
            reason: "no such column: title".to_string(),
        }
        .to_string();
        let state = AppDb {
            conn: std::sync::Mutex::new(None),
            open_error: Some(failed.clone()),
        };

        let msg = with_db(&state, |_conn| Ok(1u8)).expect_err("无连接时必须返回 Err");

        assert!(
            msg.contains(&failed),
            "文案必须原样带上启动期原因，实际: {msg}"
        );
        assert!(
            msg.contains("1→2") && msg.contains("no such column: title"),
            "迁移失败原因应完整可见（版本对 + 底层原因），实际: {msg}"
        );
    }

    /// 有连接时闭包照常执行（状态加了原因字段不得影响主路径）。
    #[test]
    fn with_db_connected_runs_closure() {
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        let state = AppDb {
            conn: std::sync::Mutex::new(Some(conn)),
            open_error: None,
        };

        let value = with_db(&state, |c| {
            c.query_row("SELECT 42", [], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())
        })
        .expect("有连接时 with_db 应成功");

        assert_eq!(value, 42, "闭包应真正作用在受管连接上");
    }
}
