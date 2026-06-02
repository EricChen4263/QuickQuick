//! macOS 生产粘贴后端与 Accessibility 探针（V5-F4-S04-9a）
//!
//! 设计对齐：设计文档§八#2 + onboarding.rs AccessibilityProbe + paste.rs PasteBackend
//!
//! 本模块提供两个生产实现：
//! - `MacOsAccessibilityProbe` — 调用 AXIsProcessTrusted() FFI
//! - `MacOsPasteBackend`       — NSPasteboard.changeCount + arboard 写入 + CGEvent Cmd+V
//!
//! 非 macOS 平台提供降级实现：
//! - `FallbackAccessibilityProbe` — is_trusted() 永远返回 false
//! - `FallbackPasteBackend`       — no-op send_paste，arboard 写入（9b 接入处由 probe=false 拦截）
//!
//! # unsafe 说明
//! - AXIsProcessTrusted：C FFI，安全包装后暴露安全签名
//! - NSPasteboard.changeCount：objc2 已标注 unsafe extern，包一层安全函数
//! - CGEvent：core-graphics crate 的 new_keyboard_event/post 内部已处理 unsafe

use crate::clipboard::CapturedItem;
use crate::onboarding::AccessibilityProbe;
use crate::paste::PasteBackend;

// macOS 平台实现
#[cfg(target_os = "macos")]
mod macos_impl {
    use super::{AccessibilityProbe, CapturedItem, PasteBackend};

    // AXIsProcessTrusted 声明：链接 ApplicationServices 框架，最小化依赖
    // 依据：https://developer.apple.com/documentation/applicationservices/axisprocesstrusted
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }

    /// 调用 AXIsProcessTrusted()，返回当前进程是否已获辅助功能授权。
    ///
    /// # Safety
    /// AXIsProcessTrusted 是 macOS 稳定 C API（10.9+），无状态、无副作用，
    /// 调用约定为 C；包成安全函数后调用方不需要 unsafe 块。
    fn ax_is_process_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    /// macOS 生产 Accessibility 探针：调用真实 AXIsProcessTrusted()。
    pub struct MacOsAccessibilityProbe;

    impl AccessibilityProbe for MacOsAccessibilityProbe {
        fn is_trusted(&self) -> bool {
            ax_is_process_trusted()
        }
    }

    /// 读取 NSPasteboard.generalPasteboard().changeCount()，转 u64。
    ///
    /// NSInteger 在 macOS 64-bit 为 i64；changeCount 单调递增、始终非负，
    /// 用 max(0, v) 后转 u64 保证类型安全（负值在实践中不出现，防御性处理）。
    fn read_nspasteboard_change_count() -> u64 {
        use objc2_app_kit::NSPasteboard;
        let pb = NSPasteboard::generalPasteboard();
        let n = pb.changeCount();
        n.max(0) as u64
    }

    /// macOS 生产粘贴后端。
    ///
    /// - `change_count`：读 NSPasteboard.changeCount()
    /// - `write_with_marker`：arboard 写文本（与 pipeline ArboardBackend 一致，bump changeCount）
    /// - `current_text`：arboard 读文本
    /// - `send_paste`：CGEvent 合成 Cmd+V 并 post 到 HID 事件流
    pub struct MacOsPasteBackend {
        clipboard: arboard::Clipboard,
    }

    impl MacOsPasteBackend {
        /// 创建实例；arboard 初始化失败时返回 `Err(String)`。
        ///
        /// # Errors
        /// arboard::Clipboard::new() 失败时返回包含原因的错误字符串。
        pub fn new() -> Result<Self, String> {
            let clipboard = arboard::Clipboard::new()
                .map_err(|e| format!("MacOsPasteBackend: 剪贴板初始化失败: {e}"))?;
            Ok(Self { clipboard })
        }
    }

    impl PasteBackend for MacOsPasteBackend {
        fn change_count(&self) -> u64 {
            read_nspasteboard_change_count()
        }

        /// 将条目文本写入剪贴板（arboard），写入会使 NSPasteboard.changeCount 递增。
        fn write_with_marker(&mut self, item: &CapturedItem) {
            if let Err(e) = self.clipboard.set_text(item.text.clone()) {
                eprintln!("[MacOsPasteBackend] write_with_marker 失败: {e}");
            }
        }

        fn current_text(&self) -> Option<String> {
            // arboard Clipboard::get_text 需要 &mut self，此处通过新实例读取以满足 &self 签名。
            // 创建临时剪贴板实例读取当前内容，代价为单次系统调用，可接受。
            arboard::Clipboard::new()
                .ok()
                .and_then(|mut cb| cb.get_text().ok())
        }

        /// 发送 Cmd+V 键盘事件到 HID 系统事件流，模拟粘贴。
        ///
        /// 实现步骤：
        /// 1. 创建 CGEventSource（HIDSystemState）
        /// 2. 合成 keyDown(keycode=0x09, flags=CGEventFlagCommand)
        /// 3. 合成 keyUp(keycode=0x09, flags=CGEventFlagCommand)
        /// 4. 两个事件均 post 到 CGEventTapLocation::HID
        ///
        /// # keycode 依据
        /// ANSI_V = 0x09（十进制 9），来源：
        /// Carbon Events.h kVK_ANSI_V = 0x09，与 core-graphics KeyCode::ANSI_V 一致。
        ///
        /// # flags 依据
        /// CGEventFlagCommand = 0x00100000，对应 macOS NXCommandMask，
        /// 等价于历史文档的 kCGEventFlagMaskCommand。
        fn send_paste(&mut self) {
            use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode};
            use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

            let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
                Ok(s) => s,
                Err(()) => {
                    eprintln!("[MacOsPasteBackend] send_paste: CGEventSource 创建失败");
                    return;
                }
            };

            let post_key = |keydown: bool| -> bool {
                let ev =
                    match CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_V, keydown) {
                        Ok(e) => e,
                        Err(()) => {
                            eprintln!(
                                "[MacOsPasteBackend] send_paste: CGEvent 创建失败 keydown={keydown}"
                            );
                            return false;
                        }
                    };
                ev.set_flags(CGEventFlags::CGEventFlagCommand);
                ev.post(CGEventTapLocation::HID);
                true
            };

            if post_key(true) {
                post_key(false);
            }
        }
    }
}

// 非 macOS 降级实现
#[cfg(not(target_os = "macos"))]
mod fallback_impl {
    use super::{AccessibilityProbe, CapturedItem, PasteBackend};

    /// 非 macOS 平台降级探针：is_trusted() 永远返回 false。
    ///
    /// 使非 mac 构建始终走 write_back 降级路径（9b 接入处由 perform_paste_or_degrade 处理）。
    pub struct FallbackAccessibilityProbe;

    impl AccessibilityProbe for FallbackAccessibilityProbe {
        fn is_trusted(&self) -> bool {
            false
        }
    }

    /// 非 macOS 平台降级粘贴后端。
    ///
    /// send_paste 为 no-op；write_with_marker/current_text 通过 arboard 实现。
    /// 运行时 probe.is_trusted()==false 会让 perform_paste_or_degrade 跳过 send_paste，
    /// 所以此处的 no-op 属防御性兜底。
    pub struct FallbackPasteBackend {
        clipboard: arboard::Clipboard,
        count: u64,
        text: Option<String>,
    }

    impl FallbackPasteBackend {
        /// 创建实例；arboard 初始化失败时返回 `Err(String)`。
        ///
        /// # Errors
        /// arboard::Clipboard::new() 失败时返回包含原因的错误字符串。
        pub fn new() -> Result<Self, String> {
            let clipboard = arboard::Clipboard::new()
                .map_err(|e| format!("FallbackPasteBackend: 剪贴板初始化失败: {e}"))?;
            Ok(Self {
                clipboard,
                count: 0,
                text: None,
            })
        }
    }

    impl PasteBackend for FallbackPasteBackend {
        fn change_count(&self) -> u64 {
            self.count
        }

        fn write_with_marker(&mut self, item: &CapturedItem) {
            if let Err(e) = self.clipboard.set_text(item.text.clone()) {
                eprintln!("[FallbackPasteBackend] write_with_marker 失败: {e}");
            }
            self.text = Some(item.text.clone());
            self.count += 1;
        }

        fn current_text(&self) -> Option<String> {
            self.text.clone()
        }

        fn send_paste(&mut self) {
            // 非 mac 降级：no-op；运行时由 probe=false 拦截，不会到达此处
        }
    }
}

// 公开导出（统一入口，9b 接入前暂 allow dead_code）
#[cfg(target_os = "macos")]
pub use macos_impl::MacOsAccessibilityProbe;
#[cfg(target_os = "macos")]
pub use macos_impl::MacOsPasteBackend;

#[cfg(not(target_os = "macos"))]
pub use fallback_impl::FallbackAccessibilityProbe;
#[cfg(not(target_os = "macos"))]
pub use fallback_impl::FallbackPasteBackend;
