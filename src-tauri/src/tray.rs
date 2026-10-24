//! 系统托盘模块：构建托盘菜单并绑定事件处理
//!
//! 策略：托盘由本模块 setup_tray() 唯一构建（带右键菜单+事件回调）；
//! tauri.conf.json 不声明 app.trayIcon，避免"配置自动建 + 代码建"双图标。
//! 图标使用专用单色模板图 tray.png，通过 include_bytes! 编译期嵌入二进制，
//! dev/prod 均从内存读取，不依赖 resource_dir 路径。以 icon_as_template(true)
//! 告知 macOS 此为 template image，系统自动按菜单栏明/暗模式反色渲染。
//!
//! 菜单项：
//! - "显示 QuickQuick" → show + set_focus
//! - "暂停捕获" / "恢复捕获"（随暂停态换文案）→ 切换暂停
//! - "退出" → app.exit(0)
//!
//! 左键点击托盘图标按**点击时刻的修饰键**分派（语义由
//! [`crate::privacy::tray_click_action`] 这个纯函数决定，本模块只负责读修饰键）：
//! - 无修饰 → show + set_focus（与"显示"菜单项等效）
//! - `⌥`    → 切换暂停捕获（持久化，与设置页开关同一实现）
//! - `⌥⇧`   → 只忽略下一次剪贴板变化（一次性，不写 settings.json）
//!
//! 暂停态与「仅忽略下一次」都要**可见**：tooltip 与暂停菜单项文案都会跟着变，
//! 否则用户按了 `⌥` 却看不到任何反馈，会以为快捷键失效。

use std::sync::atomic::Ordering;

use tauri::{
    image::Image,
    menu::{Menu, MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime,
};

use crate::privacy::{tray_click_action, TrayClickAction, TrayModifiers};

/// 托盘图标 id：刷新暂停/一次性忽略状态时按此 id 找回同一个托盘实例。
const TRAY_ID: &str = "main";
/// 菜单项 id（菜单会被重建，事件靠 id 而非对象身份分发）
const MENU_ID_SHOW: &str = "show";
const MENU_ID_PAUSE: &str = "pause";
const MENU_ID_QUIT: &str = "quit";
/// 常态提示文案
const TOOLTIP_IDLE: &str = "QuickQuick";

/// 托盘提示文案：让暂停态与「仅忽略下一次」都有可见反馈。
///
/// 一次性忽略优先展示（它更短暂、更不容易被用户记住），且带上恢复入口的提示。
#[must_use]
fn tooltip_text(paused: bool, skip_once_armed: bool) -> &'static str {
    if skip_once_armed {
        "QuickQuick（仅忽略下一次变化）"
    } else if paused {
        "QuickQuick（已暂停捕获，⌥ 点击恢复）"
    } else {
        TOOLTIP_IDLE
    }
}

/// 暂停菜单项文案：暂停时给出「恢复捕获」这个动作名，而不是重复状态。
#[must_use]
fn pause_item_text(paused: bool) -> &'static str {
    if paused {
        "恢复捕获"
    } else {
        "暂停捕获"
    }
}

/// 构建托盘菜单（暂停项文案随暂停态变化）。
///
/// 每次状态变化都整体重建菜单而不是持有 `MenuItem` 句柄：
/// 菜单项 id 不变，事件分发靠 id，重建不影响点击行为，也不需要在应用状态里
/// 存带运行时泛型的菜单对象。
fn build_menu<R: Runtime, M: Manager<R>>(manager: &M, paused: bool) -> tauri::Result<Menu<R>> {
    let show_item = MenuItemBuilder::new("显示 QuickQuick")
        .id(MENU_ID_SHOW)
        .build(manager)?;
    let pause_item = MenuItemBuilder::new(pause_item_text(paused))
        .id(MENU_ID_PAUSE)
        .build(manager)?;
    let quit_item = MenuItemBuilder::new("退出")
        .id(MENU_ID_QUIT)
        .build(manager)?;

    MenuBuilder::new(manager)
        .items(&[&show_item, &pause_item, &quit_item])
        .build()
}

/// 在 setup 阶段构建系统托盘图标与菜单。
///
/// # Errors
/// 若菜单或托盘构建失败则返回错误（由 Tauri 统一处理，不会 panic）。
pub fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    // 启动时按持久化设置还原暂停态，让托盘一上来就说真话
    let paused = current_paused(app.handle());
    let menu = build_menu(app, paused)?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip(tooltip_text(paused, false))
        .on_menu_event(|app_handle, event| {
            handle_menu_event(app_handle, event.id.as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            handle_tray_icon_event(tray.app_handle(), &event);
        });

    // tray.png 在编译期嵌入二进制，dev/prod 均从内存读取，不依赖 resource_dir。
    // icon_as_template(true) 告知 macOS 这是 template image，
    // 系统会自动按当前菜单栏明/暗模式将黑色像素反色为白色（或保持黑色），
    // 无需应用层手动处理深色模式切换。
    match Image::from_bytes(include_bytes!("../icons/tray.png")) {
        Ok(icon) => {
            builder = builder.icon(icon).icon_as_template(true);
        }
        Err(e) => {
            // 极少发生（编译期已嵌入有效 PNG），回退到应用图标但不套 template，
            // 避免彩色图标经 template 渲染后变成实心白块。
            eprintln!("[QuickQuick] 托盘：加载 tray.png 失败，回退到应用图标: {e}");
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
        }
    }

    // 构建托盘；Tauri 将其生命周期与 app 绑定，build 后无需显式持有
    let _tray = builder.build(app)?;

    Ok(())
}

/// 处理托盘菜单点击事件。
fn handle_menu_event(app: &tauri::AppHandle, item_id: &str) {
    match item_id {
        MENU_ID_SHOW => show_and_focus_window(app),
        MENU_ID_PAUSE => toggle_pause_capture(app),
        MENU_ID_QUIT => app.exit(0),
        _ => {}
    }
}

/// 处理托盘图标自身的事件（左键单击 → 按点击时刻的修饰键分派）。
fn handle_tray_icon_event(app: &tauri::AppHandle, event: &TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        match tray_click_action(current_modifiers()) {
            TrayClickAction::Show => show_and_focus_window(app),
            TrayClickAction::TogglePause => toggle_pause_capture(app),
            TrayClickAction::SkipNextOnce => arm_skip_next_once(app),
        }
    }
}

/// 读取**点击时刻**的修饰键状态。
///
/// 必须在托盘点击回调里调用：那里才是「用户按下那一刻」。
/// macOS 读 `NSEvent` 的类方法（全局当前修饰键，与具体事件对象无关）；
/// Windows 读 `GetKeyState`（同样反映调用时刻的键盘状态）。
#[cfg(target_os = "macos")]
fn current_modifiers() -> TrayModifiers {
    use objc2_app_kit::{NSEvent, NSEventModifierFlags};

    // 类方法 `+[NSEvent modifierFlags]`：不依赖事件对象，返回当前修饰键位掩码。
    // 托盘点击回调由 AppKit 在主线程序列上派发，此处不引入跨线程 NSEvent 调用。
    let flags = NSEvent::modifierFlags_class();
    TrayModifiers {
        alt: flags.contains(NSEventModifierFlags::Option),
        shift: flags.contains(NSEventModifierFlags::Shift),
    }
}

/// 读取**点击时刻**的修饰键状态（Windows 实现）。
///
/// `GetKeyState` 返回 i16，最高位（负值）为 1 表示该键当前按下。
/// 用 `VK_MENU` / `VK_SHIFT`（左右键的聚合虚拟键）而不是 `VK_LMENU` / `VK_LSHIFT`：
/// 用户按右 Alt 时也该生效。
#[cfg(target_os = "windows")]
fn current_modifiers() -> TrayModifiers {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_MENU, VK_SHIFT};

    let is_down = |virtual_key: u16| unsafe { GetKeyState(i32::from(virtual_key)) } < 0;
    TrayModifiers {
        alt: is_down(VK_MENU),
        shift: is_down(VK_SHIFT),
    }
}

/// 其它平台没有本功能的修饰键语义：一律按「无修饰」处理，点击即显示窗口。
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn current_modifiers() -> TrayModifiers {
    TrayModifiers::default()
}

/// 读取当前暂停状态（CaptureState 未注册时按「未暂停」处理，不 panic）。
fn current_paused(app: &tauri::AppHandle) -> bool {
    app.try_state::<crate::CaptureState>()
        .is_some_and(|state| state.paused.load(Ordering::Relaxed))
}

/// 切换暂停捕获（托盘 `⌥` 点击与「暂停捕获/恢复捕获」菜单项共用）。
///
/// 走与设置页完全相同的实现（[`crate::ipc::settings::set_pause_capture_impl`]）：
/// 写运行时原子 + 持久化 `settings.json`。两处共用一份实现，语义不会分叉。
/// 持久化失败不影响本次切换生效（只留痕），避免「点了没反应」。
fn toggle_pause_capture(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<crate::CaptureState>() else {
        eprintln!("[QuickQuick] 托盘：无法读取捕获状态，暂停切换被忽略");
        return;
    };
    let next = !state.paused.load(Ordering::Relaxed);

    match crate::ipc::settings::resolve_config_dir(app) {
        Ok(dir) => {
            let path = dir.join("settings.json");
            if let Err(e) = crate::ipc::settings::set_pause_capture_impl(next, &state, &path) {
                eprintln!("[QuickQuick] 托盘：暂停状态持久化失败（本次切换仍生效）: {e}");
            }
        }
        Err(e) => {
            // 拿不到配置目录时仍切换运行时状态：用户要的是「现在别记」，
            // 持久化只是下次启动的便利，不能因为磁盘问题让点击看不见效果。
            state.paused.store(next, Ordering::Relaxed);
            eprintln!("[QuickQuick] 托盘：无法解析配置目录，暂停状态未持久化: {e}");
        }
    }

    let skip_once_armed = state.skip_once.is_armed();
    refresh_tray_indicator(app, next, skip_once_armed);
}

/// 置位「仅忽略下一次」：只改内存里的一次性开关，**不碰** `settings.json`、不改 `paused`。
fn arm_skip_next_once(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<crate::CaptureState>() else {
        eprintln!("[QuickQuick] 托盘：无法读取捕获状态，一次性忽略被忽略");
        return;
    };
    state.skip_once.arm();
    refresh_tray_indicator(app, state.paused.load(Ordering::Relaxed), true);
}

/// 刷新托盘可见状态（tooltip + 暂停菜单项文案）。
///
/// 三个调用时机：托盘 `⌥`/`⌥⇧` 点击之后、设置页切换暂停之后、
/// 轮询线程真的用掉一次「仅忽略下一次」之后（否则提示会一直停在旧状态）。
/// 失败只留痕：托盘只是可见性通道，不能因它影响捕获逻辑。
pub(crate) fn refresh_tray_indicator(app: &tauri::AppHandle, paused: bool, skip_once_armed: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        eprintln!("[QuickQuick] 托盘：刷新状态时找不到托盘实例（id={TRAY_ID}）");
        return;
    };

    if let Err(e) = tray.set_tooltip(Some(tooltip_text(paused, skip_once_armed))) {
        eprintln!("[QuickQuick] 托盘：更新提示文案失败: {e}");
    }
    match build_menu(app, paused) {
        Ok(menu) => {
            if let Err(e) = tray.set_menu(Some(menu)) {
                eprintln!("[QuickQuick] 托盘：更新菜单文案失败: {e}");
            }
        }
        Err(e) => eprintln!("[QuickQuick] 托盘：重建菜单失败: {e}"),
    }
}

/// 显示 main 窗口并设置焦点；失败时记录日志不 panic。
///
/// 可见性为 `pub(crate)` 供 single-instance 插件回调复用：第二个实例启动时，
/// 已运行实例需执行与托盘「显示 QuickQuick」一致的显示行为，复用此函数避免重复实现。
pub(crate) fn show_and_focus_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        eprintln!("[QuickQuick] 托盘：找不到 main 窗口");
        return;
    };
    // Windows：show 前快照当前外部前台窗口，供粘贴还焦时 SetForegroundWindow 还原。
    #[cfg(target_os = "windows")]
    crate::frontmost::capture_foreground_window(app);
    if let Err(e) = window.show() {
        eprintln!("[QuickQuick] 托盘：显示窗口失败: {e}");
        return;
    }
    // 顺序关键：必须先 activate 进程，再 set_focus。Accessory 策略下进程不在常规
    // 激活序列里，仅靠 set_focus（底层是 macOS 14+ 已废弃的 activateIgnoringOtherApps:）
    // 无法让窗口拿到 key 状态，键盘事件进不了 webview（搜索框打不了字）。
    // 复用 popover 已验证的激活实现（见 popover.rs:83-89 与设计文档 4.2）。
    #[cfg(target_os = "macos")]
    crate::popover::activate_app_macos();
    if let Err(e) = window.set_focus() {
        eprintln!("[QuickQuick] 托盘：设置焦点失败: {e}");
    }
}

#[cfg(test)]
mod tests {
    // 与 popover 模块的 FFI 测试边界一致：activate_app_macos 是纯 NSApplication 包装，
    // headless 下不真正调用（sharedApplication 无 NSApp 时行为未定义）。此处只做
    // 编译期/类型守卫——确认 tray 能把 popover 的激活函数当作 `fn()` 引用，
    // 即 pub(crate) 提升成立、跨模块复用单一实现（DRY）。真实激活+键盘焦点行为
    // 归真机 manual_confirm（见设计文档 M2）。

    /// tray 可将 popover 的 activate_app_macos 作为 fn() 引用：守卫 pub(crate) 可见性，
    /// 确保 show_and_focus_window 复用 popover 的单一激活实现而非另造 FFI。
    #[cfg(target_os = "macos")]
    #[test]
    fn popover_activate_fn_is_referenceable_by_tray() {
        let _f: fn() = crate::popover::activate_app_macos;
    }

    // ===== 可见性文案（P3-6）=====

    use super::{pause_item_text, tooltip_text};

    #[test]
    fn tooltip_reports_idle_state_by_default() {
        assert_eq!(tooltip_text(false, false), super::TOOLTIP_IDLE);
    }

    #[test]
    fn tooltip_reflects_paused_state() {
        // 暂停必须看得见——否则用户按了 ⌥ 会以为快捷键失效
        assert_eq!(
            tooltip_text(true, false),
            "QuickQuick（已暂停捕获，⌥ 点击恢复）"
        );
    }

    #[test]
    fn tooltip_prefers_skip_once_over_paused() {
        // 一次性忽略更短暂、更容易被忘掉，优先展示
        assert_eq!(tooltip_text(true, true), "QuickQuick（仅忽略下一次变化）");
        assert_eq!(tooltip_text(false, true), "QuickQuick（仅忽略下一次变化）");
    }

    #[test]
    fn pause_menu_item_names_the_next_action() {
        assert_eq!(pause_item_text(false), "暂停捕获");
        assert_eq!(pause_item_text(true), "恢复捕获");
    }
}
