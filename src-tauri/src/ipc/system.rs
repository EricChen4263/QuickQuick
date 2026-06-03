//! 系统域 IPC 命令层（里程碑 3 · V5-F-system）
//!
//! 命令清单（前端通过 invoke 对应命令名调用）：
//! - `get_storage_stats`           — 存储统计（活跃条目数 + 库文件大小）
//! - `cleanup_history`             — 清理历史（容量裁剪 + GC 物理删除）
//! - `open_accessibility_settings` — 打开 macOS 辅助功能系统设置深链
//! - `paste_to_front`              — 将指定条目写回系统剪贴板，trusted 时走完整粘贴路径
//!
//! paste_to_front 行为（V5-F4-S04-9b）：
//! - trusted=true：写回剪贴板 → 隐藏 clip-popover/main 窗口 → hide app 让出前台
//!   → 固定等待 FOCUS_YIELD_WAIT_MS → Cmd+V 注入 → 返回 "full_paste"
//! - trusted=false 或超时：仅写回剪贴板 → 返回 "write_back_only"
//! - 图片条目：拒绝，返回错误
//!
//! 焦点编排（FocusStep 序列）：
//! - HidePanel：hide clip-popover（或 main）
//! - HideApp：app.hide() 隐藏整个 QuickQuick 进程，macOS 自动把前台还给上一个 app
//! - WaitForeground：固定等待 FOCUS_YIELD_WAIT_MS，让焦点转移自然完成后再 send_paste
//! - SimulatePaste：send_paste(Cmd+V)

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::{Manager, State};

use crate::clipboard::CapturedItem;
use crate::db::{self, DbError};
use crate::ipc::{with_db, AppDb};
use crate::onboarding::{
    perform_paste_or_degrade, AccessibilityProbe, PasteOutcome, ACCESSIBILITY_DEEPLINK,
};
use crate::paste::{self, PasteBackend};

/// 存储统计 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatsDto {
    /// 活跃（未软删）条目总数
    pub live_count: i64,
    /// 数据库文件大小（字节），文件不存在时为 0
    pub file_size_bytes: u64,
}

/// 历史清理结果 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResultDto {
    /// 本轮软删条目数（超出保留上限的旧条目）
    pub soft_deleted: usize,
    /// 物理删除的墓碑行数
    pub purged: u64,
}

/// 粘贴结果 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteResultDto {
    /// 粘贴路径："full_paste" | "write_back_only"
    pub outcome: String,
}

/// 保留条目上限：超过此数量的非收藏旧条目将被软删。
const KEEP_RECENT_COUNT: usize = 500;

/// app.hide() 后等待焦点转移完成的固定时长（毫秒）。
///
/// AppKit 的前台/激活状态在非主线程（paste 命令线程）读取是 stale 的，
/// 无法可靠轮询 NSWorkspace.frontmostApplication()——实测该 API 在命令线程始终返回
/// QuickQuick 自身 pid，轮询条件永不成立。
/// app.hide() 异步完成焦点转移约需 250-350ms，固定等待是此约束下的务实选择。
/// 100ms 实测过短致 Cmd+V 落空，800ms 可靠但用户感知明显卡顿，350ms 为平衡点。
const FOCUS_YIELD_WAIT_MS: u64 = 350;

/// get_storage_stats 的纯函数实现，可在测试中直接调用。
///
/// 返回活跃条目数与 db 文件大小。`db_path` 为 None 时文件大小返回 0。
///
/// # Errors
/// 数据库查询失败时返回 `DbError`。
pub fn get_storage_stats_impl(
    conn: &Connection,
    db_path: Option<&std::path::Path>,
) -> Result<StorageStatsDto, DbError> {
    let live_count = db::count_live(conn)?;
    let file_size_bytes = db_path
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(StorageStatsDto {
        live_count,
        file_size_bytes,
    })
}

/// cleanup_history 的纯函数实现，可在测试中直接调用。
///
/// 两步清理：先软删超出 `KEEP_RECENT_COUNT` 的旧条目，再物理删除所有墓碑行。
///
/// # Errors
/// 数据库操作失败时返回 `DbError`。
pub fn cleanup_history_impl(conn: &Connection) -> Result<CleanupResultDto, DbError> {
    let soft_deleted = db::cleanup_keep_recent(conn, KEEP_RECENT_COUNT)?;
    let purged = db::gc_purge_deleted(conn)?;
    Ok(CleanupResultDto {
        soft_deleted,
        purged,
    })
}

/// 从 DB 按 id 取条目内容，校验类型，构造 CapturedItem。
///
/// 图片条目（kind="image"）返回错误（arboard 图片格式转换留后续）。
/// id 为空或条目不存在时返回错误。
///
/// # Errors
/// - id 为空
/// - 条目不存在或已软删
/// - 条目类型为 "image"
pub fn fetch_paste_item(conn: &Connection, id: &str) -> Result<CapturedItem, DbError> {
    if id.trim().is_empty() {
        return Err(DbError::Other("id 不能为空".to_string()));
    }

    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT content, kind FROM clip_items WHERE id = ?1 AND is_deleted = 0",
            rusqlite::params![id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(DbError::Sqlite)?;

    let (content, kind) = row.ok_or_else(|| DbError::Other("条目不存在或已删除".to_string()))?;

    if kind == "image" {
        return Err(DbError::Other("图片条目暂不支持写回剪贴板".to_string()));
    }

    Ok(CapturedItem {
        text: content,
        html: None,
    })
}

/// 将 PasteOutcome 映射为前端 outcome 字符串（纯函数，可测）。
///
/// - `FullPasteDone`      → "full_paste"
/// - `WriteBackOnlyDone`  → "write_back_only"
pub fn map_outcome(outcome: &PasteOutcome) -> &'static str {
    match outcome {
        PasteOutcome::FullPasteDone => "full_paste",
        PasteOutcome::WriteBackOnlyDone => "write_back_only",
    }
}

/// 编排粘贴逻辑：调用 perform_paste_or_degrade，将结果映射为 outcome 字符串。
///
/// 此函数不含窗口 hide 或 sleep（OS 边界操作），仅封装可测的"探针决策 → 执行 → 映射"。
/// 窗口 hide 与延时由调用方（paste_to_front 命令）在 trusted 分支的 send_paste 前执行。
///
/// 超时（PasteError::Timeout）时：剪贴板已写入，映射为 "write_back_only"，不向上传播错误。
pub fn paste_orchestrate(
    probe: &impl AccessibilityProbe,
    backend: &mut dyn PasteBackend,
    item: &CapturedItem,
) -> String {
    match perform_paste_or_degrade(probe, backend, item) {
        Ok(outcome) => map_outcome(&outcome).to_string(),
        Err(_timeout) => "write_back_only".to_string(),
    }
}

/// Tauri 命令：取存储统计（活跃条目数 + 库文件大小）。
#[tauri::command]
pub fn get_storage_stats(
    state: State<'_, AppDb>,
    app: tauri::AppHandle,
) -> Result<StorageStatsDto, String> {
    let db_path = app
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("quickquick.db"));

    with_db(&state, |conn| {
        get_storage_stats_impl(conn, db_path.as_deref()).map_err(|e| e.to_string())
    })
}

/// Tauri 命令：清理历史（容量裁剪 + GC）。
#[tauri::command]
pub fn cleanup_history(state: State<'_, AppDb>) -> Result<CleanupResultDto, String> {
    with_db(&state, |conn| {
        cleanup_history_impl(conn).map_err(|e| e.to_string())
    })
}

/// Tauri 命令：打开 macOS 辅助功能系统设置深链。
///
/// 使用 `std::process::Command("open")` 打开深链 URL（项目未依赖 tauri-plugin-opener，
/// Cargo.toml 中无此依赖，改用系统 `open` 命令直接打开）。
#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    std::process::Command::new("open")
        .arg(ACCESSIBILITY_DEEPLINK)
        .spawn()
        .map_err(|e| format!("无法打开辅助功能设置：{e}"))?;
    Ok(())
}

/// 在 trusted 路径执行窗口 hide，让出前台，固定等待焦点转移完成（WaitForeground）。
///
/// 三步顺序（FocusStep 序列的后半段）：
/// 1. HidePanel：hide clip-popover（或 main）
/// 2. HideApp：app.hide() 隐藏整个 QuickQuick 进程，macOS 自动把前台还给上一个 app
/// 3. WaitForeground：固定等待 FOCUS_YIELD_WAIT_MS，让焦点转移自然完成
///
/// 为何用固定等待而非条件轮询：
/// AppKit 前台/激活状态在非主线程读取是 stale 的，NSWorkspace.frontmostApplication()
/// 在 paste 命令线程始终返回 QuickQuick 自身 pid，轮询条件永不成立（实测每次跑满上限）。
/// 固定等待是此约束下的务实选择，详见 FOCUS_YIELD_WAIT_MS 注释。
///
/// hide 失败均不 panic，降级记录日志继续尝试粘贴。
fn hide_and_restore_focus(app: &tauri::AppHandle) {
    hide_popover_window(app);
    hide_app(app);
    std::thread::sleep(std::time::Duration::from_millis(FOCUS_YIELD_WAIT_MS));
}

/// 优先 hide clip-popover，不存在则 hide main。
fn hide_popover_window(app: &tauri::AppHandle) {
    let hidden = app
        .get_webview_window("clip-popover")
        .map(|w| {
            if let Err(e) = w.hide() {
                eprintln!("[paste_to_front] hide clip-popover 失败: {e}");
                false
            } else {
                true
            }
        })
        .unwrap_or(false);

    if !hidden {
        if let Some(main_win) = app.get_webview_window("main") {
            if let Err(e) = main_win.hide() {
                eprintln!("[paste_to_front] hide main 失败: {e}");
            }
        }
    }
}

/// 隐藏整个 QuickQuick 进程，macOS 会自动把前台还给上一个 app。
///
/// 使用 AppHandle::hide()（macOS 专有 API）而非直接调用 NSApp.hide()：
/// paste_to_front 命令跑在非主线程，AppHandle::hide() 内部负责线程派发，安全可用。
/// 失败时降级记录日志，不 panic。
#[cfg(target_os = "macos")]
fn hide_app(app: &tauri::AppHandle) {
    if let Err(e) = app.hide() {
        eprintln!("[paste_to_front] hide app 失败: {e}");
    }
}

/// 非 macOS 平台：空实现，编译时零开销消除。
#[cfg(not(target_os = "macos"))]
fn hide_app(_app: &tauri::AppHandle) {}

/// Tauri 命令：将指定条目写回系统剪贴板，trusted 时走完整粘贴路径（V5-F4-S04-9b）。
///
/// trusted=true 分支：写回 → hide 窗口 → 等待 100ms → Cmd+V → 返回 "full_paste"。
/// trusted=false 或超时：仅写回 → 返回 "write_back_only"。
/// 图片条目返回 Err。
#[tauri::command]
pub fn paste_to_front(
    state: State<'_, AppDb>,
    app: tauri::AppHandle,
    id: String,
) -> Result<PasteResultDto, String> {
    let item = with_db(&state, |conn| {
        fetch_paste_item(conn, &id).map_err(|e| e.to_string())
    })?;

    let outcome = run_paste_with_backend(&app, &item);

    Ok(PasteResultDto { outcome })
}

/// 构造平台后端/probe，执行粘贴编排（含 trusted 分支的窗口 hide）。
///
/// 拆出独立函数以降低 paste_to_front 函数体长度，cfg 分流在此隔离。
///
/// trusted 分支：write_and_confirm 确认写入 → hide 窗口 → send_paste → "full_paste"。
/// write_and_confirm 失败（Timeout）或未授权：write_with_marker → "write_back_only"。
#[cfg(target_os = "macos")]
fn run_paste_with_backend(app: &tauri::AppHandle, item: &CapturedItem) -> String {
    use crate::macos_paste::{MacOsAccessibilityProbe, MacOsPasteBackend};

    let probe = MacOsAccessibilityProbe;
    let mut backend = match MacOsPasteBackend::new() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[run_paste_with_backend] 后端初始化失败: {e}");
            return "write_back_only".to_string();
        }
    };

    if probe.is_trusted() {
        match paste::write_and_confirm(&mut backend, item) {
            Ok(()) => {
                hide_and_restore_focus(app);
                backend.send_paste();
                "full_paste".to_string()
            }
            Err(_) => "write_back_only".to_string(),
        }
    } else {
        backend.write_with_marker(item);
        "write_back_only".to_string()
    }
}

#[cfg(not(target_os = "macos"))]
fn run_paste_with_backend(app: &tauri::AppHandle, item: &CapturedItem) -> String {
    use crate::macos_paste::FallbackAccessibilityProbe;
    use crate::macos_paste::FallbackPasteBackend;

    let probe = FallbackAccessibilityProbe;
    let mut backend = match FallbackPasteBackend::new() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[run_paste_with_backend] 后端初始化失败: {e}");
            return "write_back_only".to_string();
        }
    };
    let _ = app;

    paste_orchestrate(&probe, &mut backend, item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::CapturedItem;
    use crate::db::ingest;
    use crate::onboarding::AccessibilityProbe;
    use crate::paste::PasteBackend;

    fn make_test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("内存库开启失败");
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS clip_items (
                 id                TEXT PRIMARY KEY NOT NULL,
                 content           TEXT,
                 kind              TEXT NOT NULL DEFAULT 'text',
                 created_utc       INTEGER NOT NULL,
                 last_modified_utc INTEGER NOT NULL,
                 is_deleted        INTEGER NOT NULL DEFAULT 0,
                 deleted_at_utc    INTEGER,
                 text_hash         TEXT,
                 is_favorite       INTEGER NOT NULL DEFAULT 0
             );",
        )
        .expect("建测试表失败");
        conn
    }

    struct FakeProbe {
        trusted: bool,
    }
    impl AccessibilityProbe for FakeProbe {
        fn is_trusted(&self) -> bool {
            self.trusted
        }
    }

    struct FakeBackend {
        count: u64,
        text: Option<String>,
        send_paste_called: bool,
        freeze_count: bool,
    }
    impl FakeBackend {
        fn trusted_normal() -> Self {
            Self {
                count: 0,
                text: None,
                send_paste_called: false,
                freeze_count: false,
            }
        }
        fn frozen_count() -> Self {
            Self {
                count: 0,
                text: None,
                send_paste_called: false,
                freeze_count: true,
            }
        }
    }
    impl PasteBackend for FakeBackend {
        fn change_count(&self) -> u64 {
            self.count
        }
        fn write_with_marker(&mut self, item: &CapturedItem) {
            self.text = Some(item.text.clone());
            if !self.freeze_count {
                self.count += 1;
            }
        }
        fn current_text(&self) -> Option<String> {
            self.text.clone()
        }
        fn send_paste(&mut self) {
            self.send_paste_called = true;
        }
    }

    fn test_item(text: &str) -> CapturedItem {
        CapturedItem {
            text: text.to_string(),
            html: None,
        }
    }

    /// T1：get_storage_stats_impl — 空库返回 live_count=0，file_size_bytes=0（无路径）
    #[test]
    fn get_storage_stats_impl_empty_db_returns_zero() {
        let conn = make_test_conn();
        let stats = get_storage_stats_impl(&conn, None).expect("不应失败");
        assert_eq!(stats.live_count, 0, "空库活跃数应为 0");
        assert_eq!(stats.file_size_bytes, 0, "无路径时文件大小应为 0");
    }

    /// T2：get_storage_stats_impl — 写入 2 条后 live_count=2
    #[test]
    fn get_storage_stats_impl_counts_live_items() {
        let conn = make_test_conn();
        let item_a = CapturedItem {
            text: "hello".to_string(),
            html: None,
        };
        let item_b = CapturedItem {
            text: "world".to_string(),
            html: None,
        };
        ingest(&conn, &item_a).expect("入库 A 失败");
        ingest(&conn, &item_b).expect("入库 B 失败");

        let stats = get_storage_stats_impl(&conn, None).expect("不应失败");
        assert_eq!(stats.live_count, 2, "写入 2 条后 live_count 应为 2");
    }

    /// T3：cleanup_history_impl — 无超限时 soft_deleted=0，purged=0
    #[test]
    fn cleanup_history_impl_no_excess_returns_zeros() {
        let conn = make_test_conn();
        let item = CapturedItem {
            text: "only one".to_string(),
            html: None,
        };
        ingest(&conn, &item).expect("入库失败");

        let result = cleanup_history_impl(&conn).expect("不应失败");
        assert_eq!(result.soft_deleted, 0, "未超限时 soft_deleted 应为 0");
        assert_eq!(result.purged, 0, "无墓碑时 purged 应为 0");
    }

    /// T4：fetch_paste_item — 空 id 应返回 Err
    #[test]
    fn fetch_paste_item_empty_id_returns_err() {
        let conn = make_test_conn();
        let result = fetch_paste_item(&conn, "");
        assert!(result.is_err(), "空 id 应返回错误");
    }

    /// T5：fetch_paste_item — 不存在的 id 应返回 Err
    #[test]
    fn fetch_paste_item_nonexistent_id_returns_err() {
        let conn = make_test_conn();
        let result = fetch_paste_item(&conn, "nonexistent-uuid");
        assert!(result.is_err(), "不存在的 id 应返回错误");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("不存在") || msg.contains("条目"),
            "错误信息应说明条目不存在，实际：{msg}"
        );
    }

    /// T6：map_outcome — FullPasteDone 映射 "full_paste"
    #[test]
    fn map_outcome_full_paste_done_returns_full_paste() {
        assert_eq!(map_outcome(&PasteOutcome::FullPasteDone), "full_paste");
    }

    /// T7：map_outcome — WriteBackOnlyDone 映射 "write_back_only"
    #[test]
    fn map_outcome_write_back_only_done_returns_write_back_only() {
        assert_eq!(
            map_outcome(&PasteOutcome::WriteBackOnlyDone),
            "write_back_only"
        );
    }

    /// T8：paste_orchestrate — trusted=true 正常后端 → "full_paste"，send_paste 被调用
    #[test]
    fn paste_orchestrate_trusted_normal_returns_full_paste() {
        let probe = FakeProbe { trusted: true };
        let mut backend = FakeBackend::trusted_normal();
        let item = test_item("hello world");

        let outcome = paste_orchestrate(&probe, &mut backend, &item);

        assert_eq!(outcome, "full_paste", "trusted 正常路径应返回 full_paste");
        assert!(backend.send_paste_called, "trusted 路径应调用 send_paste");
    }

    /// T9：paste_orchestrate — trusted=false → "write_back_only"，send_paste 未被调用
    #[test]
    fn paste_orchestrate_untrusted_returns_write_back_only() {
        let probe = FakeProbe { trusted: false };
        let mut backend = FakeBackend::trusted_normal();
        let item = test_item("hello world");

        let outcome = paste_orchestrate(&probe, &mut backend, &item);

        assert_eq!(
            outcome, "write_back_only",
            "未授权路径应返回 write_back_only"
        );
        assert!(!backend.send_paste_called, "未授权路径不应调用 send_paste");
    }

    /// T10：paste_orchestrate — trusted=true 但 changeCount 冻结（超时）→ "write_back_only"，不崩
    #[test]
    fn paste_orchestrate_trusted_timeout_returns_write_back_only() {
        let probe = FakeProbe { trusted: true };
        let mut backend = FakeBackend::frozen_count();
        let item = test_item("hello world");

        let outcome = paste_orchestrate(&probe, &mut backend, &item);

        assert_eq!(outcome, "write_back_only", "超时应映射 write_back_only");
        assert!(!backend.send_paste_called, "超时不应调用 send_paste");
    }
}
