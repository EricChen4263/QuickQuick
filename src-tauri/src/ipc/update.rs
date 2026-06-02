//! 更新域 IPC 命令层
//!
//! 命令清单（前端通过 invoke 对应命令名调用）：
//! - `check_for_updates` — 手动检查是否有新版本（返回有无 + 版本号）
//!
//! 设计说明：
//! - 不在 setup 阶段自动调用，原因：当前 tauri.conf.json 使用占位 endpoint，
//!   自动检查会在每次启动时产生网络错误噪音，待真实 infra 就绪前仅供 UI 手动触发。
//! - updater() 返回 Err 时友好映射为中文错误，前端可直接展示给用户。

use serde::Serialize;
use tauri_plugin_updater::UpdaterExt;

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
/// 通过 tauri-plugin-updater 向配置的 endpoint 查询。
/// endpoint 为占位地址时会返回网络/解析错误，前端应以友好文案展示。
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
