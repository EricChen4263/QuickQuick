//! 系统托盘模块：构建托盘菜单并绑定事件处理
//!
//! 策略：托盘由本模块 setup_tray() 唯一构建（带右键菜单+事件回调）；
//! tauri.conf.json 不声明 app.trayIcon，避免"配置自动建 + 代码建"双图标。
//! 图标使用专用单色模板图 tray.png（非应用图标），以 icon_as_template(true)
//! 告知 macOS 此为 template image，系统自动按菜单栏明/暗模式反色渲染。
//!
//! 菜单项：
//! - "显示 QuickQuick" → show + set_focus
//! - "退出" → app.exit(0)
//!
//! 左键点击托盘图标 → show + set_focus（与"显示"菜单项等效）

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

/// 在 setup 阶段构建系统托盘图标与菜单。
///
/// # Errors
/// 若菜单或托盘构建失败则返回错误（由 Tauri 统一处理，不会 panic）。
pub fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show_item = MenuItemBuilder::new("显示 QuickQuick")
        .id("show")
        .build(app)?;
    let quit_item = MenuItemBuilder::new("退出").id("quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&show_item, &quit_item])
        .build()?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("QuickQuick")
        .on_menu_event(|app_handle, event| {
            handle_menu_event(app_handle, event.id.as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            handle_tray_icon_event(tray.app_handle(), &event);
        });

    // 加载专用单色托盘模板图（tray.png），而非应用图标。
    // icon_as_template(true) 告知 macOS 这是 template image，
    // 系统会自动按当前菜单栏明/暗模式将黑色像素反色为白色（或保持黑色），
    // 无需应用层手动处理深色模式切换。
    let tray_icon = Image::from_path(
        app.path()
            .resource_dir()
            .expect("resource_dir should exist")
            .join("icons/tray.png"),
    )
    .or_else(|_| {
        // 开发模式下 resource_dir 可能指向不同位置，回退到 default_window_icon
        app.default_window_icon()
            .cloned()
            .ok_or_else(|| tauri::Error::AssetNotFound("tray.png".into()))
    });

    if let Ok(icon) = tray_icon {
        builder = builder.icon(icon).icon_as_template(true);
    } else if let Some(icon) = app.default_window_icon() {
        // 兜底：若模板图加载失败，回退到应用图标（不设 template）
        builder = builder.icon(icon.clone());
    }

    // 构建托盘；Tauri 将其生命周期与 app 绑定，build 后无需显式持有
    let _tray = builder.build(app)?;

    Ok(())
}

/// 处理托盘菜单点击事件。
fn handle_menu_event(app: &tauri::AppHandle, item_id: &str) {
    match item_id {
        "show" => show_and_focus_window(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// 处理托盘图标自身的事件（左键单击 → 显示窗口）。
fn handle_tray_icon_event(app: &tauri::AppHandle, event: &TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        show_and_focus_window(app);
    }
}

/// 显示 main 窗口并设置焦点；失败时记录日志不 panic。
fn show_and_focus_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        eprintln!("[QuickQuick] 托盘：找不到 main 窗口");
        return;
    };
    if let Err(e) = window.show() {
        eprintln!("[QuickQuick] 托盘：显示窗口失败: {e}");
        return;
    }
    if let Err(e) = window.set_focus() {
        eprintln!("[QuickQuick] 托盘：设置焦点失败: {e}");
    }
}
