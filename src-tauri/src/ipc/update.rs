//! 更新域 IPC 命令层
//!
//! 命令清单（前端通过 invoke 对应命令名调用）：
//! - `check_for_updates` — 手动检查是否有新版本（返回有无 + 版本号）
//!
//! 设计说明：
//! - endpoint 为真实地址（github.com/EricChen4263/QuickQuick/…，CI 已产签名 latest.json），
//!   除前端手动触发 `check_for_updates` 外，启动后由 lib.rs setup 中的 `update_watcher`
//!   后台任务定期自动检查（首检延迟 + 长间隔轮询）；本轮是否真正发起检查由纯函数
//!   `should_check` 判定。
//! - updater() 返回 Err 时友好映射为中文错误，前端可直接展示给用户；后台任务遇错仅记录、不 panic。

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tauri_plugin_updater::{Update, UpdaterExt};

/// 检查更新结果 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckUpdateResult {
    /// 是否有可用新版本
    pub available: bool,
    /// 新版本号（有更新时），无更新时为空串
    pub version: String,
    /// 当前已安装版本号
    pub current_version: String,
}

/// 检查是否有可用的应用更新。
///
/// 通过 tauri-plugin-updater 向配置的 endpoint 查询（endpoint 已是真实地址，
/// 见模块顶部说明）。网络不可达或版本清单解析失败时返回 Err，前端以友好文案展示。
///
/// # Errors
/// - updater 插件初始化失败（极罕见）：返回"updater 初始化失败: …"
/// - 网络请求或版本清单解析失败：返回"检查更新失败：…"
#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<CheckUpdateResult, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("updater 初始化失败: {e}"))?;

    match updater.check().await {
        Ok(Some(update)) => Ok(CheckUpdateResult {
            available: true,
            version: update.version.clone(),
            current_version: update.current_version.clone(),
        }),
        Ok(None) => {
            let current = app.package_info().version.to_string();
            Ok(CheckUpdateResult {
                available: false,
                version: String::new(),
                current_version: current,
            })
        }
        Err(e) => Err(format!("检查更新失败：{e}")),
    }
}

/// 更新就绪事件名：下载安装完成、等待用户重启时向前端广播。
///
/// 前端 `listen(UPDATE_READY_EVENT)` 据此渲染重启提示条（payload 携带新版本号）。
pub const UPDATE_READY_EVENT: &str = "update://ready";

/// 更新就绪事件的 payload。
///
/// `Clone` 供 emit 跨 await 复制；`Serialize` 供 Tauri 序列化给前端。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReadyPayload {
    /// 已下载安装、等待重启生效的新版本号
    pub version: String,
}

/// 由新版本号构造就绪事件 payload。
///
/// 抽成纯函数以便单测——下载/emit 的真实链路无法在单测构造 `Update`。
pub fn build_ready_payload(version: &str) -> UpdateReadyPayload {
    UpdateReadyPayload {
        version: version.to_string(),
    }
}

/// 下载并安装一个已确认可用的更新，成功后广播就绪事件并置位去重标志。
///
/// 薄封装层：把不可单测的真实下载（`download_and_install`）与 I/O 隔离在此处，
/// 后台 watcher 与手动命令共用本函数（DRY），避免两份下载逻辑。
/// 静默下载：`on_chunk`/`on_finish` 仅空实现（本版不做前端进度，留极简）。
///
/// 失败语义：仅 `eprintln!` 记录、不 panic、不置位 `already_ready`（留待下轮/手动重试）。
/// 返回 `Result` 供手动路径把错误反馈前端；后台路径忽略返回值即静默。
///
/// # Errors
/// - 下载或安装失败：返回"下载更新失败：…"
async fn download_install_and_notify(
    app: &tauri::AppHandle,
    update: Update,
    already_ready: &Arc<AtomicBool>,
) -> Result<(), String> {
    let version = update.version.clone();
    if let Err(e) = update.download_and_install(|_chunk, _total| {}, || {}).await {
        eprintln!("update: 下载更新 {version} 失败：{e}");
        return Err(format!("下载更新失败：{e}"));
    }
    if let Err(e) = app.emit(UPDATE_READY_EVENT, build_ready_payload(&version)) {
        // emit 失败不影响"已安装"事实，仅记录；仍视为就绪以免重复下载。
        eprintln!("update: 广播就绪事件失败：{e}");
    }
    already_ready.store(true, Ordering::Relaxed);
    Ok(())
}

/// 后台 watcher 调用入口：检查到可用更新后走下载安装薄封装，忽略错误（静默重试）。
///
/// 与手动命令复用同一薄封装，区别仅在错误处理（此处静默、命令路径回传前端）。
pub async fn download_install_for_watcher(
    app: &tauri::AppHandle,
    update: Update,
    already_ready: &Arc<AtomicBool>,
) {
    let _ = download_install_and_notify(app, update, already_ready).await;
}

/// 手动触发下载并安装更新（供前端「下载并安装」入口调用）。
///
/// 内部先 `check()`：有可用更新则走下载安装薄封装并广播就绪事件；无更新返回 Ok。
/// 与后台 watcher 共用薄封装；区别是错误会回传给前端展示（后台则静默重试）。
///
/// # Errors
/// - updater 初始化失败：返回"updater 初始化失败: …"
/// - 检查更新失败：返回"检查更新失败：…"
/// - 下载或安装失败：返回"下载更新失败：…"
#[tauri::command]
pub async fn download_and_install_update(app: tauri::AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("updater 初始化失败: {e}"))?;
    match updater.check().await {
        Ok(Some(update)) => {
            // 手动入口用独立去重标志：仅约束本次调用的成功置位语义，
            // 后台 watcher 的去重标志由 lib.rs 跨轮持有，二者互不干扰。
            let already_ready = Arc::new(AtomicBool::new(false));
            download_install_and_notify(&app, update, &already_ready).await
        }
        Ok(None) => Ok(()),
        Err(e) => Err(format!("检查更新失败：{e}")),
    }
}

/// 重启应用以让已安装的新版本生效（供前端「立即重启」入口调用）。
///
/// 用 Tauri 核心的 `AppHandle::restart()` 而非 `tauri-plugin-process`：
/// restart 是框架内置 API，从 Rust 命令内部直接调用，无需额外 plugin 依赖、
/// 也无需在 capabilities 开放 `process:*` 权限（`core:default` 已足够）。
/// `restart()` 会替换当前进程并以新二进制重新拉起，实际永不返回，
/// 故无前端回执、本命令本身也无法单测——真机重启验证归 manual_confirm。
///
/// 签名声明 `()` 而非 `!`：`#[tauri::command]` 宏需要一个可序列化的具体回执类型，
/// 给它 `!` 会触发 E0282 类型推断失败；`restart()` 的 `!` 可强制转为 `()`，
/// 既满足宏、又如实表达"正常路径下根本走不到返回"。
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    app.restart()
}

/// 判定后台 watcher 本轮是否应发起更新检查。
///
/// 语义：`auto_update_enabled && !already_ready`。
/// - `auto_update_enabled`：用户的自动更新开关；false 时 watcher 完全跳过检查/下载/提示。
/// - `already_ready`：本进程内是否已有一次更新就绪。置位后不再重复检查，去重避免同一
///   可用版本被反复发现与下载。
///
/// 抽成纯函数以便单测——真实的 `updater().check()` 无法在单测构造 `Update`。
pub fn should_check(auto_update_enabled: bool, already_ready: bool) -> bool {
    auto_update_enabled && !already_ready
}

#[cfg(test)]
mod tests {
    use super::{build_ready_payload, should_check};

    #[test]
    fn update_ready_payload_carries_version() {
        assert_eq!(build_ready_payload("1.2.3").version, "1.2.3");
    }

    #[test]
    fn update_watcher_should_check_when_enabled() {
        assert!(should_check(true, false));
    }

    #[test]
    fn update_watcher_should_skip_when_disabled() {
        assert!(!should_check(false, false));
        assert!(!should_check(false, true));
    }

    #[test]
    fn update_watcher_dedupes_after_ready() {
        assert!(!should_check(true, true));
    }
}
