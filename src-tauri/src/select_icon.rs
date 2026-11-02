//! 选中图标（P2）：鼠标抬起后，若前台应用有选中文字，在光标旁冒出一个小图标；点击图标才翻译。
//!
//! 设计纪律（见 `docs/design/translate-interaction-alignment.md` §2 P2）：
//! - 只冒图标，绝不自动翻译；翻译只能由用户点击图标或按热键触发。
//! - 默认关闭（设置 `select_icon_enabled`）。只有开启后才会在鼠标抬起时读取 AX 选区。
//! - 只读选区，不模拟 Cmd+C、不写剪贴板。
//! - 图标是非激活面板：出现与点击都不让 QuickQuick 成为前台应用，源应用的选区保持不动。
//! - 只有 macOS 有取词通道与全局鼠标监听；其它平台的运行时入口都是空操作。
//!
//! 线程模型：全局鼠标监听回调在主线程执行。取词（跨进程 AX 调用，目标应用卡死时可能阻塞到超时）
//! 放到后台线程；结果回到主线程后才改动图标与待译状态，使「鼠标按下」与「状态落定」在主线程上串行。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

#[cfg(target_os = "macos")]
use tauri::WebviewWindow;
use tauri::{AppHandle, Manager};

use crate::ipc::trans_popover::build_trans_source;
use crate::popover;
#[cfg(target_os = "macos")]
use crate::selection;
use crate::selection::{ScreenRect, SelectionOutcome, SelectionSnapshot};
#[cfg(target_os = "macos")]
use crate::window_pos::{monitor_specs, place_for_platform, placement_near_rect};

/// 选中图标窗口 label。
pub const ICON_LABEL: &str = "select-icon";
/// 图标前端入口，与 vite 配置中的 rollup input 名一致。
const ICON_URL: &str = "src/select-icon/index.html";
const ICON_WIDTH: i32 = 28;
const ICON_HEIGHT: f64 = 28.0;
/// 鼠标抬起后等待应用把选区同步到 AX 的时长：给应用一个 runloop 周期，避免读到抬起前的旧选区。
#[cfg(target_os = "macos")]
const CAPTURE_DELAY: std::time::Duration = std::time::Duration::from_millis(120);
/// 图标冒出后的自动收起时长。用户若既不点击也不再拖选，图标不应一直悬在屏幕上。
#[cfg(target_os = "macos")]
const AUTO_HIDE_AFTER: std::time::Duration = std::time::Duration::from_secs(6);

/// 一次取词读到的待译内容，点击图标时直接用于翻译，不再重新取词。
#[derive(Debug, Clone, PartialEq)]
pub struct PendingSelection {
    /// 选中文字（已 trim，非空）。
    pub text: String,
    /// 选区边界（逻辑点）；取不到时为 `None`，浮窗回退到光标定位。
    pub bounds: Option<ScreenRect>,
}

/// 由一次取词快照得出待译内容；只有读到非空选中文字才有待译内容。
///
/// 无选中、不支持、未授权三种结局一律返回 `None`，即使快照里残留了边界：那份边界不对应任何文字，
/// 采信它会让图标或浮窗贴到与内容无关的位置。
#[must_use]
pub fn pending_from_snapshot(snapshot: SelectionSnapshot) -> Option<PendingSelection> {
    let bounds = snapshot.usable_bounds();
    match snapshot.outcome {
        SelectionOutcome::Text(text) => Some(PendingSelection { text, bounds }),
        SelectionOutcome::NoSelection
        | SelectionOutcome::Unsupported
        | SelectionOutcome::Untrusted => None,
    }
}

/// 图标锚点：光标处一条竖线，图标落在其下方并水平居中。
#[must_use]
pub fn cursor_anchor_rect(x: f64, y: f64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width: 1.0,
        height: 16.0,
    }
}

/// 选中图标的运行期状态：启用开关、代数计数与待译选区。
///
/// 代数用于作废过期结果：取词在后台进行期间，用户若又按下或抬起了鼠标，那次取词的结果已不代表
/// 屏幕上的选区，必须丢弃。
#[derive(Default)]
pub struct SelectIconState {
    enabled: AtomicBool,
    generation: AtomicU64,
    /// 锁中毒时视为「没有待译内容」：点击图标会安静地什么也不做，不影响其它功能。
    pending: Mutex<Option<PendingSelection>>,
}

impl SelectIconState {
    /// 以设置中的启用状态创建。
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            generation: AtomicU64::new(0),
            pending: Mutex::new(None),
        }
    }

    /// 功能是否开启。
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// 运行时切换开关（持久化由 IPC 层负责）。
    pub fn set_enabled(&self, value: bool) {
        self.enabled.store(value, Ordering::SeqCst);
    }

    /// 开始一轮新的鼠标动作，返回本轮代数；此前发出的取词结果全部失效。
    pub fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// 给定代数是否仍是最新一轮。
    pub fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }

    /// 记录待译内容（覆盖旧的）。
    pub fn remember(&self, pending: PendingSelection) {
        if let Ok(mut guard) = self.pending.lock() {
            *guard = Some(pending);
        }
    }

    /// 取走待译内容；取后即清空，同一次选中不会被翻译两次。
    pub fn take(&self) -> Option<PendingSelection> {
        self.pending.lock().ok().and_then(|mut guard| guard.take())
    }

    /// 丢弃待译内容。
    pub fn forget(&self) {
        if let Ok(mut guard) = self.pending.lock() {
            *guard = None;
        }
    }
}

/// 注册全局鼠标监听：抬起触发取词，按下收起图标。
///
/// 必须在主线程调用（setup 阶段即是）。两个监听对象与进程同生命周期、从不移除，故 forget。
/// 只有 macOS 实现；其它平台没有取词通道，直接不注册。
#[cfg(target_os = "macos")]
pub fn install_mouse_monitors(app: &AppHandle) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSEvent, NSEventMask};

    let up_app = app.clone();
    let up_block = RcBlock::new(move |_event: NonNull<NSEvent>| on_mouse_up(&up_app));
    let down_app = app.clone();
    let down_block = RcBlock::new(move |_event: NonNull<NSEvent>| on_mouse_down(&down_app));

    // 系统会 copy 这两个 block，其生命周期由系统管理；返回的 monitor 对象不会被移除，
    // 因而随进程存活，forget 不会造成泄漏以外的副作用。
    let up: Option<Retained<AnyObject>> =
        NSEvent::addGlobalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseUp, &up_block);
    let down: Option<Retained<AnyObject>> = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
        NSEventMask::LeftMouseDown,
        &down_block,
    );

    let mut registered = 0;
    for monitor in [up, down].into_iter().flatten() {
        registered += 1;
        std::mem::forget(monitor);
    }
    if registered < 2 {
        eprintln!(
            "[QuickQuick] 选中图标的全局鼠标监听注册不完整（{registered}/2），功能可能不可用"
        );
    }
}

/// 非 macOS 平台没有取词通道，不注册任何监听。
#[cfg(not(target_os = "macos"))]
pub fn install_mouse_monitors(_app: &AppHandle) {}

/// 鼠标抬起：若功能开启，后台取词，回到主线程后再决定冒图标还是收起。
#[cfg(target_os = "macos")]
fn on_mouse_up(app: &AppHandle) {
    let Some(state) = app.try_state::<SelectIconState>() else {
        return;
    };
    if !state.is_enabled() {
        return;
    }
    let generation = state.next_generation();
    let Some(cursor) = cursor_location() else {
        return;
    };

    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(CAPTURE_DELAY);
        // 醒来后先看代数：睡眠期间若又有鼠标动作，这次的 AX 读取就是白做，直接不读。
        // 这同时限制了目标应用卡死时线程与 AX 调用的堆积速度。
        let still_current = app
            .try_state::<SelectIconState>()
            .is_some_and(|state| state.is_current(generation));
        if !still_current {
            return;
        }
        let snapshot = selection::read_selection_snapshot();
        let app_for_ui = app.clone();
        if let Err(e) = app.run_on_main_thread(move || {
            apply_capture(&app_for_ui, generation, cursor, snapshot);
        }) {
            eprintln!("[QuickQuick] 选中图标派发主线程失败: {e}");
        }
    });
}

/// 鼠标按下：作废仍在进行的取词并收起图标。按下即说明用户开始了新动作，旧选区不再可信。
#[cfg(target_os = "macos")]
fn on_mouse_down(app: &AppHandle) {
    let Some(state) = app.try_state::<SelectIconState>() else {
        return;
    };
    if !state.is_enabled() {
        return;
    }
    state.next_generation();
    state.forget();
    hide_icon_now(app);
}

/// 主线程：取词结果落定。代数已过期则丢弃；有选中文字则记录并冒图标，否则收起。
#[cfg(target_os = "macos")]
fn apply_capture(
    app: &AppHandle,
    generation: u64,
    cursor: (f64, f64),
    snapshot: SelectionSnapshot,
) {
    let Some(state) = app.try_state::<SelectIconState>() else {
        return;
    };
    if !state.is_current(generation) {
        return;
    }
    match pending_from_snapshot(snapshot) {
        Some(pending) => {
            state.remember(pending);
            show_icon(app, cursor);
            schedule_auto_hide(app, generation);
        }
        None => {
            state.forget();
            hide_icon_now(app);
        }
    }
}

/// 到期自动收起图标。代数变了（又有鼠标动作或已被点击处理）说明图标已被更新或消费，不再动它。
#[cfg(target_os = "macos")]
fn schedule_auto_hide(app: &AppHandle, generation: u64) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(AUTO_HIDE_AFTER);
        let handle = app.clone();
        if let Err(e) = app.run_on_main_thread(move || {
            let still_current = handle
                .try_state::<SelectIconState>()
                .is_some_and(|state| state.is_current(generation));
            if still_current {
                if let Some(state) = handle.try_state::<SelectIconState>() {
                    state.forget();
                }
                hide_icon_now(&handle);
            }
        }) {
            eprintln!("[QuickQuick] 选中图标自动收起派发主线程失败: {e}");
        }
    });
}

/// 鼠标当前位置（全局坐标，逻辑点，左上原点，与 AX 选区坐标系一致）。
#[cfg(target_os = "macos")]
fn cursor_location() -> Option<(f64, f64)> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let point = event.location();
    Some((point.x, point.y))
}

/// 在光标下方冒出图标。必须主线程调用。
///
/// 光标不在任何显示器内时不显示：此时没有可靠的落点，宁可不冒。
#[cfg(target_os = "macos")]
fn show_icon(app: &AppHandle, cursor: (f64, f64)) {
    let Some(window) = get_or_create_icon_window(app) else {
        return;
    };
    let monitors = monitor_specs(&window);
    let anchor = cursor_anchor_rect(cursor.0, cursor.1);
    let window_size = (ICON_WIDTH, ICON_HEIGHT.round() as i32);
    let Some((logical, monitor)) = placement_near_rect(anchor, window_size, &monitors) else {
        return;
    };
    if let Err(e) = window.set_position(place_for_platform(logical, monitor)) {
        eprintln!("[QuickQuick] 选中图标 set_position 失败: {e}");
        return;
    }
    show_icon_panel(app);
}

/// 以非激活面板显示图标：只前置，不取 key、不激活 app。
///
/// 面板转换失败时不退化为普通窗口显示：普通窗口显示会激活 QuickQuick，源应用的焦点就丢了。
#[cfg(target_os = "macos")]
fn show_icon_panel(app: &AppHandle) {
    use tauri_nspanel::ManagerExt;

    match app.get_webview_panel(ICON_LABEL) {
        Ok(panel) => panel.order_front_regardless(),
        Err(_) => eprintln!("[QuickQuick] 选中图标面板不可用，本次不冒图标"),
    }
}

/// 收起图标。主线程或任意线程均可调用（内部派发）。
pub fn dismiss(app: &AppHandle) {
    let handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || {
        if let Some(state) = handle.try_state::<SelectIconState>() {
            state.next_generation();
            state.forget();
        }
        hide_icon_now(&handle);
    }) {
        eprintln!("[QuickQuick] 选中图标收起派发主线程失败: {e}");
    }
}

/// 隐藏图标窗口。必须主线程调用。
fn hide_icon_now(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(ICON_LABEL) {
        if let Err(e) = window.hide() {
            eprintln!("[QuickQuick] 选中图标隐藏失败: {e}");
        }
    }
}

/// 取得或创建图标窗口。窗口只建一次；macOS 上随即转为非激活面板。
#[cfg(target_os = "macos")]
fn get_or_create_icon_window(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(ICON_LABEL) {
        return Some(existing);
    }

    let window =
        tauri::WebviewWindowBuilder::new(app, ICON_LABEL, tauri::WebviewUrl::App(ICON_URL.into()))
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .resizable(false)
            // 面板不取 key，点击时源应用才是前台：不开启 accept_first_mouse，AppKit 会把
            // 第一次点击当成「激活窗口」吞掉，图标唯一的交互就失效。
            .accept_first_mouse(true)
            .focused(false)
            .inner_size(f64::from(ICON_WIDTH), ICON_HEIGHT)
            .build();

    match window {
        Ok(w) => {
            #[cfg(target_os = "macos")]
            convert_icon_to_panel(&w);
            Some(w)
        }
        Err(e) => {
            eprintln!("[QuickQuick] 选中图标窗口创建失败: {e}");
            None
        }
    }
}

#[cfg(target_os = "macos")]
mod icon_panel {
    use tauri_nspanel::tauri_panel;

    tauri_panel! {
        panel!(SelectIconPanel {
            config: {
                can_become_key_window: false,
                can_become_main_window: false,
                is_floating_panel: true
            }
        })
    }
}

/// 把图标窗口转为非激活面板。失败只记录：此时 `show_icon_panel` 会拒绝显示，而不是抢焦点。
///
/// `can_become_key_window: false`：图标不需要键盘，也不应拿走源应用的 key 状态。
#[cfg(target_os = "macos")]
fn convert_icon_to_panel(window: &WebviewWindow) {
    use tauri_nspanel::{PanelLevel, StyleMask, WebviewWindowExt};

    if !popover::on_main_thread() {
        eprintln!("[QuickQuick] 选中图标转面板不在主线程，本次不可用");
        return;
    }
    if !popover::panel_registered(window.app_handle()) {
        eprintln!("[QuickQuick] tauri-nspanel 插件未注册，选中图标不可用");
        return;
    }
    let panel = match window.to_panel::<icon_panel::SelectIconPanel>() {
        Ok(panel) => panel,
        Err(e) => {
            eprintln!("[QuickQuick] 选中图标转 NSPanel 失败: {e}");
            return;
        }
    };
    if let Err(e) = panel.add_style_mask(StyleMask::empty().nonactivating_panel().into()) {
        eprintln!("[QuickQuick] 选中图标追加 nonactivatingPanel 失败，本次不可用: {e}");
        return;
    }
    panel.set_level(PanelLevel::Dock.value());
    panel.set_collection_behavior(popover::shown_space_behavior());
}

/// 点击图标：取走待译选区，收起图标，以该选区唤起翻译浮窗。
///
/// 使用冒图标时的同一份快照，不重新取词：点击时再读既多一次 AX 往返，也会把两次读取混为一谈。
#[tauri::command]
pub fn select_icon_translate(app: AppHandle) {
    let handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || translate_pending(&handle)) {
        eprintln!("[QuickQuick] 选中图标翻译派发主线程失败: {e}");
    }
}

/// 主线程：消费待译选区并唤起翻译浮窗。没有待译内容时只收起图标。
fn translate_pending(app: &AppHandle) {
    let pending = app
        .try_state::<SelectIconState>()
        .and_then(|state| state.take());
    hide_icon_now(app);
    let Some(pending) = pending else {
        return;
    };
    let source = build_trans_source(SelectionOutcome::Text(pending.text), None);
    popover::trigger_translate_prepared(app, source, pending.bounds);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> ScreenRect {
        ScreenRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn pending_from_snapshot_keeps_text_and_bounds_when_selection_found() {
        // Arrange
        let bounds = rect(100.0, 200.0, 80.0, 16.0);
        let snapshot = SelectionSnapshot {
            outcome: SelectionOutcome::Text("hello".to_string()),
            bounds: Some(bounds),
        };

        // Act
        let pending = pending_from_snapshot(snapshot);

        // Assert
        assert_eq!(
            pending,
            Some(PendingSelection {
                text: "hello".to_string(),
                bounds: Some(bounds),
            })
        );
    }

    #[test]
    fn pending_from_snapshot_keeps_text_without_bounds_when_bounds_missing() {
        // Arrange
        let snapshot = SelectionSnapshot {
            outcome: SelectionOutcome::Text("hello".to_string()),
            bounds: None,
        };

        // Act
        let pending = pending_from_snapshot(snapshot);

        // Assert
        assert_eq!(
            pending,
            Some(PendingSelection {
                text: "hello".to_string(),
                bounds: None,
            })
        );
    }

    #[test]
    fn pending_from_snapshot_is_none_for_every_non_text_outcome_even_with_stale_bounds() {
        // Arrange：每种非文本结局都带一份残留边界，验证它不会被当成待译内容的锚点
        let stale = Some(rect(10.0, 10.0, 50.0, 12.0));
        let outcomes = [
            SelectionOutcome::NoSelection,
            SelectionOutcome::Unsupported,
            SelectionOutcome::Untrusted,
        ];

        for outcome in outcomes {
            let snapshot = SelectionSnapshot {
                outcome: outcome.clone(),
                bounds: stale,
            };

            // Act
            let pending = pending_from_snapshot(snapshot);

            // Assert
            assert_eq!(pending, None, "{outcome:?} 不应产生待译内容");
        }
    }

    #[test]
    fn cursor_anchor_rect_places_a_one_point_wide_line_at_cursor() {
        // Act
        let anchor = cursor_anchor_rect(320.5, 140.25);

        // Assert
        assert_eq!(anchor.x, 320.5);
        assert_eq!(anchor.y, 140.25);
        assert_eq!(anchor.width, 1.0);
        assert_eq!(anchor.height, 16.0);
    }

    #[test]
    fn state_starts_disabled_when_constructed_disabled() {
        // Arrange
        let state = SelectIconState::new(false);

        // Assert
        assert!(!state.is_enabled());
    }

    #[test]
    fn state_toggle_reflects_latest_value() {
        // Arrange
        let state = SelectIconState::new(false);

        // Act
        state.set_enabled(true);

        // Assert
        assert!(state.is_enabled());
    }

    #[test]
    fn next_generation_invalidates_previous_round() {
        // Arrange
        let state = SelectIconState::new(true);
        let first = state.next_generation();

        // Act
        let second = state.next_generation();

        // Assert
        assert!(!state.is_current(first), "旧一轮的取词结果必须作废");
        assert!(state.is_current(second));
    }

    #[test]
    fn take_returns_pending_once_then_none() {
        // Arrange
        let state = SelectIconState::new(true);
        state.remember(PendingSelection {
            text: "hello".to_string(),
            bounds: None,
        });

        // Act
        let first = state.take();
        let second = state.take();

        // Assert
        assert_eq!(first.map(|p| p.text), Some("hello".to_string()));
        assert_eq!(second, None, "同一次选中不应被翻译两次");
    }

    #[test]
    fn stale_capture_is_rejected_after_newer_mouse_action() {
        // Arrange：第一次抬起开始取词，随后又有一次按下，代数被推进
        let state = SelectIconState::new(true);
        let capture_generation = state.next_generation();
        state.next_generation();

        // Act：旧取词结果落定时，判定必须是「过期」
        let still_current = state.is_current(capture_generation);

        // Assert：过期结果不得被采信，也就不会把旧选区冒出来
        assert!(!still_current, "取词期间有新的鼠标动作，旧结果必须丢弃");
    }

    #[test]
    fn current_capture_is_accepted_when_no_newer_action() {
        // Arrange
        let state = SelectIconState::new(true);
        let capture_generation = state.next_generation();

        // Act
        let still_current = state.is_current(capture_generation);

        // Assert
        assert!(still_current, "无新动作时取词结果应被采信");
    }

    #[test]
    fn forget_clears_pending_selection() {
        // Arrange
        let state = SelectIconState::new(true);
        state.remember(PendingSelection {
            text: "hello".to_string(),
            bounds: None,
        });

        // Act
        state.forget();

        // Assert
        assert_eq!(state.take(), None);
    }
}
