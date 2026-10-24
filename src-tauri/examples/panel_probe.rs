//! 面板焦点语义探针：机械验证 ADR §6.2 的 #1「面板弹出不夺焦」与 #6「面板失焦自动隐藏」。
//!
//! 探针自建 NSPanel 与 AppKit 事件循环，**不依赖 Tauri 启动流程**（门禁文档要求），
//! 因此能在 QuickQuick app 未运行的情况下单独复跑。生产实现（`src-tauri/src/popover.rs`）
//! 只作为对照阅读：探针不调用它，也不依赖 `tauri-nspanel`。
//!
//! 手动触发（examples 不进默认 `cargo test`）：
//!
//! ```bash
//! cd src-tauri
//! cargo run --example panel_probe                                # 正常路径：#1 + #6
//! cargo run --example panel_probe -- control-no-nonactivating     # 对照组 A1：只去掉样式位
//! cargo run --example panel_probe -- control-legacy-activate      # 对照组 A2：面板化前的旧路径
//! cargo run --example panel_probe -- control-no-resign-hide       # 对照组 B：不挂失焦隐藏
//! ```
//!
//! 退出码：`0` 全部判据通过；`1` 存在 FAIL；`2` 参数非法；`3` 前置条件（GUI 会话）不可用。
//! 对照组按设计就是「判据失败」，所以它们的退出码是 `1` —— 非 0 即证明红色可达。
//!
//! ## 探针复刻的生产语义（缺一不可）
//!
//! 1. `NSPanel` 子类把 `canBecomeKeyWindow` 覆盖为 true。**这是必须的一步**：无标题栏窗口的
//!    `canBecomeKeyWindow` 默认返回 false，`makeKeyWindow` 会静默失败——生产在
//!    `tauri_panel! { panel!(ClipPopoverPanel { config: { can_become_key_window: true } }) }`
//!    里给的是同一个开关。
//! 2. 样式位追加 `NSWindowStyleMaskNonactivatingPanel`：让面板**不激活本 app 也能成为
//!    key window**，这就是「不夺焦」的机制来源。
//! 3. show 顺序与生产 `show_and_make_key()` 一致：`makeFirstResponder(contentView)` →
//!    `orderFrontRegardless` → `makeKeyWindow`（先 order front 再 make key）。
//! 4. 失焦隐藏走 `NSWindowDidResignKeyNotification` 观察者 + `orderOut`，不占用 delegate。
//!    生产同样用通知（`observe_resign_key_hide`）而非 `tauri-nspanel` 的
//!    `set_event_handler`——后者会 `setDelegate:` 替换 tao 的窗口代理，掐断前端 focus 事件。
//!
//! ## 判据口径
//!
//! - `#1`：show 面板前后各取一次 `NSWorkspace.frontmostApplication` 的
//!   bundleIdentifier + processIdentifier，断言两者都不变。
//! - `#1-key`：同一轮 show 之后面板确实持有 key（`isKeyWindow`）。没有这条，#1 可能是
//!   空判据——面板若根本没拿到 key，「不夺焦」自然成立但与「面板能收键盘」无关。
//! - `#6`：面板处于 key 后触发一次 key 转移，断言面板 `isVisible` 变为 false。驱动器见
//!   `evaluate_resign_hide` 的注释：先试「激活另一个 app」（最贴近真实场景），该驱动在本机
//!   执行环境下不产生 resign-key 时退到「显式 resignKeyWindow」，并在证据里标明实际用了哪个。
//!
//! ## 探针会动前台焦点
//!
//! `#6` 的驱动器 A 会尝试激活另一个 app（因此探针跑完会把原先的前台 app 激活回来），
//! 对照组 A2 会让探针进程成为前台——两者都会在屏幕上产生可见的焦点切换。
//! 探针不修改任何文件、不写库，只在内存里造窗口。

fn main() {
    #[cfg(target_os = "macos")]
    {
        std::process::exit(macos::run());
    }

    #[cfg(not(target_os = "macos"))]
    {
        println!("PROBE_ENV gui_session=unavailable reason=unsupported_platform");
        println!("PANEL_PROBE_SUMMARY variant=none gates=0 pass=0 fail=0 exit=3");
        std::process::exit(3);
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use block2::RcBlock;
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::{AnyObject, NSObjectProtocol};
    use objc2::{define_class, extern_methods, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicy,
        NSBackingStoreType, NSPanel, NSRunningApplication, NSWindowDidResignKeyNotification,
        NSWindowStyleMask, NSWorkspace,
    };
    use objc2_foundation::{
        NSDate, NSNotification, NSNotificationCenter, NSPoint, NSRect, NSRunLoop, NSSize, NSTimer,
    };

    /// 正常路径下全部判据通过时的退出码。
    const EXIT_PASS: i32 = 0;
    /// 存在 FAIL 判据时的退出码（对照组按设计落在这里）。
    const EXIT_FAIL: i32 = 1;
    /// 参数非法时的退出码。
    const EXIT_USAGE: i32 = 2;
    /// GUI 会话不可用时的退出码。
    const EXIT_UNAVAILABLE: i32 = 3;

    /// 探针面板的初始 frame（位置与尺寸都不参与判据，只要求落在屏幕内）。
    const PANEL_ORIGIN: (f64, f64) = (240.0, 240.0);
    const PANEL_SIZE: (f64, f64) = (420.0, 260.0);
    /// 进入 NSApplication.run() 后第一拍执行判据的延迟。
    const FIRST_TICK_SECONDS: f64 = 0.3;
    /// 单次 runloop 泵的切片长度，用来让 AppKit 处理窗口服务器事件与通知。
    const RUNLOOP_SLICE_SECONDS: f64 = 0.02;
    /// show 之后等待「窗口上屏 + 可能的前台切换」落定。
    const SHOW_SETTLE_SECONDS: f64 = 0.4;
    /// 单个失焦驱动器的等待上限。
    const RESIGN_HIDE_TIMEOUT_MS: u64 = 1500;
    /// 优先用作「另一个 app」的失焦源。
    const PREFERRED_TARGET_BUNDLE: &str = "com.apple.finder";

    const CHECK_KEEPS_FRONT: &str = "keeps_frontmost";
    const CHECK_HOLDS_KEY: &str = "panel_holds_key";
    const CHECK_RESIGN_HIDE: &str = "hides_on_resign_key";

    /// 退出码出口：判据在 NSApplication.run() 的定时器回调里跑，结果经此回传。
    static EXIT_CODE: AtomicI32 = AtomicI32::new(EXIT_UNAVAILABLE);
    /// 实际收到的 NSWindowDidResignKeyNotification 条数，作为 #6 的证据。
    static RESIGN_NOTIFICATIONS: AtomicUsize = AtomicUsize::new(0);

    define_class!(
        // 与生产 ClipPopoverPanel 对齐的最小差异：无标题栏的 NSPanel 必须显式声明
        // 「可成为 key window」，否则 makeKeyWindow 静默失败（生产在 tauri_panel! 里
        // 配了 can_become_key_window: true）。
        #[unsafe(super(NSPanel))]
        #[name = "QQPanelProbePanel"]
        #[ivars = ()]
        struct ProbePanel;

        unsafe impl NSObjectProtocol for ProbePanel {}

        impl ProbePanel {
            #[unsafe(method(canBecomeKeyWindow))]
            fn can_become_key_window(&self) -> bool {
                true
            }

            #[unsafe(method(canBecomeMainWindow))]
            fn can_become_main_window(&self) -> bool {
                false
            }
        }
    );

    impl ProbePanel {
        extern_methods!(
            #[unsafe(method(initWithContentRect:styleMask:backing:defer:))]
            #[unsafe(method_family = init)]
            unsafe fn init_with_content_rect(
                this: Allocated<Self>,
                content_rect: NSRect,
                style: NSWindowStyleMask,
                backing_store_type: NSBackingStoreType,
                flag: bool,
            ) -> Retained<Self>;
        );
    }

    /// 探针变体：正常路径 + 两组对照组。
    #[derive(Clone, Copy)]
    enum Variant {
        /// 面板 + nonactivatingPanel，不激活本 app。
        Baseline,
        /// 对照组 A1：只去掉 nonactivatingPanel 样式位，显示方式不变。
        ControlNoNonactivating,
        /// 对照组 A2：面板化之前的普通窗口路径（必须先激活 app 才能拿到 key）。
        ControlLegacyActivate,
        /// 对照组 B：不挂 NSWindowDidResignKeyNotification → orderOut。
        ControlNoResignHide,
    }

    impl Variant {
        fn from_args() -> Result<Self, String> {
            match std::env::args().nth(1).as_deref() {
                None | Some("baseline") => Ok(Self::Baseline),
                Some("control-no-nonactivating") => Ok(Self::ControlNoNonactivating),
                Some("control-legacy-activate") => Ok(Self::ControlLegacyActivate),
                Some("control-no-resign-hide") => Ok(Self::ControlNoResignHide),
                Some(other) => Err(other.to_string()),
            }
        }

        const fn as_str(self) -> &'static str {
            match self {
                Self::Baseline => "baseline",
                Self::ControlNoNonactivating => "control-no-nonactivating",
                Self::ControlLegacyActivate => "control-legacy-activate",
                Self::ControlNoResignHide => "control-no-resign-hide",
            }
        }

        /// 是否给窗口加上 nonactivatingPanel 样式位。
        const fn uses_nonactivating_panel(self) -> bool {
            matches!(self, Self::Baseline | Self::ControlNoResignHide)
        }

        /// 是否走「普通窗口旧路径」：显式激活 app 后再 makeKeyAndOrderFront。
        const fn activates_app_before_show(self) -> bool {
            matches!(self, Self::ControlLegacyActivate)
        }

        /// 是否跑 show 类判据（#1 / #1-key）。
        const fn runs_show_checks(self) -> bool {
            !matches!(self, Self::ControlNoResignHide)
        }

        /// 是否跑失焦隐藏判据（#6）。
        const fn runs_resign_hide(self) -> bool {
            matches!(self, Self::Baseline | Self::ControlNoResignHide)
        }

        /// 是否挂接失焦隐藏；对照组 B 故意不挂。
        const fn installs_resign_hide(self) -> bool {
            matches!(self, Self::Baseline)
        }
    }

    /// 前台应用身份：bundleIdentifier 可能缺失（无 Info.plist 的进程），故用 Option。
    struct Frontmost {
        bundle: Option<String>,
        pid: i32,
    }

    impl Frontmost {
        fn capture() -> Option<Self> {
            let workspace = NSWorkspace::sharedWorkspace();
            let app = workspace.frontmostApplication()?;
            Some(Self {
                bundle: app.bundleIdentifier().map(|b| b.to_string()),
                pid: app.processIdentifier(),
            })
        }

        fn describe(&self) -> String {
            describe_identity(self.bundle.as_deref(), self.pid)
        }

        fn same_as(&self, other: &Self) -> bool {
            self.pid == other.pid && self.bundle == other.bundle
        }
    }

    fn describe_identity(bundle: Option<&str>, pid: i32) -> String {
        format!("{}#{}", bundle.unwrap_or("<no-bundle>"), pid)
    }

    fn describe_running_app(app: &NSRunningApplication) -> String {
        describe_identity(
            app.bundleIdentifier().map(|b| b.to_string()).as_deref(),
            app.processIdentifier(),
        )
    }

    /// 探针进程自己是否被系统算作「活跃应用」。这是外部视角，
    /// 与 `NSApplication.isActive`（进程内状态）可能不一致，故单独取证。
    fn own_is_active() -> String {
        let own_pid = std::process::id() as i32;
        let workspace = NSWorkspace::sharedWorkspace();
        for app in workspace.runningApplications().iter() {
            if app.processIdentifier() == own_pid {
                return app.isActive().to_string();
            }
        }
        "not_listed".to_string()
    }

    /// 单条判据的结果与证据。
    struct CheckResult {
        check: &'static str,
        gate: &'static str,
        passed: bool,
        evidence: String,
    }

    impl CheckResult {
        fn new(check: &'static str, gate: &'static str, passed: bool, evidence: String) -> Self {
            Self {
                check,
                gate,
                passed,
                evidence,
            }
        }

        fn report(&self) {
            let verdict = if self.passed { "PASS" } else { "FAIL" };
            println!(
                "CHECK check={} gate={} verdict={}",
                self.check, self.gate, verdict
            );
            println!("EVIDENCE check={} {}", self.check, self.evidence);
        }
    }

    /// 泵 runloop 指定时长，让 AppKit 处理窗口与通知事件。
    fn pump_runloop(seconds: f64) {
        let run_loop = NSRunLoop::currentRunLoop();
        let deadline = Instant::now() + Duration::from_secs_f64(seconds);
        while Instant::now() < deadline {
            let slice = NSDate::dateWithTimeIntervalSinceNow(RUNLOOP_SLICE_SECONDS);
            run_loop.runUntilDate(&slice);
        }
    }

    /// 泵 runloop 直到条件成立或超时；返回条件是否成立。
    fn pump_until(mut predicate: impl FnMut() -> bool, timeout: Duration) -> bool {
        let run_loop = NSRunLoop::currentRunLoop();
        let deadline = Instant::now() + timeout;
        loop {
            if predicate() {
                return true;
            }
            if Instant::now() >= deadline {
                return predicate();
            }
            let slice = NSDate::dateWithTimeIntervalSinceNow(RUNLOOP_SLICE_SECONDS);
            run_loop.runUntilDate(&slice);
        }
    }

    /// 创建探针面板。样式位与生产的 tao 建窗结果对齐：无标题栏（Borderless），
    /// 可最小化位不作为判据输入；`nonactivating` 为 false 时去掉「不激活 app」的样式位。
    fn create_panel(mtm: MainThreadMarker, nonactivating: bool) -> Retained<ProbePanel> {
        let mut mask = NSWindowStyleMask::Borderless | NSWindowStyleMask::Miniaturizable;
        if nonactivating {
            mask |= NSWindowStyleMask::NonactivatingPanel;
        }
        let frame = NSRect::new(
            NSPoint::new(PANEL_ORIGIN.0, PANEL_ORIGIN.1),
            NSSize::new(PANEL_SIZE.0, PANEL_SIZE.1),
        );
        let panel = unsafe {
            ProbePanel::init_with_content_rect(
                ProbePanel::alloc(mtm),
                frame,
                mask,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // 探针自己持有面板（Retained 在作用域内），不让 close 语义参与释放，
        // 否则 orderOut 之后窗口可能被回收，观察 isVisible 就成了未定义行为。
        unsafe { panel.setReleasedWhenClosed(false) };
        panel
    }

    /// 与生产 `show_and_make_key()` 同序：makeFirstResponder → orderFrontRegardless → makeKeyWindow。
    fn show_nonactivating(panel: &ProbePanel) {
        if let Some(view) = panel.contentView() {
            panel.makeFirstResponder(Some(&view));
        }
        panel.orderFrontRegardless();
        panel.makeKeyWindow();
    }

    /// 面板化之前的普通窗口旧路径：显式激活本 app 后再 makeKeyAndOrderFront。
    ///
    /// 这里用 `activateIgnoringOtherApps(true)`（已废弃但本机仍生效），因为面板化之前
    /// 生产代码 `activate_app_macos()` 追求的就是「把 app 拉到前台」这个效果；
    /// 现代 `activate()` 在非前台进程里对本机探针不生效（实测 frontmost 不变）。
    fn show_legacy(app: &NSApplication, panel: &ProbePanel) {
        if let Some(view) = panel.contentView() {
            panel.makeFirstResponder(Some(&view));
        }
        panel.orderFrontRegardless();
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        panel.makeKeyAndOrderFront(None);
    }

    /// 挂接失焦隐藏：观察本面板的 NSWindowDidResignKeyNotification 后 orderOut。
    ///
    /// 不占用 delegate，因此与 tao / 其他观察者可以共存——这也是生产实现的选型理由。
    /// 观察者与面板都要活到进程结束，故双方都不反注册/不释放（探针是一次性进程）。
    fn install_resign_key_hide(panel: &ProbePanel) {
        let panel_ptr = panel as *const ProbePanel as usize;
        let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            // 通知在主线程投递，探针就在这里直接 orderOut，不做跨线程派发。
            RESIGN_NOTIFICATIONS.fetch_add(1, Ordering::SeqCst);
            let panel = unsafe { &*(panel_ptr as *const ProbePanel) };
            panel.orderOut(None);
        });

        let panel_object: &AnyObject = unsafe { &*(panel_ptr as *const AnyObject) };
        let center = NSNotificationCenter::defaultCenter();
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidResignKeyNotification),
                Some(panel_object),
                None,
                &block,
            )
        };
        std::mem::forget(observer);
    }

    /// 找一个「另一个 app」来做失焦源：优先 Finder，退而取任一普通激活策略的外部 app。
    fn find_activation_target() -> Option<Retained<NSRunningApplication>> {
        let own_pid = std::process::id() as i32;
        let workspace = NSWorkspace::sharedWorkspace();
        let apps = workspace.runningApplications();
        let mut fallback: Option<Retained<NSRunningApplication>> = None;
        for app in apps.iter() {
            if app.processIdentifier() == own_pid {
                continue;
            }
            let bundle = app.bundleIdentifier().map(|b| b.to_string());
            if bundle.as_deref() == Some(PREFERRED_TARGET_BUNDLE) {
                return Some(app);
            }
            if fallback.is_none()
                && bundle.is_some()
                && app.activationPolicy() == NSApplicationActivationPolicy::Regular
            {
                fallback = Some(app);
            }
        }
        fallback
    }

    /// 把前台应用还给探针启动前的那个 app，避免把用户的焦点留在 Finder。
    fn restore_frontmost(original: &Frontmost) {
        let workspace = NSWorkspace::sharedWorkspace();
        let apps = workspace.runningApplications();
        for app in apps.iter() {
            if app.processIdentifier() == original.pid {
                let _ = app.activateWithOptions(NSApplicationActivationOptions::empty());
                pump_runloop(SHOW_SETTLE_SECONDS);
                return;
            }
        }
    }

    /// show 类判据：一次 show 同时产出 #1（前台不变）与 #1-key（面板确实拿到 key）。
    fn evaluate_show(
        mtm: MainThreadMarker,
        app: &NSApplication,
        variant: Variant,
    ) -> Vec<CheckResult> {
        let before = Frontmost::capture();

        let panel = create_panel(mtm, variant.uses_nonactivating_panel());
        if variant.activates_app_before_show() {
            show_legacy(app, &panel);
        } else {
            show_nonactivating(&panel);
        }
        pump_runloop(SHOW_SETTLE_SECONDS);

        let after = Frontmost::capture();
        let panel_key = panel.isKeyWindow();
        let panel_visible = panel.isVisible();
        let own_active = own_is_active();
        panel.orderOut(None);
        pump_runloop(RUNLOOP_SLICE_SECONDS * 5.0);

        let before_desc = before
            .as_ref()
            .map(|f| f.describe())
            .unwrap_or_else(|| "unreadable".to_string());
        let after_desc = after
            .as_ref()
            .map(|f| f.describe())
            .unwrap_or_else(|| "unreadable".to_string());
        let unchanged = before
            .as_ref()
            .zip(after.as_ref())
            .map(|(b, a)| a.same_as(b))
            .unwrap_or(false);

        let keeps_front = CheckResult::new(
            CHECK_KEEPS_FRONT,
            "#1",
            unchanged,
            format!(
                "before={} after={} panel_key={} panel_visible={} probe_app_is_active={}",
                before_desc, after_desc, panel_key, panel_visible, own_active
            ),
        );
        let holds_key = CheckResult::new(
            CHECK_HOLDS_KEY,
            "#1-key",
            panel_key,
            format!(
                "panel_key={} show_mode={} nonactivating_style={}",
                panel_key,
                if variant.activates_app_before_show() {
                    "activate_then_make_key_and_order_front"
                } else {
                    "order_front_then_make_key"
                },
                variant.uses_nonactivating_panel()
            ),
        );
        vec![keeps_front, holds_key]
    }

    /// 判据 #6：面板成为 key 后，一次 key 转移应使面板不可见。
    ///
    /// 驱动器按「贴近真实场景 → 确定性」排序：
    ///
    /// - A：让另一个 app 激活（`activateWithOptions:`，优先 Finder）。这是门禁描述的原始场景。
    /// - B：显式 `resignKeyWindow`。当前执行环境下驱动器 A 让前台 app 换了，但非激活面板
    ///   不会因此 resign key（app 从未活跃过，没有「失活」这一跳），因此需要 B 才能把
    ///   「resign key → orderOut」这条接线打成可复跑证据。
    ///
    /// 证据里写明实际生效的是哪个驱动器；对照组 B 两个驱动器都不会让它隐藏。
    fn evaluate_resign_hide(
        mtm: MainThreadMarker,
        app: &NSApplication,
        variant: Variant,
    ) -> CheckResult {
        let panel = create_panel(mtm, variant.uses_nonactivating_panel());
        show_nonactivating(&panel);
        pump_runloop(SHOW_SETTLE_SECONDS);

        let key_before = panel.isKeyWindow();
        let visible_before = panel.isVisible();
        if variant.installs_resign_hide() {
            install_resign_key_hide(&panel);
        }

        // 驱动器 A：激活另一个 app。
        let target = find_activation_target();
        let target_desc = target
            .as_ref()
            .map(|t| describe_running_app(t))
            .unwrap_or_else(|| "<no-target>".to_string());
        let activation_returned = target
            .as_ref()
            .map(|t| t.activateWithOptions(NSApplicationActivationOptions::empty()))
            .unwrap_or(false);
        let frontmost_after_activation = Frontmost::capture()
            .map(|f| f.describe())
            .unwrap_or_else(|| "unreadable".to_string());
        let hidden_by_activation = pump_until(
            || !panel.isVisible(),
            Duration::from_millis(RESIGN_HIDE_TIMEOUT_MS),
        );
        let notifications_after_activation = RESIGN_NOTIFICATIONS.load(Ordering::SeqCst);

        // 驱动器 B：显式让出 key（只在 A 没把面板隐藏时才需要）。
        let mut drive = "foreign_app_activation";
        let mut hidden = hidden_by_activation;
        if !hidden {
            drive = "explicit_resign_key";
            panel.resignKeyWindow();
            hidden = pump_until(
                || !panel.isVisible(),
                Duration::from_millis(RESIGN_HIDE_TIMEOUT_MS),
            );
        }

        let visible_after = panel.isVisible();
        let key_after = panel.isKeyWindow();
        let app_active_after = app.isActive();
        let notifications = RESIGN_NOTIFICATIONS.load(Ordering::SeqCst);
        if visible_after {
            panel.orderOut(None);
        }
        // 面板指针被通知块捕获，这里故意不释放：一次性进程里泄露一个窗口比悬垂指针安全。
        std::mem::forget(panel);

        let passed = key_before && hidden;
        CheckResult::new(
            CHECK_RESIGN_HIDE,
            "#6",
            passed,
            format!(
                "panel_key_before={} panel_visible_before={} target={} activate_returned={} \
                 frontmost_after_activation={} hidden_by_activation={} \
                 resign_notifications_after_activation={} drive={} panel_visible_after={} \
                 panel_key_after={} nsapp_is_active_after={} resign_notifications={} hook={} \
                 timeout_ms={}",
                key_before,
                visible_before,
                target_desc,
                activation_returned,
                frontmost_after_activation,
                hidden_by_activation,
                notifications_after_activation,
                drive,
                visible_after,
                key_after,
                app_active_after,
                notifications,
                variant.installs_resign_hide(),
                RESIGN_HIDE_TIMEOUT_MS
            ),
        )
    }

    /// GUI 会话前置探测：能建 NSApplication / 能建并上屏窗口 / 能读前台应用。
    /// 失败时返回 Err(原因)，由调用方以 EXIT_UNAVAILABLE 退出，不伪造判据结果。
    fn preflight(mtm: MainThreadMarker, app: &NSApplication) -> Result<(), &'static str> {
        if Frontmost::capture().is_none() {
            return Err("frontmost_unreadable");
        }
        let panel = create_panel(mtm, true);
        show_nonactivating(&panel);
        pump_runloop(SHOW_SETTLE_SECONDS);
        let panel_visible = panel.isVisible();
        let panel_key = panel.isKeyWindow();
        let can_become_key = panel.canBecomeKeyWindow();
        panel.orderOut(None);
        pump_runloop(RUNLOOP_SLICE_SECONDS * 5.0);
        if !panel_visible {
            return Err("panel_not_visible");
        }
        println!(
            "PROBE_ENV window_created=true panel_visible_after_show={} panel_key_after_show={} \
             can_become_key_window={} activation_policy={}",
            panel_visible,
            panel_key,
            can_become_key,
            app.activationPolicy().0
        );
        Ok(())
    }

    /// 在事件循环内跑全部判据并打印结果，返回退出码。
    fn execute_probe(
        mtm: MainThreadMarker,
        app: &NSApplication,
        variant: Variant,
        original_frontmost: &Frontmost,
    ) -> i32 {
        println!(
            "PROBE_ENV gui_session=probing variant={} nsapp_ready=true",
            variant.as_str()
        );
        match preflight(mtm, app) {
            Ok(()) => {}
            Err(reason) => {
                println!("PROBE_ENV gui_session=unavailable reason={}", reason);
                println!(
                    "PANEL_PROBE_SUMMARY variant={} gates=0 pass=0 fail=0 exit=3",
                    variant.as_str()
                );
                return EXIT_UNAVAILABLE;
            }
        }
        // 判据要求「面板弹出前后前台不变」，前提是探针进程此刻不在前台：
        // 若 AppKit 已经把探针拉起为活跃应用，先把前台还回去，再开始量。
        if own_is_active() == "true" {
            restore_frontmost(original_frontmost);
        }
        println!(
            "PROBE_ENV gui_session=ok frontmost_before={} probe_app_is_active={}",
            Frontmost::capture()
                .map(|f| f.describe())
                .unwrap_or_else(|| "unreadable".to_string()),
            own_is_active()
        );

        let mut results = Vec::new();
        if variant.runs_show_checks() {
            results.extend(evaluate_show(mtm, app, variant));
            // 两条判据之间的间隔：让上一条的 orderOut 落定，避免污染 #6 的 key 前置。
            pump_runloop(SHOW_SETTLE_SECONDS);
        }
        if variant.runs_resign_hide() {
            results.push(evaluate_resign_hide(mtm, app, variant));
        }

        restore_frontmost(original_frontmost);

        let pass = results.iter().filter(|r| r.passed).count();
        let fail = results.len() - pass;
        for result in &results {
            result.report();
        }
        let exit = if fail == 0 { EXIT_PASS } else { EXIT_FAIL };
        println!(
            "PANEL_PROBE_SUMMARY variant={} gates={} pass={} fail={} exit={}",
            variant.as_str(),
            results.len(),
            pass,
            fail,
            exit
        );
        exit
    }

    pub fn run() -> i32 {
        // AppKit 全部操作都在主线程：run 由 main 调用，判据在 NSApplication.run()
        // 的定时器回调里执行——激活（activate）必须发生在事件循环内才生效。
        let Some(mtm) = MainThreadMarker::new() else {
            println!("PROBE_ENV gui_session=unavailable reason=not_main_thread");
            println!("PANEL_PROBE_SUMMARY variant=none gates=0 pass=0 fail=0 exit=3");
            return EXIT_UNAVAILABLE;
        };

        let variant = match Variant::from_args() {
            Ok(variant) => variant,
            Err(raw) => {
                println!(
                    "PROBE_ENV gui_session=unknown reason=bad_variant value={}",
                    raw
                );
                println!(
                    "PANEL_PROBE_SUMMARY variant={} gates=0 pass=0 fail=0 exit=2",
                    raw
                );
                return EXIT_USAGE;
            }
        };

        // 原始前台必须在探针改动任何 AppKit 状态之前取，用于「把焦点还回去」。
        let original_frontmost = Frontmost::capture();
        let app = NSApplication::sharedApplication(mtm);
        // Regular：与生产 app 一致（有 Dock 图标的普通应用），也是对照组 A2
        // 「普通窗口路径会夺焦」能成立的前提。
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.finishLaunching();

        let mut original_for_block = original_frontmost;
        if original_for_block.is_none() {
            original_for_block = Frontmost::capture();
        }
        let app_for_block = app.clone();
        let block = RcBlock::new(move |_timer: NonNull<NSTimer>| {
            let code = match original_for_block.as_ref() {
                Some(original) => execute_probe(mtm, &app_for_block, variant, original),
                None => {
                    println!("PROBE_ENV gui_session=unavailable reason=frontmost_unreadable");
                    println!(
                        "PANEL_PROBE_SUMMARY variant={} gates=0 pass=0 fail=0 exit=3",
                        variant.as_str()
                    );
                    EXIT_UNAVAILABLE
                }
            };
            EXIT_CODE.store(code, Ordering::SeqCst);
            app_for_block.stop(None);
        });
        let _timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_repeats_block(FIRST_TICK_SECONDS, false, &block)
        };
        app.run();
        EXIT_CODE.load(Ordering::SeqCst)
    }
}
