//! Popover 浮层窗口管理模块
//!
//! 负责两个独立 popover 窗口的懒建、触发、失焦隐藏：
//! - `clip-popover`（720×480）：剪贴板快速选取
//! - `trans-popover`（320×200）：迷你翻译
//!
//! macOS 的 clip-popover 以非激活面板（NSPanel + nonactivatingPanel）承载：
//! 面板能成为 key window 接收键盘输入，却不会把 QuickQuick 变成前台应用。
//! 这一条决定了整条焦点编排——粘贴前不必隐藏 app、也不必等焦点交还，
//! 因为前台自始至终没被换走过。trans-popover 保持普通窗口语义。
//!
//! 窗口不写进 tauri.conf.json，首次触发时按需创建，复用时直接定位并显示。
//! 所有可失败操作均用 eprintln 记录并优雅降级，不 panic、不向上抛错。

use tauri::{Emitter, LogicalPosition, Manager, WebviewWindow, WindowEvent};

use crate::ipc::clipboard::pick_latest_translate_text_impl;
use crate::ipc::trans_popover::{build_trans_source, TransSource, TransSourceStore};
use crate::ipc::{with_db, AppDb};
use crate::selection::{self, ScreenRect, SelectionOutcome};
use crate::window_pos::{
    monitor_specs, place_for_platform, placement_for_width, placement_near_rect,
};

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use std::time::Duration;

/// trans-popover 待译文本推送事件名。
/// 与前端 src/ipc/events.ts 的 TRANS_SOURCE_EVENT 必须一致；
/// payload 为 [`TransSource`]（含文本、来源与降级原因），前端 `TransSourcePayload` 与之对齐。
/// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
const TRANS_SOURCE_EVENT: &str = "trans-source";

/// clip-popover 窗口 label。该窗口在 macOS 上以非激活面板承载。
const CLIP_POPOVER_LABEL: &str = "clip-popover";

/// trans-popover 窗口 label，用于区分仅对翻译 popover 推送待译文本。
const TRANS_POPOVER_LABEL: &str = "trans-popover";

/// clip-popover 窗口宽度（逻辑点：`inner_size` 按逻辑尺寸创建窗口，物理像素为逻辑 × 屏幕缩放比）
const CLIP_WIDTH: i32 = 720;
/// clip-popover 窗口高度（逻辑点）
const CLIP_HEIGHT: f64 = 480.0;
/// trans-popover 窗口宽度（逻辑点）
const TRANS_WIDTH: i32 = 320;
/// trans-popover 窗口高度（逻辑点）
const TRANS_HEIGHT: f64 = 200.0;

/// 把主线程任务派发出去后等待其完成的上限。
///
/// 主线程被长任务占住时不应无限等待：粘贴命令卡住比粘贴失败更糟，用户会以为整个
/// app 挂了。超时即判为「无法确认完成」，由调用方退成只写回剪贴板；正常路径远小于
/// 这个上界。
#[cfg(target_os = "macos")]
const MAIN_THREAD_WAIT_TIMEOUT: Duration = Duration::from_millis(500);

/// clip-popover 面板当前是否持有 key 状态。
///
/// 用原子布尔而不是随时读 `NSWindow.keyWindow`：粘贴命令跑在 worker 线程，
/// 跨线程读 AppKit 状态不安全；而「是否 key」只在唤起与 resign 两个已知时机变化，
/// 无锁原子量足够且线程安全。
#[cfg(target_os = "macos")]
static CLIP_PANEL_HOLDS_KEY: AtomicBool = AtomicBool::new(false);

/// 面板隐藏**正在执行中**的标记（重入守卫）。
///
/// 为什么必须有：`hide` 之前要先 `resign_key_window` 把键焦点交还前台，而
/// `NSWindow::resignKeyWindow` 会**同步** post `NSWindowDidResignKeyNotification`，
/// 我们的观察者回调（`observe_resign_key_hide`）又会调回隐藏逻辑——通知在主线程投递，
/// `on_main_thread_blocking` 发现自己已在主线程就直接内联执行，于是
/// `hide → resign → 通知 → hide → resign → 通知 → …` 无限递归，栈溢出后 SIGABRT。
/// 崩溃报告（`quickquick-*.ips`，EXC_BAD_ACCESS / 栈溢出）里的调用链就是这个环。
static PANEL_HIDING: AtomicBool = AtomicBool::new(false);

/// `PANEL_HIDING` 的 RAII 复位：任何退出路径（正常返回 / 提前 return / panic）都会复位。
///
/// 只在**真正执行隐藏的那一层**创建——重入的那一层不能复位，否则会把外层的标记清掉，
/// 让第三次重入又跑一遍隐藏逻辑。
struct PanelHidingGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> PanelHidingGuard<'a> {
    fn new(flag: &'a AtomicBool) -> Self {
        Self { flag }
    }
}

impl Drop for PanelHidingGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

/// 单次隐藏的执行骨架：**外层执行、内层直接返回成功**。
///
/// `resign` / `hide` 以闭包注入，使「resign 同步回调重入」这条路径可以在单测里精确复现，
/// 不必真的建 NSPanel。
///
/// 返回值：真正完成隐藏或确认「外层已在隐藏」都算 `true`；`hide` 自身的返回值原样透出。
fn hide_once<R, H>(guard: &AtomicBool, mut resign: R, mut hide: H) -> bool
where
    R: FnMut(),
    H: FnMut() -> bool,
{
    if guard.swap(true, Ordering::SeqCst) {
        // 重入：外层正在隐藏，隐藏动作已在进行中，不重复执行、也不复位外层的标记
        return true;
    }
    let _reset = PanelHidingGuard::new(guard);
    resign();
    hide()
}

/// Popover 窗口规格：统一描述一个 popover 所需的全部参数，避免重复硬编码。
struct PopoverSpec {
    label: &'static str,
    url: &'static str,
    width: i32,
    height: f64,
}

/// 两个 popover 的规格表，顺序与 label 一一对应。
const POPOVER_SPECS: &[PopoverSpec] = &[
    PopoverSpec {
        label: CLIP_POPOVER_LABEL,
        url: "src/clip-popover/index.html",
        width: CLIP_WIDTH,
        height: CLIP_HEIGHT,
    },
    PopoverSpec {
        label: TRANS_POPOVER_LABEL,
        url: "src/trans-popover/index.html",
        width: TRANS_WIDTH,
        height: TRANS_HEIGHT,
    },
];

/// clip-popover 的非激活面板类型定义。仅 macOS 需要。
///
/// `can_become_key_window`：面板不激活 app 也能成为 key window，键盘事件才会进入 webview，
/// 这是整套非激活方案成立的前提（普通 NSWindow 必须先激活 app 才收键盘）。
/// `can_become_main_window=false`：它不是主窗口，不应参与主窗语义。
/// `is_floating_panel=true`：浮动面板不会随 app 失活自动隐藏，可稳定浮在普通窗口之上。
#[cfg(target_os = "macos")]
mod clip_panel {
    use tauri_nspanel::tauri_panel;

    tauri_panel! {
        panel!(ClipPopoverPanel {
            config: {
                can_become_key_window: true,
                can_become_main_window: false,
                is_floating_panel: true
            }
        })
    }
}

/// 粘贴前的键盘焦点交出策略。
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasteFocusPlan {
    /// clip-popover 面板持有 key：只需让面板让出 key，⌘V 就落到前台应用。
    YieldClipPanel,
    /// 面板未持有 key（主窗口粘贴 / 面板降级路径）：沿用隐藏窗口 + 隐藏 app + 激活目标 app。
    HideAppAndActivateTarget,
}

/// 依「面板是否持有 key」决定粘贴前如何交出键盘焦点。
///
/// 为什么必须让出：面板处于 key 状态时，合成出来的 CGEvent ⌘V 会先被面板自己的
/// webview 消费掉，前台应用什么也收不到——表现就是「粘贴看起来成功但没内容」。
/// 让面板先释放 key，键焦点回到前台应用，注入才有效。
///
/// 抽成纯函数是为了让这条判据能在 headless 环境单测：真实聚焦行为只能真机验证，
/// 但「什么时候该走面板路径」是可以断言的决策。
#[must_use]
#[cfg(target_os = "macos")]
pub(crate) fn paste_focus_plan(clip_panel_holds_key: bool) -> PasteFocusPlan {
    if clip_panel_holds_key {
        PasteFocusPlan::YieldClipPanel
    } else {
        PasteFocusPlan::HideAppAndActivateTarget
    }
}

/// 交出焦点之后是否还能安全注入 ⌘V。
///
/// 只有面板路径存在「确定性交出失败」这种状态：`hide_clip_popover` 超时或派发失败时，
/// 面板可能仍持有 key，此时注入的 ⌘V 会被面板自己的 webview 吞掉——注入方看到的是
/// "按键已发出"，用户看到的是"粘贴成功"，而内容根本没进前台应用。
/// 确认不了就该退成只写回剪贴板，把"要不要手动粘"交回用户，而不是兑现不了的成功。
///
/// 主窗口路径不参与本判据：它靠 hide + 显式激活目标 app + 固定等待完成还焦，
/// 结果无法在命令线程回读（见 `ipc::system::FOCUS_YIELD_WAIT_MS` 的注释），
/// 故沿用既有行为，避免在无法判定的地方臆造一个失败信号。
#[must_use]
#[cfg(target_os = "macos")]
pub(crate) fn should_inject_paste(plan: PasteFocusPlan, focus_yielded: bool) -> bool {
    match plan {
        PasteFocusPlan::YieldClipPanel => focus_yielded,
        PasteFocusPlan::HideAppAndActivateTarget => true,
    }
}

/// clip-popover 面板当前是否持有键盘焦点。
///
/// 只有 macOS 面板路径有这个概念：非 macOS 上 clip-popover 始终是普通窗口，
/// 粘贴一律走隐藏 app 那条旧路径。
#[must_use]
#[cfg(target_os = "macos")]
pub(crate) fn clip_popover_holds_key() -> bool {
    CLIP_PANEL_HOLDS_KEY.load(Ordering::SeqCst)
}

/// 隐藏 clip-popover 面板（macOS 面板路径专用）。
///
/// 供两处调用，语义都是"让面板消失并把键盘焦点交回前台应用"：
/// - 粘贴前：面板若不先让出 key，合成的 ⌘V 会被它自己吞掉；
/// - Esc 关闭：面板本来就是非激活的，隐藏它就把焦点还给了前台应用，
///   不需要隐藏整个 app，也不需要显式激活目标 app。
///
/// 返回 `true` 表示面板已完成 resign + order out，调用方可以立刻注入 ⌘V；
/// 返回 `false` 表示无法确认面板已让出 key（主线程派发失败 / 等待超时 / 取不到面板）。
/// 粘贴路径必须据此放弃注入：面板还持有 key 时注入的 ⌘V 会被它自己吞掉，
/// 而注入方无从察觉，最终表现成"界面报成功、实际什么都没粘"。
#[must_use]
#[cfg(target_os = "macos")]
pub(crate) fn hide_clip_popover(app: &tauri::AppHandle) -> bool {
    let app_main = app.clone();
    on_main_thread_blocking(app, move || hide_clip_panel_now(&app_main))
}

/// 触发指定 label 的 popover 窗口：get_or_create → 定位 → 唤起。
///
/// `label` 必须是 `"clip-popover"` 或 `"trans-popover"`；
/// 未知 label 会记录错误并立即返回。
pub fn trigger_popover(handle: &tauri::AppHandle, label: &str) {
    let Some(spec) = POPOVER_SPECS.iter().find(|s| s.label == label) else {
        eprintln!("[QuickQuick] 未知 popover label: {label}");
        return;
    };

    // 取词与取选区边界都必须发生在任何 show / 激活之前：QuickQuick 一旦成为前台应用，
    // 键盘焦点就离开了源应用，AX 读到的选中文字与选区边界都只会是空。故先把「译什么 +
    // 来源 + 浮窗该放哪儿」一次定下来，后面只负责展示。
    let prepared = (spec.label == TRANS_POPOVER_LABEL).then(|| prepare_translate(handle));

    let Some(window) = get_or_create_popover(handle, spec) else {
        return;
    };

    // 定位放在 show 之前：面板 order front 时就已经落在目标位置，避免可见的跳动。
    // 优先级：选区边界（有且落在某显示器内）→ 光标所在屏居中靠上。
    // 两条路径都先算出逻辑点与目标屏，再由平台出口决定交逻辑点（macOS）还是按目标屏换算的物理点（Windows）。
    // clip-popover 没有取词，走的是后者。注意：回退路径的位置在 macOS 单屏 2x 下相对改动前
    // 有意改变（改动前偏右窗口宽/4，现为真正居中），见 window_pos 模块头与 TP1-M02。
    // Windows 上两条路径都按目标屏的 scale 换算物理点，不依赖窗口当前所在屏。
    let (logical, target) = match prepared.as_ref().and_then(|p| p.selection_bounds) {
        Some(bounds) => translate_popover_placement(&window, spec, bounds),
        None => fallback_popover_placement(&window, spec),
    };
    // 没有目标屏（显示器列表为空）时无从做平台换算，直接交逻辑点原点兜底。
    let position = target.map_or_else(
        || {
            tauri::Position::Logical(tauri::LogicalPosition::new(
                f64::from(logical.x),
                f64::from(logical.y),
            ))
        },
        |monitor| place_for_platform(logical, monitor),
    );
    if let Err(e) = window.set_position(position) {
        eprintln!("[QuickQuick] {label} set_position 失败: {e}");
    }
    // Windows：show 前快照当前外部前台窗口，供粘贴还焦时 SetForegroundWindow 还原。
    // 一旦 show 自己的窗口，前台就成了 QuickQuick，再取就晚了。
    #[cfg(target_os = "windows")]
    crate::frontmost::capture_foreground_window(handle);

    if label == CLIP_POPOVER_LABEL {
        show_clip_popover(handle, &window);
        return;
    }

    if let Err(e) = window.show() {
        eprintln!("[QuickQuick] {label} show 失败: {e}");
        return;
    }
    // show 后立即把取好的内容推给 trans-popover：让前端无需再 listClipItems()
    // 往返查加密 DB（去掉可见延迟），并由 Rust 主动推送驱动重译（消除旧译文残影）。
    // 推送安排在激活之前：先让内容就位，再让窗口接住键盘焦点。
    if let Some(prepared) = prepared {
        push_translate_source(&window, prepared.source);
    }
    // trans-popover 仍是普通窗口：先激活 app，再聚焦窗口。
    // 顺序说明：macOS 要求进程本身处于"活跃"(active)状态后，
    // 窗口的 makeKeyAndOrderFront 才能拿到 key 状态。
    // 若反序（先 set_focus 后 activate），窗口已前置但 app 未激活，
    // 依然收不到键盘事件——全局热键回调场景必须显式激活。
    activate_app_macos();
    if let Err(e) = window.set_focus() {
        eprintln!("[QuickQuick] {label} set_focus 失败: {e}");
    }
}

/// 唤起 clip-popover。
///
/// macOS 上它是非激活面板：`show_and_make_key()` 一步同时完成「前置」与「拿键盘」，
/// 且不改变前台应用——所以面板路径既没有 `activate_app_macos()` 也没有 `set_focus()`。
///
/// 面板不可用（非 macOS / 转面板失败）时降级为普通窗口的「show → activate → set_focus」
/// 编排，即本改动之前的行为，保证任何环境都还能用。
fn show_clip_popover(handle: &tauri::AppHandle, window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        if show_clip_panel(handle) {
            return;
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handle;

    if let Err(e) = window.show() {
        eprintln!("[QuickQuick] clip-popover show 失败: {e}");
        return;
    }
    activate_app_macos();
    if let Err(e) = window.set_focus() {
        eprintln!("[QuickQuick] clip-popover set_focus 失败: {e}");
    }
}

/// 一次热键唤起所需的两样结果：译什么（含来源）+ 选区落在哪儿（用于定位）。
///
/// 两者必须来自**同一次** AX 读取：本进程 show 之后源应用不再有键盘焦点，补读只能拿到空选区。
/// 合成一个结构体而不是返回元组，是为了让调用点自己说清「这两项同源」。
struct TranslatePreparation {
    /// 待译文本与来源，同时也是推给前端的 payload。
    source: TransSource,
    /// 选区边界（逻辑点）；没读到选中文字时为 `None`。
    selection_bounds: Option<ScreenRect>,
}

/// 定下本次热键该翻译什么、浮窗贴近哪里，并把来源快照留给前端冷启动补读。
///
/// 顺序是「先取选中文字，再决定要不要查库」：命中选中文字时省掉一次解密 DB 往返，
/// 热键路径的可见延迟主要就花在那里。取词失败（未授权 / 无焦点 / 平台不支持）一律
/// 降级为剪贴板最新一条，并把**原因**带在 payload 里，由浮窗如实标注来源。
fn prepare_translate(handle: &tauri::AppHandle) -> TranslatePreparation {
    let snapshot = selection::read_selection_snapshot();
    // 先取边界再移动 outcome：只有读到选中文字时 bounds 才可信（见 SelectionSnapshot::usable_bounds）。
    let selection_bounds = snapshot.usable_bounds();

    // 命中选中文字时省掉一次解密 DB 往返；只有取词失败才读剪贴板降级。
    let clipboard = match &snapshot.outcome {
        SelectionOutcome::Text(_) => None,
        _ => read_latest_clipboard_text(handle),
    };
    let source = build_trans_source(snapshot.outcome, clipboard);

    // 快照必须在窗口显示之前写入：前端首次挂载时 emit 已经过去，只能补读这一份。
    // 状态未注册（测试环境或未来重构）时只降级成「冷启动走前端兜底」，不让热键路径 panic。
    match handle.try_state::<TransSourceStore>() {
        Some(store) => store.remember(source.clone()),
        None => eprintln!("[QuickQuick] trans-popover 来源快照未注册，冷启动将退回前端自读"),
    }

    TranslatePreparation {
        source,
        selection_bounds,
    }
}

/// trans-popover 贴近选区的落点；选区不在任何显示器内时回退到光标定位。
///
/// 出口是**逻辑点**（tauri 的 `LogicalPosition`）：`set_outer_position` 会先用窗口自身的
/// scale 把位置转成逻辑点，窗口隐藏时那个 scale 还是上次所在屏的值——交物理坐标会在
/// 1x/2x 混合布局里被除错、落到别的屏上；逻辑点在 tao 侧原样透传。
///
/// `compute_position_near_rect` 返回 `None` 就等于「这份 bounds 不可用」——那是显示器
/// 坐标空间对不上或显示器列表为空的信号，此时回退比硬算一个可能跑出屏幕的坐标安全。
fn translate_popover_placement(
    window: &WebviewWindow,
    spec: &PopoverSpec,
    bounds: ScreenRect,
) -> (LogicalPosition<i32>, Option<crate::window_pos::MonitorSpec>) {
    let monitors = monitor_specs(window);
    let window_size = (spec.width, spec.height.round() as i32);
    match placement_near_rect(bounds, window_size, &monitors) {
        Some((position, monitor)) => (position, Some(monitor)),
        None => fallback_popover_placement(window, spec),
    }
}

/// 回退落点：光标所在屏居中靠上（逻辑点 + 目标屏）。
fn fallback_popover_placement(
    window: &WebviewWindow,
    spec: &PopoverSpec,
) -> (LogicalPosition<i32>, Option<crate::window_pos::MonitorSpec>) {
    placement_for_width(window, spec.width)
}

/// 读剪贴板历史里「当前该翻译的那一条」；库不可用/查询失败时记录原因并返回 None。
fn read_latest_clipboard_text(handle: &tauri::AppHandle) -> Option<String> {
    let state = handle.state::<AppDb>();
    match with_db(&state, |conn| {
        pick_latest_translate_text_impl(conn).map_err(|e| e.to_string())
    }) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("[QuickQuick] trans-popover 取剪贴板待译文本失败: {e}");
            None
        }
    }
}

/// 把本次唤起的待译文本与来源推给 trans-popover 窗口。
///
/// emit 失败只记日志、不向上抛：前端挂载后仍可用 `get_trans_popover_source` 补读同一份
/// 快照，最坏结果是多一次 IPC，而不是浮窗空白。与本文件其他可失败操作的处理风格一致。
fn push_translate_source(window: &WebviewWindow, source: TransSource) {
    if let Err(e) = window.emit(TRANS_SOURCE_EVENT, source) {
        eprintln!("[QuickQuick] trans-popover emit {TRANS_SOURCE_EVENT} 失败: {e}");
    }
}

/// 显式激活当前 app，使 QuickQuick 成为 macOS 活跃应用。
///
/// 为何需要此调用：tauri/tao 的 `window.set_focus()` 底层用的是已废弃的
/// `activateIgnoringOtherApps:YES`，在 macOS 14+ 上该方法已成 no-op。
/// 从全局热键触发时前台是其他 app，QuickQuick 进程未被激活，
/// `makeKeyAndOrderFront` 无法让窗口真正拿到 key——键盘事件永远不会进入 webview。
///
/// 此 helper 调用 `NSApplication.activate()`（macOS 14+ 正式接口），
/// 替代已废弃的 `activateIgnoringOtherApps:`，确保进程被激活。
///
/// clip-popover 改走非激活面板后不再需要它，但 trans-popover 与托盘显示 main
/// 仍是普通窗口路径，依然依赖它。
///
/// 拿不到 MainThreadMarker（理论上不应发生：热键回调在主线程）时优雅跳过，
/// 不 panic，与本文件其他错误处理风格（eprintln 降级）一致。
#[cfg(target_os = "macos")]
pub(crate) fn activate_app_macos() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    // MainThreadMarker::new() 是安全函数：内部读 pthread_main_np() 判断线程，
    // 全局热键回调在主线程，实践中此处总返回 Some；拿不到时优雅跳过。
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("[QuickQuick] activate_app_macos: 非主线程，跳过激活");
        return;
    };

    // sharedApplication 与 activate 均为 objc2-app-kit 0.3 安全 API，
    // MainThreadMarker 已保证主线程约束，无需额外 unsafe。
    let app = NSApplication::sharedApplication(mtm);
    app.activate();
}

/// 非 macOS 平台：空实现，编译时零开销消除。
#[cfg(not(target_os = "macos"))]
pub(crate) fn activate_app_macos() {}

/// 获取已存在的 popover 窗口，或在首次调用时按规格创建并绑定生命周期钩子。
fn get_or_create_popover(handle: &tauri::AppHandle, spec: &PopoverSpec) -> Option<WebviewWindow> {
    // 已存在则直接复用，无需重建
    if let Some(existing) = handle.get_webview_window(spec.label) {
        return Some(existing);
    }

    let window = tauri::WebviewWindowBuilder::new(
        handle,
        spec.label,
        tauri::WebviewUrl::App(spec.url.into()),
    )
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .resizable(false)
    .inner_size(spec.width as f64, spec.height)
    .build();

    match window {
        Ok(w) => {
            bind_popover_lifecycle(&w, spec.label == CLIP_POPOVER_LABEL);
            Some(w)
        }
        Err(e) => {
            eprintln!("[QuickQuick] {} 创建失败: {e}", spec.label);
            None
        }
    }
}

/// 绑定 popover 的生命周期钩子（失焦隐藏）。
///
/// clip-popover 在 macOS 上是面板：它转面板成功就由 resign key 通知驱动隐藏
/// （见 `observe_resign_key_hide`）。转面板失败时降级回普通窗口的
/// `WindowEvent::Focused(false)` 语义，行为等同改造前。
fn bind_popover_lifecycle(window: &WebviewWindow, is_clip: bool) {
    #[cfg(target_os = "macos")]
    {
        if is_clip && convert_clip_popover_to_panel(window) {
            return;
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = is_clip;

    register_focus_lost_hide(window);
}

/// 在窗口上注册失焦隐藏事件：失去焦点时自动 hide。
///
/// 用 clone 将窗口句柄移入闭包，避免生命周期问题。
fn register_focus_lost_hide(window: &WebviewWindow) {
    let window_clone = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            if let Err(e) = window_clone.hide() {
                eprintln!("[QuickQuick] popover hide 失败: {e}");
            }
            // trans-popover 隐藏后清掉来源快照：选中的文字不应在窗口关闭后继续留在内存里。
            // 只在失焦隐藏时清：挂载补读发生在用户能看到窗口之前，不会被这里清掉。
            if window_clone.label() == TRANS_POPOVER_LABEL {
                if let Some(store) = window_clone.app_handle().try_state::<TransSourceStore>() {
                    store.clear();
                }
            }
        }
    });
}

/// 把已创建的 clip-popover 窗口就地转成非激活 NSPanel，并绑定失焦隐藏。返回是否成功。
///
/// 动机：普通 NSWindow 要收键盘必须先让整个 app 成为活跃应用，于是粘贴时得先
/// 隐藏 app 再等焦点交还（固定等待 + 目标 app 错乱风险）。NSPanel 配 nonactivatingPanel
/// 后，面板不激活本 app 也能成为 key window，键盘事件照常进入 webview，
/// 粘贴时只需让面板自己让出 key。
///
/// 转换失败时保持普通窗口语义（eprintln 降级），功能不受影响。
///
/// 必须主线程调用：`to_panel` 内部用 `object_setClass` 换窗口类，受 AppKit 主线程约束。
#[cfg(target_os = "macos")]
fn convert_clip_popover_to_panel(window: &WebviewWindow) -> bool {
    use tauri_nspanel::{PanelLevel, StyleMask, WebviewWindowExt};

    if !on_main_thread() {
        eprintln!("[QuickQuick] 转面板不在主线程，降级为普通窗口");
        return false;
    }
    if !panel_registered(window.app_handle()) {
        eprintln!("[QuickQuick] tauri-nspanel 插件未注册，clip-popover 保持普通窗口");
        return false;
    }

    let panel = match window.to_panel::<clip_panel::ClipPopoverPanel>() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[QuickQuick] clip-popover 转 NSPanel 失败，保持普通窗口: {e}");
            return false;
        }
    };

    // 用 add 而非 set：保留 Tauri 已设的窗口结构样式，只追加「不激活 app」语义。
    // 该调用同时同步 AppKit 侧「阻止应用被激活」的开关，是面板不抢前台的关键。
    if let Err(e) = panel.add_style_mask(StyleMask::empty().nonactivating_panel().into()) {
        eprintln!("[QuickQuick] clip-popover 追加 nonactivatingPanel 失败（面板会激活 app，粘贴需退回旧路径）: {e}");
    }
    // Dock 层级（20）足以盖住普通窗口，又不会压过系统菜单栏与系统弹窗。
    panel.set_level(PanelLevel::Dock.value());
    panel.set_collection_behavior(hidden_space_behavior());

    observe_resign_key_hide(window);
    true
}

/// 面板处于隐藏态时的跨 Space 行为：跟随用户当前所在的 Space 落位。
///
/// `stationary` 避免随 Space 切换做位移动画；`full_screen_auxiliary` 是它能出现在
/// 全屏应用上方的必要条件；`move_to_active_space` 保证显示时落在用户此刻的 Space，
/// 而不是上次隐藏时那个（多屏 + 多 Space 下最典型的「唤起了却看不见」）。
#[cfg(target_os = "macos")]
fn hidden_space_behavior() -> tauri_nspanel::objc2_app_kit::NSWindowCollectionBehavior {
    use tauri_nspanel::CollectionBehavior;

    CollectionBehavior::new()
        .stationary()
        .move_to_active_space()
        .full_screen_auxiliary()
        .into()
}

/// 面板处于显示态时的跨 Space 行为：加入所有 Space，跟随用户当前视野。
///
/// 与 `hidden_space_behavior` 成对切换：显示期间面板稳定跟随用户，
/// 隐藏后复位，下次显示重新按当时所在的 Space 落位。
#[cfg(target_os = "macos")]
fn shown_space_behavior() -> tauri_nspanel::objc2_app_kit::NSWindowCollectionBehavior {
    use tauri_nspanel::CollectionBehavior;

    CollectionBehavior::new()
        .stationary()
        .can_join_all_spaces()
        .full_screen_auxiliary()
        .into()
}

/// 监听窗口「失去 key」通知来做失焦隐藏。
///
/// 为什么不用 `tauri_nspanel` 的 `set_event_handler`：它内部会 `setDelegate:`
/// 替换掉 tao 的窗口代理，此后 tao 收不到任何窗口回调——包括前端
/// `clip-popover` 依赖的 focus 事件（前端靠它在每次唤起时聚焦搜索框并清空搜索词）。
/// 换成观察 `NSWindowDidResignKeyNotification`：与 resign key 事件的信号源相同，
/// 但由通知中心广播，不占用 delegate，tao 原有事件链完好。
///
/// 观察者需与进程同生命周期（窗口只建一次、不重建），故 forget token 防止反注册。
#[cfg(target_os = "macos")]
fn observe_resign_key_hide(window: &WebviewWindow) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSWindowDidResignKeyNotification;
    use objc2_foundation::{NSNotification, NSNotificationCenter};

    let Ok(ns_window_ptr) = window.ns_window() else {
        eprintln!("[QuickQuick] 取 clip-popover 的 NSWindow 失败，失焦隐藏将不可用");
        return;
    };
    // ns_window() 返回的指针在窗口存活期间有效；这里只当作通知中心的对象过滤条件用，
    // 即「只监听我自己这个窗口的 resign key」。
    let ns_window_as_object: &AnyObject = unsafe { &*(ns_window_ptr as *const AnyObject) };

    let handle = window.app_handle().clone();
    let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
        CLIP_PANEL_HOLDS_KEY.store(false, Ordering::SeqCst);
        // 隐藏动作落到主线程：通知在主线程投递，这里再确认一次派发目标。
        let handle_for_hide = handle.clone();
        if !on_main_thread_blocking(&handle, move || hide_clip_panel_now(&handle_for_hide)) {
            eprintln!("[QuickQuick] clip-popover 失焦隐藏未完成，面板可能仍可见");
        }
    });

    let center = NSNotificationCenter::defaultCenter();
    let observer: Retained<_> = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWindowDidResignKeyNotification),
            Some(ns_window_as_object),
            None,
            &block,
        )
    };

    std::mem::forget(observer);
}

/// tauri-nspanel 插件状态是否已注册。
///
/// crate 的 `to_panel` / `get_webview_panel` 内部直接取插件状态，未注册时会 panic；
/// 本函数让所有面板入口先探测再调用，把「崩溃」降级成「保持普通窗口」。
#[cfg(target_os = "macos")]
fn panel_registered(app: &tauri::AppHandle) -> bool {
    app.try_state::<tauri_nspanel::WebviewPanelManager<tauri::Wry>>()
        .is_some()
}

/// 显示 clip-popover 面板并取 key（不激活 app）。返回是否成功。
#[cfg(target_os = "macos")]
fn show_clip_panel(app: &tauri::AppHandle) -> bool {
    use tauri_nspanel::ManagerExt;

    if !on_main_thread() {
        eprintln!("[QuickQuick] 唤起面板不在主线程，降级为普通窗口唤起");
        return false;
    }
    if !panel_registered(app) {
        return false;
    }
    let Ok(panel) = app.get_webview_panel(CLIP_POPOVER_LABEL) else {
        return false;
    };

    panel.set_collection_behavior(shown_space_behavior());
    panel.show_and_make_key();
    // 面板理论上总能成为 key；即便被外部抢走焦点，这里乐观置位也不会造成错误——
    // 粘贴路径最坏多做一次「让出 key + 隐藏」，结果仍然正确。
    CLIP_PANEL_HOLDS_KEY.store(true, Ordering::SeqCst);
    true
}

/// 隐藏面板并把跨 Space 行为复位。须主线程调用。返回是否真正完成。
///
/// 先用 `resign_key_window` 把键焦点交回前台应用，再 `hide` 让面板 order out：
/// 面板仍是 key 时合成出来的 ⌘V 会被面板自己的 webview 吞掉，顺序反了粘贴就落空。
#[must_use]
#[cfg(target_os = "macos")]
fn hide_clip_panel_now(app: &tauri::AppHandle) -> bool {
    use tauri_nspanel::ManagerExt;

    CLIP_PANEL_HOLDS_KEY.store(false, Ordering::SeqCst);

    // 前置检查放在守卫之外：这两条早退不会 resign、也不会 post 通知，没有重入风险。
    if !panel_registered(app) {
        eprintln!("[QuickQuick] 面板插件未注册，无法让 clip-popover 让出 key");
        return false;
    }
    let Ok(panel) = app.get_webview_panel(CLIP_POPOVER_LABEL) else {
        eprintln!("[QuickQuick] 取不到 clip-popover 面板，无法让出 key");
        return false;
    };

    // resign_key_window 会同步 post resign 通知 → 观察者回调重入本函数；
    // 由 hide_once 的重入守卫把内层变成「外层已在隐藏」的快速返回，避免无限递归。
    hide_once(
        &PANEL_HIDING,
        || panel.resign_key_window(),
        || {
            panel.hide();
            panel.set_collection_behavior(hidden_space_behavior());
            true
        },
    )
}

/// 在主线程上同步执行 `f`，返回 `f` 的结果；无法确认已完成时返回 `false`。
///
/// 为什么需要「同步」：AppKit 调用只能主线程做，而粘贴命令跑在 worker 线程；
/// 若只投递不等待（`run_on_main_thread` 本身是异步的），紧接着注入的 ⌘V 会赶在
/// 面板真正 order out 之前发出，被面板自己的 webview 吞掉——表现是粘贴静默失败。
/// 这里投递后阻塞等待完成，用「确定已做完」替代「猜一个等待时长」。
///
/// 派发失败与等待超时都返回 `false`：两者都无法确认主线程上的操作已经完成，
/// 调用方若照「已完成」继续推进，就只是在赌一个自己没验证过的前提。
///
/// 若调用时已在主线程（粘贴命令理论上不在主线程，但防止未来改动踩坑），
/// 就地执行，避免自己等自己造成死锁。
#[must_use]
#[cfg(target_os = "macos")]
fn on_main_thread_blocking(
    app: &tauri::AppHandle,
    f: impl FnOnce() -> bool + Send + 'static,
) -> bool {
    if on_main_thread() {
        return f();
    }

    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    if let Err(e) = app.run_on_main_thread(move || {
        let done = f();
        // 接收端可能已超时放弃，发送失败不值得记录。
        let _ = tx.send(done);
    }) {
        eprintln!("[QuickQuick] 派发主线程任务失败: {e}");
        return false;
    }

    match rx.recv_timeout(MAIN_THREAD_WAIT_TIMEOUT) {
        Ok(done) => done,
        Err(_) => {
            eprintln!(
                "[QuickQuick] 等待主线程任务超时（{}ms）",
                MAIN_THREAD_WAIT_TIMEOUT.as_millis()
            );
            false
        }
    }
}

/// 当前线程是否就是 macOS 主线程。
///
/// 面板操作（`object_setClass`、`makeKeyAndOrderFront`、`orderOut`）都只能在主线程做，
/// 越线程调用属于未定义行为且不报错。用一次探针把这种可能性显式化：
/// 万一将来唤起点换了线程，降级成普通窗口并在日志里说清楚，而不是静默踩 UB。
#[cfg(target_os = "macos")]
fn on_main_thread() -> bool {
    objc2::MainThreadMarker::new().is_some()
}

#[cfg(test)]
mod tests {
    // activate_app_macos 是纯 FFI 包装，无法在无 GUI 的 headless 环境中真正调用
    // NSApplication（sharedApplication 在无 NSApp 时行为未定义）。
    // 此处验证的是：函数签名存在、可被引用、cfg 门控正确——即"编译即通过"的结构测试。
    // 运行时激活行为需通过手动集成测试（全局热键触发 + 键盘输入验证）验证。

    /// 重入守卫：resign 同步回调里再次触发隐藏时，内层只快速返回，不重复执行隐藏。
    ///
    /// 这条对应真机崩溃链：hide → resign_key_window → 同步 post 通知 → 观察者回调 → hide。
    /// 没有守卫时内层会再跑一遍 resign + hide（真机上就是无限递归 → 栈溢出 SIGABRT）。
    #[test]
    fn hide_once_ignores_reentrant_hide_from_resign_callback() {
        use std::cell::Cell;
        use std::sync::atomic::{AtomicBool, Ordering};

        let guard = AtomicBool::new(false);
        let resign_calls = Cell::new(0);
        let hide_calls = Cell::new(0);
        // 回调里的第二次隐藏是否已经跑过（用它避免「守卫失效时」真的无限递归，
        // 让变异能给出干净的红，而不是把测试进程跑崩）
        let reentered = Cell::new(false);

        let outer = {
            let guard = &guard;
            let resign_calls = &resign_calls;
            let hide_calls = &hide_calls;
            let reentered = &reentered;
            let mut resign = || {
                resign_calls.set(resign_calls.get() + 1);
                // 模拟 AppKit：resign 内同步派发通知，观察者回调重入隐藏
                if !reentered.get() {
                    reentered.set(true);
                    let inner = super::hide_once(
                        guard,
                        || resign_calls.set(resign_calls.get() + 1),
                        || {
                            hide_calls.set(hide_calls.get() + 1);
                            true
                        },
                    );
                    assert!(
                        inner,
                        "重入调用必须返回 true（隐藏已在外层进行中，视为完成）"
                    );
                }
            };
            let mut hide = || {
                hide_calls.set(hide_calls.get() + 1);
                true
            };
            super::hide_once(guard, &mut resign, &mut hide)
        };

        assert!(outer, "外层隐藏应返回 true");
        assert_eq!(
            resign_calls.get(),
            1,
            "resign 只应执行 1 次（内层不得重复 resign）"
        );
        assert_eq!(
            hide_calls.get(),
            1,
            "hide 只应执行 1 次（内层不得重复隐藏）"
        );
        assert!(!guard.load(Ordering::SeqCst), "守卫退出后必须复位为 false");
    }

    /// 守卫复位：正常隐藏结束后，下一次隐藏仍会真正执行（否则之后再也隐藏不了）。
    #[test]
    fn hide_once_allows_subsequent_hides_after_guard_resets() {
        use std::cell::Cell;
        use std::sync::atomic::{AtomicBool, Ordering};

        let guard = AtomicBool::new(false);
        let hide_calls = Cell::new(0);

        for _ in 0..3 {
            let done = {
                let hide_calls = &hide_calls;
                let mut hide = || {
                    hide_calls.set(hide_calls.get() + 1);
                    true
                };
                super::hide_once(&guard, || {}, &mut hide)
            };
            assert!(done);
            assert!(!guard.load(Ordering::SeqCst), "每次隐藏后都必须复位");
        }

        assert_eq!(hide_calls.get(), 3, "三次独立隐藏都应真正执行");
    }

    /// 异常路径复位：resign 里 panic 时守卫也要复位（否则之后再也隐藏不了）。
    #[test]
    fn hide_once_resets_guard_when_resign_panics() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let guard = AtomicBool::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            super::hide_once(&guard, || panic!("resign 抛错"), || true)
        }));

        assert!(result.is_err(), "resign panic 应向上传播");
        assert!(
            !guard.load(Ordering::SeqCst),
            "panic 之后守卫必须复位，否则此后再也不会隐藏面板"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn activate_app_macos_is_callable_as_fn_pointer() {
        // 将私有函数赋值给函数指针，确认签名为 fn()。
        // 此测试在 headless CI 中不调用 NSApplication，只验证函数存在且类型正确。
        let _f: fn() = super::activate_app_macos;
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn activate_app_macos_noop_on_non_macos() {
        // 非 macOS 平台的空实现应可被安全调用，无副作用。
        super::activate_app_macos();
    }

    /// 面板持有 key 时必须走「让出面板 key」路径：否则 ⌘V 会被面板自己吞掉。
    #[cfg(target_os = "macos")]
    #[test]
    fn paste_focus_plan_yields_panel_when_panel_holds_key() {
        assert_eq!(
            super::paste_focus_plan(true),
            super::PasteFocusPlan::YieldClipPanel
        );
    }

    /// 面板未持有 key（主窗口粘贴 / 面板降级）必须沿用旧的隐藏 app 路径：
    /// 否则从主窗口粘贴时前台应用不会被还焦，⌘V 会落到 QuickQuick 自己身上。
    #[cfg(target_os = "macos")]
    #[test]
    fn paste_focus_plan_falls_back_when_panel_not_key() {
        assert_eq!(
            super::paste_focus_plan(false),
            super::PasteFocusPlan::HideAppAndActivateTarget
        );
    }

    /// 面板路径交出失败（超时 / 派发失败）时必须放弃注入：此时 ⌘V 会被面板吞掉，
    /// 继续注入只会让界面报出一个兑现不了的成功。
    #[cfg(target_os = "macos")]
    #[test]
    fn should_inject_paste_skips_when_panel_yield_failed() {
        assert!(!super::should_inject_paste(
            super::PasteFocusPlan::YieldClipPanel,
            false
        ));
    }

    /// 面板路径交出成功时照常注入 ⌘V。
    #[cfg(target_os = "macos")]
    #[test]
    fn should_inject_paste_injects_when_panel_yield_succeeded() {
        assert!(super::should_inject_paste(
            super::PasteFocusPlan::YieldClipPanel,
            true
        ));
    }

    /// 主窗口旧路径没有可靠的交出信号，必须保持改造前「一律注入」的行为；
    /// 若按交出结果判定，从主窗口粘贴会静默退化成只写回。
    #[cfg(target_os = "macos")]
    #[test]
    fn should_inject_paste_keeps_main_window_path_unchanged() {
        for focus_yielded in [false, true] {
            assert!(
                super::should_inject_paste(
                    super::PasteFocusPlan::HideAppAndActivateTarget,
                    focus_yielded
                ),
                "主窗口路径交出结果={focus_yielded} 时仍须注入 ⌘V"
            );
        }
    }
}
