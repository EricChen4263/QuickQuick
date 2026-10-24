//! 系统托盘模块：构建托盘菜单并绑定事件处理
//!
//! 策略：托盘由本模块 setup_tray() 唯一构建（带右键菜单+事件回调）；
//! tauri.conf.json 不声明 app.trayIcon，避免"配置自动建 + 代码建"双图标。
//! 图标使用专用单色模板图 tray.png，通过 include_bytes! 编译期嵌入二进制，
//! dev/prod 均从内存读取，不依赖 resource_dir 路径。以 icon_as_template(true)
//! 告知 macOS 此为 template image，系统自动按菜单栏明/暗模式反色渲染。
//!
//! 菜单结构（顺序见 [`menu_entries`]，它是唯一真源）：
//! - "显示 QuickQuick" → show + set_focus
//! - "暂停捕获" / "恢复捕获"（随暂停态换文案）→ 切换暂停
//! - "打开设置" → 主窗口前置 + 路由到设置页
//! - "检查更新" / "正在检查更新…" / "已是最新版本" / … → 走既有 updater 链路
//! - "QuickQuick v<当前版本>"（disabled 只读项）→ 展示真实打包版本
//! - "退出" → app.exit(0)
//!
//! 左键点击托盘图标按**点击时刻的修饰键**分派（语义由
//! [`crate::privacy::tray_click_action`] 这个纯函数决定，本模块只负责读修饰键）：
//! - 无修饰 → show + set_focus（与"显示"菜单项等效）
//! - `⌥`    → 切换暂停捕获（持久化，与设置页开关同一实现）
//! - `⌥⇧`   → 只忽略下一次剪贴板变化（一次性，不写 settings.json）
//!
//! 暂停态、「仅忽略下一次」与更新检查结果都要**可见**：tooltip 与菜单项文案都会
//! 跟着变，否则用户点了却看不到任何反馈，会以为功能失效。
//!
//! 可单测性：菜单结构、文案、版本号格式与更新检查状态机全部抽成纯函数/纯数据
//! （[`menu_entries`]、[`version_label`]、[`next_status`] 等），事件闭包只做接线。

use std::sync::atomic::Ordering;
use std::sync::Mutex;

use tauri::{
    image::Image,
    menu::{Menu, MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Runtime,
};

use crate::privacy::{tray_click_action, TrayClickAction, TrayModifiers};

/// 托盘图标 id：刷新暂停/一次性忽略/更新状态时按此 id 找回同一个托盘实例。
const TRAY_ID: &str = "main";

/// 菜单项 id（菜单会被重建，事件靠 id 而非对象身份分发）
const MENU_ID_SHOW: &str = "show";
const MENU_ID_PAUSE: &str = "pause";
const MENU_ID_OPEN_SETTINGS: &str = "open-settings";
const MENU_ID_CHECK_UPDATE: &str = "check-update";
const MENU_ID_VERSION: &str = "version";
const MENU_ID_QUIT: &str = "quit";

/// 应用名（提示文案与版本号条目共用，避免品牌名散落多处）
const APP_NAME: &str = "QuickQuick";

/// 常态提示文案
const TOOLTIP_IDLE: &str = APP_NAME;

/// 主窗口路由事件名：托盘「打开设置」据此让前端切到设置页。
///
/// 与前端 `src/ipc/events.ts` 的 `ROUTE_EVENT` 必须一致。Tauri 事件名跨语言
/// 无法编译期共享，故两侧各留一条断言钉住字面量。
const ROUTE_EVENT: &str = "route";

/// 路由事件 payload：要求主窗口切到设置页（前端 `RoutePayload` 的 settings 分支）。
const ROUTE_SETTINGS: &str = "settings";

/// 托盘提示文案：暂停态、「仅忽略下一次」与更新检查结果组合成一条可见反馈。
///
/// 优先级沿用既有语义：一次性忽略比暂停更短暂、更容易被忘掉，故替代暂停提示；
/// 更新检查结果追加在后（它是最近一次发生的动作），多段用「；」拼进同一对括号。
/// 三段都没有时退化为常态文案。
#[must_use]
fn tooltip_text(paused: bool, skip_once_armed: bool, update: &UpdateStatus) -> String {
    let mut hints: Vec<String> = Vec::new();
    if skip_once_armed {
        hints.push("仅忽略下一次变化".to_string());
    } else if paused {
        hints.push("已暂停捕获，⌥ 点击恢复".to_string());
    }
    if let Some(detail) = update_detail(update) {
        hints.push(detail);
    }

    if hints.is_empty() {
        TOOLTIP_IDLE.to_string()
    } else {
        format!("{APP_NAME}（{}）", hints.join("；"))
    }
}

/// 更新检查结果的短文案（tooltip 用）；无结果时不追加任何内容。
fn update_detail(update: &UpdateStatus) -> Option<String> {
    match update {
        UpdateStatus::Idle => None,
        UpdateStatus::Checking => Some("正在检查更新…".to_string()),
        UpdateStatus::UpToDate => Some("已是最新版本".to_string()),
        UpdateStatus::Available(version) => Some(format!("发现新版本 v{version}，可在设置页下载")),
        // 后端错误文案已经自解释（如「检查更新失败：网络错误」），原样透出便于用户
        // 判断是网络问题还是更新服务未配置。
        UpdateStatus::Failed(message) => Some(message.clone()),
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

/// 一次更新检查的生命周期状态。
///
/// 托盘是「检查更新」唯一会回显结果的通道，故状态必须记住上一次结论：
/// 无更新/失败都要有用户可见文案，不能静默什么都不发生。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum UpdateStatus {
    /// 从未检查过（或结果已不再展示）
    #[default]
    Idle,
    /// 正在检查：菜单项禁用，禁止重复点击
    Checking,
    /// 已是最新版本
    UpToDate,
    /// 发现可用新版本（携带新版本号）
    Available(String),
    /// 检查失败（携带后端给出的中文原因）
    Failed(String),
}

/// 更新检查状态机可接收的事件。
#[derive(Debug, Clone)]
enum UpdateEvent {
    /// 用户点了「检查更新」
    Started,
    /// 一次检查结束：`Ok(Some(version))` 有新版、`Ok(None)` 已是最新、`Err(msg)` 失败
    Finished(Result<Option<String>, String>),
}

/// 更新检查状态迁移（纯函数，生产路径真实调用）。
///
/// 语义：
/// - `Started`：任何非「检查中」状态都进入 `Checking`；已在检查中则保持不动
///   （调用方用 [`can_start_update_check`] 提前拦截，这里只是兜底）。
/// - `Finished`：只有「检查中」才接受结果；其余状态说明这不是当前这轮检查的回执
///   （陈旧响应），保持原状不覆盖。
fn next_status(current: &UpdateStatus, event: UpdateEvent) -> UpdateStatus {
    match (current, event) {
        (_, UpdateEvent::Started) => UpdateStatus::Checking,
        (UpdateStatus::Checking, UpdateEvent::Finished(Ok(Some(version)))) => {
            UpdateStatus::Available(version)
        }
        (UpdateStatus::Checking, UpdateEvent::Finished(Ok(None))) => UpdateStatus::UpToDate,
        (UpdateStatus::Checking, UpdateEvent::Finished(Err(message))) => {
            UpdateStatus::Failed(message)
        }
        (current, UpdateEvent::Finished(_)) => current.clone(),
    }
}

/// 现在能否发起一次更新检查：检查进行中禁止重复点击。
#[must_use]
fn can_start_update_check(current: &UpdateStatus) -> bool {
    !matches!(current, UpdateStatus::Checking)
}

/// 「检查更新」菜单项文案：结果留在菜单上，用户不用回忆上次点了什么。
#[must_use]
fn update_item_label(update: &UpdateStatus) -> String {
    match update {
        UpdateStatus::Idle => "检查更新".to_string(),
        UpdateStatus::Checking => "正在检查更新…".to_string(),
        UpdateStatus::UpToDate => "已是最新版本".to_string(),
        UpdateStatus::Available(version) => format!("发现新版本 v{version}"),
        UpdateStatus::Failed(_) => "检查更新失败，点击重试".to_string(),
    }
}

/// 「检查更新」菜单项是否可点击：检查中禁用（失败后仍可点击=重试）。
#[must_use]
fn update_item_enabled(update: &UpdateStatus) -> bool {
    can_start_update_check(update)
}

/// 菜单里的版本号展示文案，形如 `QuickQuick v1.2.3`。
///
/// 版本号来自 `app.package_info().version`（真实打包版本）；这里兜一次前导 `v`，
/// 避免上游带上 v 时出现 `vv0.3.8`。
#[must_use]
fn version_label(version: &str) -> String {
    format!("{APP_NAME} v{}", version.trim_start_matches('v'))
}

/// 托盘菜单的一项：分隔线，或带 id/文案/启用位的条目（纯数据，便于单测）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum MenuEntry {
    /// 菜单分隔线
    Separator,
    /// 可点击或只读的条目（`enabled == false` 时仅展示，如版本号）
    Item {
        /// 菜单项 id（事件分发靠它）
        id: &'static str,
        /// 展示文案
        label: String,
        /// 是否可点击
        enabled: bool,
    },
}

impl MenuEntry {
    /// 构造一个菜单条目。
    fn item(id: &'static str, label: impl Into<String>, enabled: bool) -> Self {
        Self::Item {
            id,
            label: label.into(),
            enabled,
        }
    }
}

/// 构建托盘菜单的全部条目（顺序、文案、启用位的唯一真源）。
///
/// 「版本号」是 disabled 只读项：它只陈述事实、不承载动作，放在「退出」之前的
/// 独立分组里，符合「关于」类信息的位置预期。
fn menu_entries(paused: bool, version: &str, update: &UpdateStatus) -> Vec<MenuEntry> {
    vec![
        MenuEntry::item(MENU_ID_SHOW, "显示 QuickQuick", true),
        MenuEntry::item(MENU_ID_PAUSE, pause_item_text(paused), true),
        MenuEntry::Separator,
        MenuEntry::item(MENU_ID_OPEN_SETTINGS, "打开设置", true),
        MenuEntry::item(
            MENU_ID_CHECK_UPDATE,
            update_item_label(update),
            update_item_enabled(update),
        ),
        MenuEntry::Separator,
        MenuEntry::item(MENU_ID_VERSION, version_label(version), false),
        MenuEntry::Separator,
        MenuEntry::item(MENU_ID_QUIT, "退出", true),
    ]
}

/// 按 [`menu_entries`] 的纯数据构建原生菜单。
///
/// 每次状态变化都整体重建菜单而不是持有 `MenuItem` 句柄：
/// 菜单项 id 不变，事件分发靠 id，重建不影响点击行为，也不需要在应用状态里
/// 存带运行时泛型的菜单对象。
fn build_menu<R: Runtime, M: Manager<R>>(
    manager: &M,
    paused: bool,
    version: &str,
    update: &UpdateStatus,
) -> tauri::Result<Menu<R>> {
    let mut builder = MenuBuilder::new(manager);
    for entry in menu_entries(paused, version, update) {
        builder = match entry {
            MenuEntry::Separator => builder.separator(),
            MenuEntry::Item { id, label, enabled } => {
                let item = MenuItemBuilder::new(label)
                    .id(id)
                    .enabled(enabled)
                    .build(manager)?;
                builder.item(&item)
            }
        };
    }
    builder.build()
}

/// 托盘可见状态的共享真源：更新检查状态。
///
/// 用 `Mutex` 而非原子：更新状态携带字符串（版本号/错误原因），无法用原子表达。
/// 写入只发生在「用户点检查更新」与「一次检查结束」两处，读取只在重建菜单时，
/// 竞争压力可忽略。谁能写：仅 [`apply_update_event`]；其余路径只读快照。
pub(crate) struct TrayState {
    /// 当前更新检查状态
    update: Mutex<UpdateStatus>,
}

impl TrayState {
    /// 读取当前更新检查状态（克隆快照，避免持锁跨越菜单/托盘调用）。
    fn update_status(&self) -> UpdateStatus {
        // 锁中毒只可能因为持锁线程 panic；此时状态仍是上一次可信快照，
        // 取回内部值继续用，避免托盘因一次 panic 永久失去刷新能力。
        self.update
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// 覆盖当前更新检查状态。
    fn set_update_status(&self, status: UpdateStatus) {
        *self
            .update
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = status;
    }
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            update: Mutex::new(UpdateStatus::Idle),
        }
    }
}

/// 读取当前更新检查状态（状态未注册时按 Idle 处理，不 panic）。
fn update_status_of(app: &tauri::AppHandle) -> UpdateStatus {
    app.try_state::<TrayState>()
        .map_or(UpdateStatus::Idle, |state| state.update_status())
}

/// 在 setup 阶段构建系统托盘图标与菜单。
///
/// # Errors
/// 若菜单或托盘构建失败则返回错误（由 Tauri 统一处理，不会 panic）。
pub fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    app.manage(TrayState::default());

    // 启动时按持久化设置还原暂停态，让托盘一上来就说真话
    let paused = current_paused(app.handle());
    let version = app.package_info().version.to_string();
    let update = UpdateStatus::Idle;
    let menu = build_menu(app, paused, &version, &update)?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip(tooltip_text(paused, false, &update))
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
        MENU_ID_OPEN_SETTINGS => open_settings(app),
        MENU_ID_CHECK_UPDATE => start_update_check(app),
        MENU_ID_QUIT => app.exit(0),
        // 版本号是 disabled 只读项（不会派发事件）；未知 id 一律忽略
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

    refresh_tray_from_state(app);
}

/// 置位「仅忽略下一次」：只改内存里的一次性开关，**不碰** `settings.json`、不改 `paused`。
fn arm_skip_next_once(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<crate::CaptureState>() else {
        eprintln!("[QuickQuick] 托盘：无法读取捕获状态，一次性忽略被忽略");
        return;
    };
    state.skip_once.arm();
    refresh_tray_from_state(app);
}

/// 按共享状态刷新托盘可见状态（暂停态 + 一次性忽略 + 更新检查结果）。
fn refresh_tray_from_state(app: &tauri::AppHandle) {
    let paused = current_paused(app);
    let skip_once_armed = app
        .try_state::<crate::CaptureState>()
        .is_some_and(|state| state.skip_once.is_armed());
    refresh_tray_indicator(app, paused, skip_once_armed);
}

/// 刷新托盘可见状态（tooltip + 菜单文案与启用位）。
///
/// 三个调用时机：托盘 `⌥`/`⌥⇧` 点击之后、设置页切换暂停之后、
/// 轮询线程真的用掉一次「仅忽略下一次」之后（否则提示会一直停在旧状态）；
/// 更新检查的状态变化也复用本函数。
/// 失败只留痕：托盘只是可见性通道，不能因它影响捕获逻辑。
pub(crate) fn refresh_tray_indicator(app: &tauri::AppHandle, paused: bool, skip_once_armed: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        eprintln!("[QuickQuick] 托盘：刷新状态时找不到托盘实例（id={TRAY_ID}）");
        return;
    };

    let update = update_status_of(app);
    if let Err(e) = tray.set_tooltip(Some(tooltip_text(paused, skip_once_armed, &update))) {
        eprintln!("[QuickQuick] 托盘：更新提示文案失败: {e}");
    }
    let version = app.package_info().version.to_string();
    match build_menu(app, paused, &version, &update) {
        Ok(menu) => {
            if let Err(e) = tray.set_menu(Some(menu)) {
                eprintln!("[QuickQuick] 托盘：更新菜单文案失败: {e}");
            }
        }
        Err(e) => eprintln!("[QuickQuick] 托盘：重建菜单失败: {e}"),
    }
}

/// 把一次状态机事件落到共享状态上。
///
/// 返回 `false` 表示状态未注册（调用方不必再刷新托盘）。纯函数 [`next_status`]
/// 负责判定迁移，这里只做读写接线。
fn apply_update_event(app: &tauri::AppHandle, event: UpdateEvent) -> bool {
    let Some(state) = app.try_state::<TrayState>() else {
        eprintln!("[QuickQuick] 托盘：更新状态未注册，检查更新被忽略");
        return false;
    };
    let next = next_status(&state.update_status(), event);
    state.set_update_status(next);
    true
}

/// 触发一次「检查更新」。
///
/// 检查进行中直接忽略（菜单项同时是 disabled，两条路径共同防止重复请求）。
/// 网络请求跑在 Tauri 异步运行时上：托盘菜单回调不能阻塞等待。
fn start_update_check(app: &tauri::AppHandle) {
    if !can_start_update_check(&update_status_of(app)) {
        // 上一轮还在检查中：菜单项已 disabled，这里兜住键盘/重复派发
        return;
    }
    apply_update_event(app, UpdateEvent::Started);
    // 立即把「正在检查更新…」摆出来：请求可能要几秒，先给点击一个回执
    refresh_tray_from_state(app);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let outcome = check_update_outcome(&handle).await;
        apply_update_event(&handle, UpdateEvent::Finished(outcome));
        refresh_tray_from_state(&handle);
    });
}

/// 调用既有更新命令，并把结果归一成状态机事件。
///
/// 复用 [`crate::ipc::update::check_for_updates`]（同一 endpoint、同一套中文错误
/// 文案）：托盘与设置页看到的结论必须一致，不能各写一套更新链路。
async fn check_update_outcome(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    match crate::ipc::update::check_for_updates(app.clone()).await {
        Ok(result) if result.available => Ok(Some(result.version)),
        Ok(_) => Ok(None),
        Err(message) => Err(message),
    }
}

/// 打开设置：前置主窗口并把路由切到设置页。
///
/// 本应用只有一个窗口（`main`），「设置」是其中的一个页面而不是独立窗口，
/// 故这里不新建窗口：窗口部分复用 [`show_and_focus_window`]（与「显示 QuickQuick」
/// 完全同一条路径），页面路由由前端监听 [`ROUTE_EVENT`] 完成。
/// 先广播再显示：窗口还在隐藏状态时就已完成路由，用户看到的第一帧就是设置页，
/// 不会先闪一下剪贴板页。
fn open_settings(app: &tauri::AppHandle) {
    if let Err(e) = app.emit(ROUTE_EVENT, ROUTE_SETTINGS) {
        // 事件发不出去时窗口仍会显示（下方），用户至少不会面对「点了没反应」；
        // 但页面会停在默认页，故必须留痕以便定位。
        eprintln!("[QuickQuick] 托盘：广播打开设置路由失败: {e}");
    }
    show_and_focus_window(app);
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

    use super::{
        can_start_update_check, menu_entries, next_status, pause_item_text, tooltip_text,
        update_item_enabled, update_item_label, version_label, MenuEntry, UpdateEvent,
        UpdateStatus, MENU_ID_CHECK_UPDATE, MENU_ID_OPEN_SETTINGS, MENU_ID_PAUSE, MENU_ID_QUIT,
        MENU_ID_SHOW, MENU_ID_VERSION, ROUTE_EVENT, ROUTE_SETTINGS, TOOLTIP_IDLE,
    };

    /// 把菜单条目压成 `(id, 文案, 启用位)` 便于断言顺序与文案；分隔线 id 为 `None`。
    fn flatten(entries: &[MenuEntry]) -> Vec<(Option<&'static str>, String, bool)> {
        entries
            .iter()
            .map(|entry| match entry {
                MenuEntry::Separator => (None, String::new(), true),
                MenuEntry::Item { id, label, enabled } => (Some(*id), label.clone(), *enabled),
            })
            .collect()
    }

    /// 只取非分隔线条目的 `(id, 文案, 启用位)`。
    fn items(entries: &[MenuEntry]) -> Vec<(&'static str, String, bool)> {
        flatten(entries)
            .into_iter()
            .filter_map(|(id, label, enabled)| id.map(|id| (id, label, enabled)))
            .collect()
    }

    // ===== 菜单结构（顺序 / 文案 / 启用位）=====

    #[test]
    fn menu_entries_keep_fixed_order() {
        // Arrange & Act
        let entries = menu_entries(false, "0.3.8", &UpdateStatus::Idle);
        let slots: Vec<Option<&'static str>> =
            flatten(&entries).into_iter().map(|(id, _, _)| id).collect();

        // Assert：顺序即契约，状态相关项固定分组，重建菜单时不靠插入位置猜
        assert_eq!(
            slots,
            vec![
                Some(MENU_ID_SHOW),
                Some(MENU_ID_PAUSE),
                None,
                Some(MENU_ID_OPEN_SETTINGS),
                Some(MENU_ID_CHECK_UPDATE),
                None,
                Some(MENU_ID_VERSION),
                None,
                Some(MENU_ID_QUIT),
            ]
        );
    }

    #[test]
    fn menu_entries_carry_expected_labels() {
        // Arrange & Act
        let labels: Vec<String> = items(&menu_entries(false, "0.3.8", &UpdateStatus::Idle))
            .into_iter()
            .map(|(_, label, _)| label)
            .collect();

        // Assert
        assert_eq!(
            labels,
            vec![
                "显示 QuickQuick",
                "暂停捕获",
                "打开设置",
                "检查更新",
                "QuickQuick v0.3.8",
                "退出",
            ]
        );
    }

    #[test]
    fn menu_entries_only_version_entry_is_read_only() {
        // Arrange & Act
        let read_only: Vec<(&str, String)> =
            items(&menu_entries(true, "0.3.8", &UpdateStatus::Idle))
                .into_iter()
                .filter(|(_, _, enabled)| !enabled)
                .map(|(id, label, _)| (id, label))
                .collect();

        // Assert：版本号只读展示，其余项都必须可点击，否则又会「点了没反应」
        assert_eq!(
            read_only,
            vec![(MENU_ID_VERSION, "QuickQuick v0.3.8".to_string())]
        );
    }

    #[test]
    fn menu_entries_reflect_pause_state_in_pause_item() {
        // Arrange & Act
        let paused = items(&menu_entries(true, "0.3.8", &UpdateStatus::Idle));
        let resumed = items(&menu_entries(false, "0.3.8", &UpdateStatus::Idle));
        let pause_label = |rows: &[(&'static str, String, bool)]| -> String {
            rows.iter()
                .find(|(id, _, _)| *id == MENU_ID_PAUSE)
                .map(|(_, label, _)| label.clone())
                .expect("菜单必须含暂停项")
        };

        // Assert
        assert_eq!(pause_label(&paused), "恢复捕获");
        assert_eq!(pause_label(&resumed), "暂停捕获");
    }

    #[test]
    fn check_update_item_is_disabled_while_checking() {
        // Arrange & Act
        let rows = items(&menu_entries(false, "0.3.8", &UpdateStatus::Checking));
        let check_row = rows
            .iter()
            .find(|(id, _, _)| *id == MENU_ID_CHECK_UPDATE)
            .expect("菜单必须含检查更新项");

        // Assert
        assert_eq!(check_row.1, "正在检查更新…");
        assert!(!check_row.2, "检查进行中必须禁用，防止重复请求");
    }

    #[test]
    fn check_update_item_keeps_result_visible_after_finish() {
        // Arrange & Act
        let rows = items(&menu_entries(
            false,
            "0.3.8",
            &UpdateStatus::Failed("检查更新失败：网络错误".to_string()),
        ));
        let check_row = rows
            .iter()
            .find(|(id, _, _)| *id == MENU_ID_CHECK_UPDATE)
            .expect("菜单必须含检查更新项");

        // Assert：结果留在菜单上（不能静默），且恢复可点击以支持重试
        assert_eq!(check_row.1, "检查更新失败，点击重试");
        assert!(check_row.2);
    }

    // ===== 版本号 =====

    #[test]
    fn version_label_formats_installed_version() {
        // Arrange & Act & Assert
        assert_eq!(version_label("0.3.8"), "QuickQuick v0.3.8");
    }

    #[test]
    fn version_label_tolerates_prefixed_version() {
        // Arrange & Act & Assert：package_info 给裸版本号，上游带上 v 也不能变 vv0.3.8
        assert_eq!(version_label("v0.3.8"), "QuickQuick v0.3.8");
    }

    // ===== 更新检查状态机 =====

    #[test]
    fn update_status_starts_from_idle_into_checking() {
        // Arrange & Act & Assert
        assert_eq!(
            next_status(&UpdateStatus::Idle, UpdateEvent::Started),
            UpdateStatus::Checking
        );
    }

    #[test]
    fn update_status_reports_up_to_date_without_new_version() {
        // Arrange & Act & Assert
        assert_eq!(
            next_status(&UpdateStatus::Checking, UpdateEvent::Finished(Ok(None))),
            UpdateStatus::UpToDate
        );
    }

    #[test]
    fn update_status_carries_available_version() {
        // Arrange & Act & Assert
        assert_eq!(
            next_status(
                &UpdateStatus::Checking,
                UpdateEvent::Finished(Ok(Some("1.2.3".to_string())))
            ),
            UpdateStatus::Available("1.2.3".to_string())
        );
    }

    #[test]
    fn update_status_carries_failure_reason() {
        // Arrange & Act & Assert
        assert_eq!(
            next_status(
                &UpdateStatus::Checking,
                UpdateEvent::Finished(Err("检查更新失败：网络错误".to_string()))
            ),
            UpdateStatus::Failed("检查更新失败：网络错误".to_string())
        );
    }

    #[test]
    fn update_status_ignores_duplicate_start_while_checking() {
        // Arrange & Act & Assert：重复点击既不能重置成新一轮，也不能退回 Idle
        assert_eq!(
            next_status(&UpdateStatus::Checking, UpdateEvent::Started),
            UpdateStatus::Checking
        );
    }

    #[test]
    fn update_status_ignores_stale_finish_after_result_shown() {
        // Arrange
        let shown = UpdateStatus::UpToDate;

        // Act：结果已展示后到达的陈旧回执（例如上一轮的超时）
        let next = next_status(&shown, UpdateEvent::Finished(Ok(Some("9.9.9".to_string()))));

        // Assert
        assert_eq!(next, UpdateStatus::UpToDate);
    }

    #[test]
    fn update_check_can_start_unless_already_checking() {
        // Arrange & Act & Assert
        assert!(can_start_update_check(&UpdateStatus::Idle));
        assert!(can_start_update_check(&UpdateStatus::UpToDate));
        assert!(can_start_update_check(&UpdateStatus::Available(
            "1.2.3".to_string()
        )));
        assert!(can_start_update_check(&UpdateStatus::Failed(
            "检查更新失败：网络错误".to_string()
        )));
        assert!(!can_start_update_check(&UpdateStatus::Checking));
    }

    #[test]
    fn update_item_label_reports_every_state() {
        // Arrange & Act & Assert
        assert_eq!(update_item_label(&UpdateStatus::Idle), "检查更新");
        assert_eq!(update_item_label(&UpdateStatus::Checking), "正在检查更新…");
        assert_eq!(update_item_label(&UpdateStatus::UpToDate), "已是最新版本");
        assert_eq!(
            update_item_label(&UpdateStatus::Available("1.2.3".to_string())),
            "发现新版本 v1.2.3"
        );
        assert_eq!(
            update_item_label(&UpdateStatus::Failed("检查更新失败：网络错误".to_string())),
            "检查更新失败，点击重试"
        );
    }

    #[test]
    fn update_item_is_clickable_again_after_result() {
        // Arrange & Act & Assert：「结束后恢复」——失败与已是最新都必须能再点
        assert!(update_item_enabled(&UpdateStatus::UpToDate));
        assert!(update_item_enabled(&UpdateStatus::Failed(
            "检查更新失败：网络错误".to_string()
        )));
        assert!(!update_item_enabled(&UpdateStatus::Checking));
    }

    // ===== 可见性文案 =====

    #[test]
    fn tooltip_reports_idle_state_by_default() {
        // Arrange & Act & Assert
        assert_eq!(
            tooltip_text(false, false, &UpdateStatus::Idle),
            TOOLTIP_IDLE
        );
    }

    #[test]
    fn tooltip_reflects_paused_state() {
        // Arrange & Act & Assert：暂停必须看得见，否则用户按了 ⌥ 会以为快捷键失效
        assert_eq!(
            tooltip_text(true, false, &UpdateStatus::Idle),
            "QuickQuick（已暂停捕获，⌥ 点击恢复）"
        );
    }

    #[test]
    fn tooltip_prefers_skip_once_over_paused() {
        // Arrange & Act & Assert：一次性忽略更短暂、更容易被忘掉，优先展示
        assert_eq!(
            tooltip_text(true, true, &UpdateStatus::Idle),
            "QuickQuick（仅忽略下一次变化）"
        );
        assert_eq!(
            tooltip_text(false, true, &UpdateStatus::Idle),
            "QuickQuick（仅忽略下一次变化）"
        );
    }

    #[test]
    fn tooltip_appends_update_result_to_pause_state() {
        // Arrange & Act & Assert：用户可能同时处于暂停态又点了检查更新，两段都要在
        assert_eq!(
            tooltip_text(true, false, &UpdateStatus::UpToDate),
            "QuickQuick（已暂停捕获，⌥ 点击恢复；已是最新版本）"
        );
        assert_eq!(
            tooltip_text(false, true, &UpdateStatus::Checking),
            "QuickQuick（仅忽略下一次变化；正在检查更新…）"
        );
    }

    #[test]
    fn tooltip_carries_failure_reason() {
        // Arrange & Act：失败不能只留 eprintln——托盘是打包后唯一用户可见通道
        let text = tooltip_text(
            false,
            false,
            &UpdateStatus::Failed("检查更新失败：网络错误".to_string()),
        );

        // Assert
        assert_eq!(text, "QuickQuick（检查更新失败：网络错误）");
    }

    #[test]
    fn tooltip_marks_available_version_with_download_hint() {
        // Arrange & Act & Assert
        assert_eq!(
            tooltip_text(false, false, &UpdateStatus::Available("1.2.3".to_string())),
            "QuickQuick（发现新版本 v1.2.3，可在设置页下载）"
        );
    }

    #[test]
    fn pause_menu_item_names_the_next_action() {
        // Arrange & Act & Assert
        assert_eq!(pause_item_text(false), "暂停捕获");
        assert_eq!(pause_item_text(true), "恢复捕获");
    }

    // ===== 跨语言契约 =====

    /// 路由事件名与 payload 必须与前端 `src/ipc/events.ts` 的 `ROUTE_EVENT` /
    /// `RoutePayload` 一致。Tauri 事件名跨语言无法编译期共享，故两侧各钉一条断言：
    /// 这里改成别的字面量就会红，提醒同步前端。
    #[test]
    fn route_event_matches_frontend_contract() {
        // Arrange & Act & Assert
        assert_eq!(ROUTE_EVENT, "route");
        assert_eq!(ROUTE_SETTINGS, "settings");
    }
}
