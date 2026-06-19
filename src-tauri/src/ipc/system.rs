//! 系统域 IPC 命令层（里程碑 3 · V5-F-system）
//!
//! 命令清单（前端通过 invoke 对应命令名调用）：
//! - `get_storage_stats`           — 存储统计（活跃条目数 + 库文件大小）
//! - `cleanup_history`             — 清理历史（容量裁剪 + GC 物理删除）
//! - `open_accessibility_settings` — 打开 macOS 辅助功能系统设置深链
//! - `paste_to_front`              — 将指定条目写回系统剪贴板，trusted 时走完整粘贴路径
//! - `copy_clip_to_clipboard`      — 按 id 把条目（富文本带 html）写入系统剪贴板（仅写回，不注入粘贴）
//!
//! paste_to_front 行为（V5-F4-S04-9b）：
//! - trusted=true：写回剪贴板 → 隐藏 clip-popover/main 窗口 → hide app 让出前台
//!   → 固定等待 FOCUS_YIELD_WAIT_MS → Cmd+V 注入 → 返回 "full_paste"
//! - trusted=false 或超时：仅写回剪贴板 → 返回 "write_back_only"
//! - 图片条目：取原图 PNG 解码为 RGBA → set_image 写回；trusted 时同样走完整粘贴路径，
//!   原图已不可用（降级剥离）时返回错误
//!
//! 焦点编排（FocusStep 序列）：
//! - HidePanel：hide clip-popover（或 main）
//! - HideApp：app.hide() 隐藏整个 QuickQuick 进程，macOS 自动把前台还给上一个 app
//! - WaitForeground：固定等待 FOCUS_YIELD_WAIT_MS，让焦点转移自然完成后再 send_paste
//! - SimulatePaste：send_paste(Cmd+V)

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::{Manager, State};

use std::sync::Arc;

use crate::clipboard::{png_to_rgba, CapturedItem, ClipboardPayload};
use crate::db::{self, DbError};
use crate::frontmost::LastExternalApp;
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

/// 从 DB 按 id 取条目内容，校验类型，构造写回剪贴板的 `ClipboardPayload`。
///
/// - text/richtext 条目：组 `Text(CapturedItem{text,html})`。
/// - image 条目：按 clip_item_id 查 `clip_images` 原图 PNG BLOB，非空则 `png_to_rgba`
///   解码为 `Image{width,height,rgba}`；无原图/空 BLOB（降级剥离原图后）返回 Err，
///   语义为"图片原图已不可用，无法写回剪贴板"。
///
/// id 为空或条目不存在时返回错误。
///
/// # Errors
/// - id 为空
/// - 条目不存在或已软删
/// - image 条目原图不可用（无原图行 / 空 BLOB / PNG 解码失败）
pub fn fetch_paste_item(conn: &Connection, id: &str) -> Result<ClipboardPayload, DbError> {
    if id.trim().is_empty() {
        return Err(DbError::Other("id 不能为空".to_string()));
    }

    let row: Option<(String, String, Option<String>)> = conn
        .query_row(
            "SELECT content, kind, html_content FROM clip_items WHERE id = ?1 AND is_deleted = 0",
            rusqlite::params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(DbError::Sqlite)?;

    let (content, kind, html) =
        row.ok_or_else(|| DbError::Other("条目不存在或已删除".to_string()))?;

    if kind == "image" {
        return fetch_image_payload(conn, id);
    }

    Ok(ClipboardPayload::Text(CapturedItem {
        text: content,
        html,
    }))
}

/// 按 clip_item_id 取关联 clip_images 的原图 PNG BLOB，解码为 `Image` 载荷。
///
/// 抽出独立函数以降低 `fetch_paste_item` 的嵌套层级与长度。
/// 原图不存在、BLOB 为空（降级剥离原图）或 PNG 解码失败时返回 Err，
/// 语义统一为"图片原图已不可用，无法写回剪贴板"。
///
/// # Errors
/// 无关联原图行 / 原图 BLOB 为空 / PNG 解码失败。
fn fetch_image_payload(conn: &Connection, clip_item_id: &str) -> Result<ClipboardPayload, DbError> {
    // clip_item_id 无 UNIQUE 约束，理论上可命中多行；加确定性排序兜底取最新一行，
    // 避免同值并列时取序不定（last_modified_utc 同毫秒并列时再以 rowid DESC 稳定）。
    let png: Option<Vec<u8>> = conn
        .query_row(
            "SELECT original FROM clip_images
             WHERE clip_item_id = ?1 AND is_deleted = 0
             ORDER BY last_modified_utc DESC, rowid DESC
             LIMIT 1",
            rusqlite::params![clip_item_id],
            |row| row.get::<_, Option<Vec<u8>>>(0),
        )
        .optional()
        .map_err(DbError::Sqlite)?
        .flatten();

    let png = png.filter(|bytes| !bytes.is_empty()).ok_or_else(|| {
        DbError::Other("图片原图已不可用，无法写回剪贴板".to_string())
    })?;

    let (width, height, rgba) = png_to_rgba(&png).map_err(DbError::Other)?;
    Ok(ClipboardPayload::Image {
        width,
        height,
        rgba,
    })
}

/// 复制命令取数：与 `fetch_paste_item` 同逻辑，返回写回剪贴板的 `ClipboardPayload`。
///
/// 语义独立于复制域：复制只需内容、不涉及粘贴的 trusted/焦点编排。
/// 两者保持独立函数便于各自演进；当前复制与粘贴的取数规则一致，故委托。
///
/// # Errors
/// 同 `fetch_paste_item`。
pub fn fetch_clip_for_copy(conn: &Connection, id: &str) -> Result<ClipboardPayload, DbError> {
    fetch_paste_item(conn, id)
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

/// 编排图片粘贴：write_image → 仅在写入成功且 trusted 时 send_paste（可测纯逻辑）。
///
/// 与 `paste_orchestrate` 一样不含窗口 hide（OS 边界），仅封装"写图 → 探针决策 → 注入"。
/// 关键不变量（C1）：`write_image` 返回 Err 时**绝不** send_paste——否则会把前台旧剪贴板
/// 内容粘出去（与文本路径 `write_and_confirm` 失败即不粘对齐）。写图成功且未授权时仅写回。
///
/// 返回 "full_paste"（写图成功 + trusted + 已 send_paste）或 "write_back_only"（写图失败 / 未授权）。
pub fn paste_image_orchestrate(
    probe: &impl AccessibilityProbe,
    backend: &mut dyn PasteBackend,
    width: usize,
    height: usize,
    rgba: &[u8],
) -> String {
    if backend.write_image(width, height, rgba).is_err() {
        // 写图失败：剪贴板内容未更新，注入会粘出旧内容，故跳过 send_paste。
        return "write_back_only".to_string();
    }
    if probe.is_trusted() {
        backend.send_paste();
        "full_paste".to_string()
    } else {
        "write_back_only".to_string()
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

/// 辅助功能设置打开逻辑的可测实现：同步执行 `program <url>` 并按退出码返回结果。
///
/// 参数化 `program`/`url` 而非写死 `open`，是为了在单元/集成测试中注入假命令
/// （真实退出码 0 的 `true`、非零的 `false`、不存在的命令）覆盖三条返回路径；
/// 生产仅由下方 `open_accessibility_settings` 以 `open` 调用，语义专用于打开设置。
///
/// 用 `.status()` 而非 `.spawn()`：`Child` drop 时既不 wait 也不 kill，常驻托盘进程从不
/// 回收子进程、也未装 SIGCHLD 回收器，每次调用都会泄漏一个 defunct 僵尸进程直到 app 退出
/// 才被收割。`open` 是请求型命令（向 LaunchServices 发起后毫秒级返回），同步等待开销可忽略，
/// 借此原地回收子进程避免僵尸堆积；并能拿到退出码，让前台感知打开失败。
pub fn run_open_status_impl(program: &str, url: &str) -> Result<(), String> {
    let status = std::process::Command::new(program)
        .arg(url)
        .status()
        .map_err(|e| format!("无法打开辅助功能设置：{e}"))?;
    if !status.success() {
        return Err(format!("打开辅助功能设置失败：open 退出码 {status}"));
    }
    Ok(())
}

/// Tauri 命令：打开 macOS 辅助功能系统设置深链。
///
/// 使用系统 `open` 命令打开深链 URL（项目未依赖 tauri-plugin-opener，Cargo.toml 中无此
/// 依赖）。子进程回收语义见 `run_open_status_impl`。
#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    run_open_status_impl("open", ACCESSIBILITY_DEEPLINK)
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
///
/// 方案 B 修复：在 hide 之后、等待之前插入"按 pid 显式激活目标 app"。
/// 主窗口路径下 QuickQuick 长时间是前台，仅靠 `app.hide()` 隐式还焦给"上一个 app"
/// 会把焦点交给陈旧/错误的 app 致 Cmd+V 落空；显式激活记录到的目标 app 修正此问题。
/// `target_pid` 为 None（尚未记录到）或激活失败时降级走原隐式路径，不破坏 popover 流程。
fn hide_and_restore_focus(app: &tauri::AppHandle, target_pid: Option<i32>) {
    hide_window_and_activate_target(app, target_pid);
    // 仅粘贴路径需固定等待焦点转移完成后再 send_paste；纯关窗还焦（方案 C）不 send_paste，
    // 故等待逻辑留在本函数、不下沉到共享 helper，避免给关窗路径平添 350ms 卡顿。
    std::thread::sleep(std::time::Duration::from_millis(FOCUS_YIELD_WAIT_MS));
}

/// 隐藏窗口 + 隐藏 app + 按 pid 显式激活目标 app（hide-and-restore 的共享前半段，无等待）。
///
/// 三步顺序：HidePanel（hide clip-popover，无则 hide main）→ HideApp（app.hide() 让出前台）
/// → 按 `target_pid` 显式激活目标 app。激活放在 hide 之后，让目标在还焦后成为 key app。
///
/// 由两条路径复用：
/// - 粘贴路径 `hide_and_restore_focus`：在本函数后追加固定等待再 send_paste。
/// - 关窗还焦路径 `hide_and_return_focus` 命令（方案 C）：仅需还焦、无需等待。
///
/// `target_pid` 为 None 或激活失败时降级走 `app.hide()` 隐式还焦路径，不 panic。
fn hide_window_and_activate_target(app: &tauri::AppHandle, target_pid: Option<i32>) {
    hide_popover_window(app);
    hide_app(app);
    activate_target_app(app, target_pid);
}

/// Tauri 命令：隐藏当前窗口并把前台焦点还给上一个外部 app（方案 C）。
///
/// 复用 `hide_window_and_activate_target`：hide 窗口（popover 优先，无则 main）→ app.hide()
/// → 按 `LastExternalApp` 记录的 pid 显式激活。供 popover 的 Esc 关闭路径 invoke，
/// 替代裸 `getCurrentWindow().hide()`——后者只隐藏窗口、不把焦点显式交还触发处的 app。
///
/// 降级：未记录到 pid 时退化为纯 `app.hide()` 隐式还焦（macOS 自动让出前台），不 panic。
/// 不调用 send_paste、不固定等待，故无粘贴路径的 350ms 卡顿。
#[tauri::command]
pub fn hide_and_return_focus(
    last_external: State<'_, Arc<LastExternalApp>>,
    app: tauri::AppHandle,
) {
    hide_window_and_activate_target(&app, last_external.get());
}

/// 主窗关闭（CloseRequested）路径的还焦：app.hide() + 按记录 pid 显式激活上一个外部 app（方案 C）。
///
/// 与命令 `hide_and_return_focus` 的差异：此处窗口已由 `CloseRequested` 分支的 `win.hide()`
/// 隐藏，故不再 hide 窗口，只补 app.hide() 让出前台 + 显式激活目标，保留 stay_in_tray 语义
/// （进程驻留托盘、不退出）。
///
/// 通过 `app.state::<Arc<LastExternalApp>>()` 取托管状态——该状态在 setup 阶段必被托管
/// （非 macOS 也托管空状态），故 `try_state` 正常恒为 Some；缺失时降级跳过激活，不 panic。
pub fn return_focus_after_main_hide(app: &tauri::AppHandle) {
    let target_pid = app
        .try_state::<Arc<LastExternalApp>>()
        .and_then(|state| state.get());
    hide_app(app);
    activate_target_app(app, target_pid);
}

/// 按 pid 显式激活目标 app（方案 B 核心，macOS）。
///
/// 决策由纯函数 `activation_decision` 给出：
/// - `ActivatePid(pid)`：在主线程调 NSRunningApplication.activateWithOptions 激活目标。
/// - `FallbackHide`：无可用 pid，跳过显式激活，沿用 `app.hide()` 隐式还焦路径。
///
/// 线程模型：激活必须在主线程，故用 `run_on_main_thread` 派发（本函数在 paste 命令线程）。
/// 二次降级：runningApplicationWithProcessIdentifier 返回 nil（目标已退出）时跳过，不 panic。
#[cfg(target_os = "macos")]
fn activate_target_app(app: &tauri::AppHandle, target_pid: Option<i32>) {
    use crate::frontmost::{activation_decision, ActivationDecision};

    let ActivationDecision::ActivatePid(pid) = activation_decision(target_pid) else {
        return;
    };

    if let Err(e) = app.run_on_main_thread(move || activate_running_app_by_pid(pid)) {
        eprintln!("[paste_to_front] run_on_main_thread 派发激活失败: {e}");
    }
}

/// 在主线程按 pid 取回 NSRunningApplication 并激活（macOS）。
///
/// `activateWithOptions(empty)` 替代 objc2-app-kit 0.3 未暴露的无参 `activate()`，
/// 语义等价（activateWithOptions 在 macOS14+ 标弃用但仍可用，无参 activate 此版本未生成）。
/// app 已退出（runningApplication 返回 nil）时静默跳过，不 panic。
#[cfg(target_os = "macos")]
fn activate_running_app_by_pid(pid: i32) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

    // runningApplicationWithProcessIdentifier 是安全 API；pid 已由纯函数确保 > 0。
    let Some(running_app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
    else {
        // 目标 app 已退出：降级跳过，原 app.hide() 隐式路径已让出前台。
        return;
    };

    // 空选项即"激活但不强制前置全部窗口"，配合先前 hide 已让出前台，
    // 足以让目标成为 key app 接收 Cmd+V。
    running_app.activateWithOptions(NSApplicationActivationOptions::empty());
}

/// 非 macOS：无 NSRunningApplication，显式激活为 no-op（降级隐式路径）。
#[cfg(not(target_os = "macos"))]
fn activate_target_app(_app: &tauri::AppHandle, _target_pid: Option<i32>) {}

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
/// 图片原图已不可用（降级剥离）时返回 Err。
#[tauri::command]
pub fn paste_to_front(
    state: State<'_, AppDb>,
    last_external: State<'_, Arc<LastExternalApp>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<PasteResultDto, String> {
    let payload = with_db(&state, |conn| {
        fetch_paste_item(conn, &id).map_err(|e| e.to_string())
    })?;

    // 读取观察者记录的"最近外部前台 app" pid（方案 B），供粘贴时显式激活目标。
    let target_pid = last_external.get();
    let outcome = run_paste_with_backend(&app, &payload, target_pid);

    Ok(PasteResultDto { outcome })
}

/// 薄封装：新建 arboard 剪贴板并把载荷写入系统剪贴板（文本/图片两态）。
///
/// arboard 实写属 GUI 副作用、不进自动化（归 RT1-M01 manual_confirm）；
/// 取数+组装的可测逻辑在 `fetch_clip_for_copy`，本函数只做系统写入。
/// - `Text`：复用 `macos_paste::write_item_to_clipboard`（保证复制/粘贴写入行为一致）。
/// - `Image`：复用 `macos_paste::write_image_to_clipboard`（arboard set_image）。
///
/// # Errors
/// arboard 初始化或写入失败时返回错误字符串。
fn write_clip_to_system_clipboard(payload: &ClipboardPayload) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("剪贴板初始化失败: {e}"))?;
    match payload {
        ClipboardPayload::Text(item) => {
            crate::macos_paste::write_item_to_clipboard(&mut clipboard, item)
        }
        ClipboardPayload::Image {
            width,
            height,
            rgba,
        } => crate::macos_paste::write_image_to_clipboard(&mut clipboard, *width, *height, rgba),
    }
}

/// Tauri 命令：按 id 把条目（富文本带 html）写入系统剪贴板（RT1-F1-S04）。
///
/// 与 `paste_to_front` 的区别：仅写回剪贴板，不做 trusted 探测 / 焦点编排 / Cmd+V 注入，
/// 供前端"复制"按钮调用（F2 改前端接入此命令替代 navigator.clipboard.writeText）。
/// 文本条目有非空 html 时写富文本（HTML + 纯文本兜底），否则写纯文本；图片条目写图（set_image）。
/// 图片原图已不可用（降级剥离）时返回 Err。
#[tauri::command]
pub fn copy_clip_to_clipboard(state: State<'_, AppDb>, id: String) -> Result<(), String> {
    let payload = with_db(&state, |conn| {
        fetch_clip_for_copy(conn, &id).map_err(|e| e.to_string())
    })?;
    write_clip_to_system_clipboard(&payload)
}

/// 构造平台后端/probe，按载荷分流执行粘贴编排（含 trusted 分支的窗口 hide）。
///
/// 拆出独立函数以降低 paste_to_front 函数体长度，cfg 分流在此隔离。
///
/// - Text 臂：write_and_confirm 确认写入（marker 回读）→ hide 窗口 → send_paste → "full_paste"；
///   失败（Timeout）或未授权 → write_with_marker → "write_back_only"。
/// - Image 臂：write_image 写图（不套 marker 回读）→ trusted ? (hide → send_paste → "full_paste")
///   : "write_back_only"。
#[cfg(target_os = "macos")]
fn run_paste_with_backend(
    app: &tauri::AppHandle,
    payload: &ClipboardPayload,
    target_pid: Option<i32>,
) -> String {
    use crate::macos_paste::{MacOsAccessibilityProbe, MacOsPasteBackend};

    let probe = MacOsAccessibilityProbe;
    let mut backend = match MacOsPasteBackend::new() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[run_paste_with_backend] 后端初始化失败: {e}");
            return "write_back_only".to_string();
        }
    };

    match payload {
        ClipboardPayload::Text(item) => {
            paste_text_with_backend(app, &probe, &mut backend, item, target_pid)
        }
        ClipboardPayload::Image {
            width,
            height,
            rgba,
        } => paste_image_with_backend(
            app,
            &probe,
            &mut backend,
            *width,
            *height,
            rgba,
            target_pid,
        ),
    }
}

/// Text 臂粘贴：write_and_confirm（marker 回读）→ trusted 时 hide+send_paste。
///
/// 拆出以降低 `run_paste_with_backend` 的嵌套与长度。语义见调用方文档。
#[cfg(target_os = "macos")]
fn paste_text_with_backend(
    app: &tauri::AppHandle,
    probe: &impl AccessibilityProbe,
    backend: &mut crate::macos_paste::MacOsPasteBackend,
    item: &CapturedItem,
    target_pid: Option<i32>,
) -> String {
    if !probe.is_trusted() {
        backend.write_with_marker(item);
        return "write_back_only".to_string();
    }
    match paste::write_and_confirm(backend, item) {
        Ok(()) => {
            hide_and_restore_focus(app, target_pid);
            backend.send_paste();
            "full_paste".to_string()
        }
        Err(_) => "write_back_only".to_string(),
    }
}

/// Image 臂粘贴：write_image 写图（无 marker 回读）→ trusted 时 hide+send_paste。
///
/// 为何图片不套 marker 回读确认：macOS 写图后系统重编码为 TIFF，回读字节不等值，
/// marker 机制对图片无意义；arboard set_image 同步写入，写回去重由 image_hash 兜底。
#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
fn paste_image_with_backend(
    app: &tauri::AppHandle,
    probe: &impl AccessibilityProbe,
    backend: &mut crate::macos_paste::MacOsPasteBackend,
    width: usize,
    height: usize,
    rgba: &[u8],
    target_pid: Option<i32>,
) -> String {
    if backend.write_image(width, height, rgba).is_err() {
        // C1：写图失败时剪贴板未更新，跳过 send_paste 避免粘出前台旧内容。
        return "write_back_only".to_string();
    }
    if probe.is_trusted() {
        hide_and_restore_focus(app, target_pid);
        backend.send_paste();
        "full_paste".to_string()
    } else {
        "write_back_only".to_string()
    }
}

/// Windows：写回剪贴板 → 还焦到捕获的外部窗口 → SendInput Ctrl+V（完整粘贴）。
///
/// probe 恒 trusted（Windows 无辅助功能授权概念），故走完整路径：
/// write_and_confirm（轮询剪贴板序号确认写入）→ hide popover → SetForegroundWindow 还焦
/// → 固定等待 → send_paste。`target_pid`（macOS pid）在 Windows 无意义，忽略；
/// 还焦目标从托管的 `LastExternalHwnd` 取。
#[cfg(target_os = "windows")]
fn run_paste_with_backend(
    app: &tauri::AppHandle,
    payload: &ClipboardPayload,
    _target_pid: Option<i32>,
) -> String {
    use crate::windows_paste::{WindowsAccessibilityProbe, WindowsPasteBackend};

    let probe = WindowsAccessibilityProbe;
    let mut backend = match WindowsPasteBackend::new() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[run_paste_with_backend] 后端初始化失败: {e}");
            return "write_back_only".to_string();
        }
    };
    let target_hwnd = app
        .try_state::<Arc<crate::frontmost::LastExternalHwnd>>()
        .and_then(|state| state.get());

    match payload {
        ClipboardPayload::Text(item) => {
            paste_text_with_backend_windows(app, &probe, &mut backend, item, target_hwnd)
        }
        ClipboardPayload::Image {
            width,
            height,
            rgba,
        } => paste_image_with_backend_windows(
            app,
            &probe,
            &mut backend,
            *width,
            *height,
            rgba,
            target_hwnd,
        ),
    }
}

/// Windows Text 臂：write_and_confirm（序号确认）→ 还焦 → send_paste。
///
/// 对称于 macOS `paste_text_with_backend`，但还焦走 `hide_and_restore_focus_windows`
/// （SetForegroundWindow），且 probe 恒 trusted。
#[cfg(target_os = "windows")]
fn paste_text_with_backend_windows(
    app: &tauri::AppHandle,
    probe: &impl AccessibilityProbe,
    backend: &mut crate::windows_paste::WindowsPasteBackend,
    item: &CapturedItem,
    target_hwnd: Option<isize>,
) -> String {
    if !probe.is_trusted() {
        backend.write_with_marker(item);
        return "write_back_only".to_string();
    }
    match paste::write_and_confirm(backend, item) {
        Ok(()) => {
            if hide_and_restore_focus_windows(app, target_hwnd) {
                backend.send_paste();
                "full_paste".to_string()
            } else {
                "write_back_only".to_string()
            }
        }
        Err(_) => "write_back_only".to_string(),
    }
}

/// Windows Image 臂：write_image（无 marker 回读）→ 还焦 → send_paste。
///
/// 对称于 macOS `paste_image_with_backend`；写图失败时跳过 send_paste（C1，避免粘旧内容）。
#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments)]
fn paste_image_with_backend_windows(
    app: &tauri::AppHandle,
    probe: &impl AccessibilityProbe,
    backend: &mut crate::windows_paste::WindowsPasteBackend,
    width: usize,
    height: usize,
    rgba: &[u8],
    target_hwnd: Option<isize>,
) -> String {
    if backend.write_image(width, height, rgba).is_err() {
        return "write_back_only".to_string();
    }
    if probe.is_trusted() && hide_and_restore_focus_windows(app, target_hwnd) {
        backend.send_paste();
        "full_paste".to_string()
    } else {
        "write_back_only".to_string()
    }
}

/// Windows：hide popover/main → SetForegroundWindow 还焦 → 固定等待焦点转移完成。
///
/// 对称于 macOS `hide_and_restore_focus`，但还焦用 `SetForegroundWindow(hwnd)`
/// 把焦点交回唤起时捕获的外部窗口（`hide_popover_window` 仅隐藏自身窗口、不交焦点）。
/// 返回是否成功把焦点交回目标窗口：`target_hwnd` 为 Some 且 `SetForegroundWindow` 成功才 `true`；
/// 为 None（未捕获到）或还焦失败时返回 `false`——调用方据此跳过 `send_paste`、仅写回，
/// 避免在焦点未交回目标窗口时把 Ctrl+V 注入到当前任意活跃窗口。
/// 等待复用 `FOCUS_YIELD_WAIT_MS`：让焦点转移自然完成后再 send_paste，避免 Ctrl+V 落空。
#[cfg(target_os = "windows")]
fn hide_and_restore_focus_windows(app: &tauri::AppHandle, target_hwnd: Option<isize>) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

    hide_popover_window(app);

    let Some(hwnd) = target_hwnd else {
        return false;
    };

    // SetForegroundWindow 把焦点交回目标窗口；返回 0 表示失败（如目标已关闭），降级记录。
    let ok = unsafe { SetForegroundWindow(hwnd as _) } != 0;
    if !ok {
        eprintln!("[paste_to_front] SetForegroundWindow 失败（目标窗口可能已关闭），降级仅写回");
    }

    std::thread::sleep(std::time::Duration::from_millis(FOCUS_YIELD_WAIT_MS));
    ok
}

/// Linux 等其余非 macOS/非 Windows 平台：probe 恒 false，仅写回不注入粘贴（保留 Fallback）。
#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn run_paste_with_backend(
    app: &tauri::AppHandle,
    payload: &ClipboardPayload,
    _target_pid: Option<i32>,
) -> String {
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

    // 仅写回不注入粘贴（图片走与文本对齐的 C1-安全编排）。
    match payload {
        ClipboardPayload::Text(item) => paste_orchestrate(&probe, &mut backend, item),
        ClipboardPayload::Image {
            width,
            height,
            rgba,
        } => paste_image_orchestrate(&probe, &mut backend, *width, *height, rgba),
    }
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
                 is_favorite       INTEGER NOT NULL DEFAULT 0,
                 html_content      TEXT
             );
             CREATE TABLE IF NOT EXISTS clip_images (
                 id                TEXT PRIMARY KEY NOT NULL,
                 clip_item_id      TEXT REFERENCES clip_items(id) ON DELETE CASCADE,
                 thumbnail         BLOB,
                 original          BLOB,
                 original_present  INTEGER NOT NULL DEFAULT 0,
                 image_hash        TEXT,
                 created_utc       INTEGER NOT NULL,
                 last_modified_utc INTEGER NOT NULL,
                 is_deleted        INTEGER NOT NULL DEFAULT 0,
                 deleted_at_utc    INTEGER,
                 is_favorite       INTEGER NOT NULL DEFAULT 0
             );",
        )
        .expect("建测试表失败");
        conn
    }

    /// 造一条 image clip_item + 关联 clip_images 行（原图为给定 PNG BLOB）。
    ///
    /// 返回 clip_item id。`original` 传空 Vec 可模拟降级剥离原图后的空 BLOB。
    fn insert_image_clip(conn: &Connection, original: &[u8]) -> String {
        let item_id = "img-item-1".to_string();
        conn.execute(
            "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc)
             VALUES (?1, '', 'image', 1, 1)",
            rusqlite::params![item_id],
        )
        .expect("插入 image clip_item 失败");
        conn.execute(
            "INSERT INTO clip_images
                 (id, clip_item_id, original, original_present, created_utc, last_modified_utc)
             VALUES ('img-1', ?1, ?2, 1, 1, 1)",
            rusqlite::params![item_id, original],
        )
        .expect("插入 clip_images 失败");
        item_id
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
        /// 为 true 时 write_image 返回 Err，模拟 arboard set_image 失败（C1）
        write_image_fails: bool,
        /// write_image 是否被调用过
        write_image_called: bool,
    }
    impl FakeBackend {
        fn trusted_normal() -> Self {
            Self {
                count: 0,
                text: None,
                send_paste_called: false,
                freeze_count: false,
                write_image_fails: false,
                write_image_called: false,
            }
        }
        fn frozen_count() -> Self {
            Self {
                count: 0,
                text: None,
                send_paste_called: false,
                freeze_count: true,
                write_image_fails: false,
                write_image_called: false,
            }
        }
        /// 写图必失败的后端，用于 C1：验证写图失败时不 send_paste。
        fn image_write_fails() -> Self {
            Self {
                write_image_fails: true,
                ..Self::trusted_normal()
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
        fn write_image(
            &mut self,
            _width: usize,
            _height: usize,
            _rgba: &[u8],
        ) -> Result<(), String> {
            self.write_image_called = true;
            if self.write_image_fails {
                Err("模拟 set_image 失败".to_string())
            } else {
                Ok(())
            }
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

    /// T5b：fetch_paste_item — image 条目有原图 → 返回 Image 载荷，锚定宽高
    #[test]
    fn fetch_paste_item_image_with_original_returns_image_payload() {
        use crate::clipboard::{rgba_to_png_for_test, ClipboardPayload};

        let conn = make_test_conn();
        // 2×1 已知小图：像素0=红、像素1=绿
        let rgba = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let png = rgba_to_png_for_test(2, 1, &rgba).expect("编码已知小图应成功");
        let item_id = insert_image_clip(&conn, &png);

        let payload = fetch_paste_item(&conn, &item_id).expect("有原图的 image 条目应返回 Ok");

        match payload {
            ClipboardPayload::Image {
                width,
                height,
                rgba: decoded,
            } => {
                assert_eq!(width, 2, "解码宽度应为 2");
                assert_eq!(height, 1, "解码高度应为 1");
                assert_eq!(decoded, rgba, "解码 RGBA 应与原图逐字节相等");
            }
            ClipboardPayload::Text(_) => panic!("image 条目应返回 Image 载荷，而非 Text"),
        }
    }

    /// T5c：fetch_paste_item — image 条目原图为空 BLOB（降级剥离）→ 返回 Err
    #[test]
    fn fetch_paste_item_image_empty_original_returns_err() {
        let conn = make_test_conn();
        let item_id = insert_image_clip(&conn, &[]); // 空 BLOB 模拟降级剥离

        let result = fetch_paste_item(&conn, &item_id);

        assert!(result.is_err(), "原图为空时应返回 Err");
        assert!(
            result.unwrap_err().to_string().contains("原图已不可用"),
            "错误信息应说明原图已不可用"
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

    /// C1①：paste_image_orchestrate — 写图失败 → 不调 send_paste，返回 write_back_only
    #[test]
    fn paste_image_orchestrate_write_fails_skips_send_paste() {
        let probe = FakeProbe { trusted: true };
        let mut backend = FakeBackend::image_write_fails();
        let rgba = vec![0u8; 4]; // 1×1 占位

        let outcome = paste_image_orchestrate(&probe, &mut backend, 1, 1, &rgba);

        assert_eq!(
            outcome, "write_back_only",
            "写图失败应返回 write_back_only"
        );
        assert!(backend.write_image_called, "应尝试过 write_image");
        assert!(
            !backend.send_paste_called,
            "写图失败时绝不能 send_paste（否则粘出前台旧内容）"
        );
    }

    /// C1②：paste_image_orchestrate — 写图成功 + trusted → send_paste 被调用，返回 full_paste
    #[test]
    fn paste_image_orchestrate_write_ok_trusted_sends_paste() {
        let probe = FakeProbe { trusted: true };
        let mut backend = FakeBackend::trusted_normal();
        let rgba = vec![0u8; 4];

        let outcome = paste_image_orchestrate(&probe, &mut backend, 1, 1, &rgba);

        assert_eq!(outcome, "full_paste", "写图成功+trusted 应返回 full_paste");
        assert!(backend.write_image_called, "应调用 write_image");
        assert!(backend.send_paste_called, "写图成功+trusted 应调用 send_paste");
    }

    /// C1③：paste_image_orchestrate — 写图成功但未授权 → 仅写回，不 send_paste
    #[test]
    fn paste_image_orchestrate_write_ok_untrusted_skips_send_paste() {
        let probe = FakeProbe { trusted: false };
        let mut backend = FakeBackend::trusted_normal();
        let rgba = vec![0u8; 4];

        let outcome = paste_image_orchestrate(&probe, &mut backend, 1, 1, &rgba);

        assert_eq!(
            outcome, "write_back_only",
            "未授权应返回 write_back_only"
        );
        assert!(backend.write_image_called, "应调用 write_image");
        assert!(!backend.send_paste_called, "未授权不应 send_paste");
    }

    /// 缺口 B：fetch_paste_item — 文本条目返回 Text 载荷，html 透传正确
    #[test]
    fn fetch_paste_item_text_returns_text_payload_with_html() {
        use crate::clipboard::ClipboardPayload;

        let conn = make_test_conn();
        // 直接插入带 html 的富文本条目（content + html_content 都非空）
        conn.execute(
            "INSERT INTO clip_items
                 (id, content, kind, created_utc, last_modified_utc, html_content)
             VALUES ('txt-1', 'plain text', 'text', 1, 1, '<b>plain text</b>')",
            [],
        )
        .expect("插入富文本条目失败");

        let payload = fetch_paste_item(&conn, "txt-1").expect("文本条目应返回 Ok");

        match payload {
            ClipboardPayload::Text(item) => {
                assert_eq!(item.text, "plain text", "纯文本字段应透传");
                assert_eq!(
                    item.html.as_deref(),
                    Some("<b>plain text</b>"),
                    "html 应原样透传"
                );
            }
            ClipboardPayload::Image { .. } => {
                panic!("文本条目应返回 Text 载荷，而非 Image")
            }
        }
    }
}
