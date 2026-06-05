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
    use super::should_check;

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
