//! 集成测试：macOS 生产后端 FFI（9a）
//!
//! 验收项：
//! - 9a-macos-A1：MacOsPasteBackend 实现 PasteBackend trait（可实例化、方法签名正确）
//! - 9a-macos-A2：MacOsAccessibilityProbe 实现 AccessibilityProbe trait（可实例化）
//! - 9a-macos-A3：非 mac 降级实现存在（FallbackAccessibilityProbe.is_trusted() == false）
//! - 9a-macos-A4：非 mac 降级 PasteBackend 实现存在（可实例化）

use quickquick_lib::onboarding::AccessibilityProbe;
use quickquick_lib::paste::PasteBackend;

/// A3：非 mac 降级探针（或 mac 探针在 mac 上）实现 AccessibilityProbe trait。
///
/// 在 macOS 上验证 MacOsAccessibilityProbe 可构造；
/// 在非 macOS 上验证 FallbackAccessibilityProbe 可构造且 is_trusted() == false。
#[test]
fn accessibility_probe_impl_exists_and_constructable() {
    #[cfg(target_os = "macos")]
    {
        use quickquick_lib::macos_paste::MacOsAccessibilityProbe;
        let probe = MacOsAccessibilityProbe;
        // is_trusted 调用真实 AXIsProcessTrusted()，CI 返回 false，本地有权限时返回 true。
        // 这里只验证调用不 panic、返回 bool。
        let _result: bool = probe.is_trusted();
    }

    #[cfg(not(target_os = "macos"))]
    {
        use quickquick_lib::macos_paste::FallbackAccessibilityProbe;
        let probe = FallbackAccessibilityProbe;
        assert!(
            !probe.is_trusted(),
            "非 mac 降级探针 is_trusted() 应永远返回 false"
        );
    }
}

/// A1：MacOsPasteBackend（macOS）或降级 PasteBackend（非 mac）可实例化。
///
/// 仅验证 trait 方法签名正确、可调用、不 panic；
/// 不验证运行时 CGEvent 实际注入效果（需授权 + GUI，单测无法验证）。
#[test]
fn paste_backend_impl_exists_and_constructable() {
    #[cfg(target_os = "macos")]
    {
        use quickquick_lib::clipboard::CapturedItem;
        use quickquick_lib::macos_paste::MacOsPasteBackend;
        assert!(
            MacOsPasteBackend::new().is_ok(),
            "MacOsPasteBackend::new() 应成功"
        );
        let mut backend = MacOsPasteBackend::new().expect("MacOsPasteBackend 初始化失败");

        // change_count 调用 NSPasteboard.changeCount()，应返回非负整数
        let count = backend.change_count();
        let _ = count;

        // current_text 读取剪贴板，返回 Option<String>，不 panic
        let _text = backend.current_text();

        // write_with_marker 写文本到剪贴板，应 bump change_count
        let item = CapturedItem {
            text: "9a-test-marker".to_owned(),
            html: None,
        };
        let count_before = backend.change_count();
        backend.write_with_marker(&item);
        let count_after = backend.change_count();
        assert!(
            count_after > count_before,
            "write_with_marker 应使 change_count 递增（写前={count_before}, 写后={count_after}）"
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        use quickquick_lib::clipboard::CapturedItem;
        use quickquick_lib::macos_paste::FallbackPasteBackend;
        assert!(
            FallbackPasteBackend::new().is_ok(),
            "FallbackPasteBackend::new() 应成功"
        );
        let mut backend = FallbackPasteBackend::new().expect("FallbackPasteBackend 初始化失败");

        let _count = backend.change_count();
        let _text = backend.current_text();

        let item = CapturedItem {
            text: "fallback-test".to_owned(),
            html: None,
        };
        backend.write_with_marker(&item);
        backend.send_paste();
    }
}
