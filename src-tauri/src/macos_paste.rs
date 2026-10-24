//! macOS 生产粘贴后端与 Accessibility 探针（V5-F4-S04-9a）
//!
//! 设计对齐：设计文档§八#2 + onboarding.rs AccessibilityProbe + paste.rs PasteBackend
//!
//! 本模块提供两个生产实现：
//! - `MacOsAccessibilityProbe` — 调用 AXIsProcessTrusted() FFI
//! - `MacOsPasteBackend`       — NSPasteboard.changeCount + arboard 写入 + CGEvent Cmd+V
//!
//! macOS 上所有 `NSPasteboard` 访问（changeCount、正文读写、自产标记）都经
//! `pasteboard_serial` 串行通道进入主线程，避免与轮询线程的读取并发（并发会 SIGSEGV）。
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

/// 把条目写入 arboard 剪贴板：有非空 html 写富文本（HTML + 纯文本 alt 兜底），否则写纯文本。
///
/// 抽出供 macOS / fallback 两个后端复用，避免富文本写入分支重复。
/// 纯文本兜底用 `item.text`：不支持富文本的目标 app 粘贴时仍能拿到纯文本（设计§四任务4 决策2）。
///
/// 写入成功后追加"本工具自产"标记（macOS 私有 UTI / 其余平台写入门闩），
/// 让轮询引擎跳过这次自身写入，避免粘贴后历史里又多一条、顺序被打乱。
///
/// macOS 上写入与追加标记合成**一个**串行任务，且使用串行线程上现场创建的剪贴板句柄
/// （入参句柄不会被使用）：轮询的全量读也在同一线程，两者不可能交错，
/// 因而不会读到「正文已写、标记未到」的中间态而把自己的写入再记一遍。
///
/// # Errors
/// arboard 写入失败时返回错误字符串（由调用方降级记录日志，不 panic）。
pub(crate) fn write_item_to_clipboard(
    clipboard: &mut arboard::Clipboard,
    item: &CapturedItem,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = clipboard;
        write_item_serialized(item)
    }

    #[cfg(not(target_os = "macos"))]
    {
        write_item_now(clipboard, item)
    }
}

/// 在当前线程立即写入文本载荷并追加自产标记（调用方负责线程约束）。
fn write_item_now(clipboard: &mut arboard::Clipboard, item: &CapturedItem) -> Result<(), String> {
    let written = match &item.html {
        Some(html) if !html.is_empty() => clipboard
            .set()
            .html(html.clone(), Some(item.text.clone()))
            .map_err(|e| e.to_string()),
        _ => clipboard
            .set_text(item.text.clone())
            .map_err(|e| e.to_string()),
    };
    written?;
    mark_self_write_or_log();
    Ok(())
}

/// 将图片（裸 RGBA 字节）写入 arboard 剪贴板。
///
/// arboard `set_image` 接收 `ImageData{width,height,bytes}`，bytes 为 RGBA 裸字节
/// （与捕获侧 `get_image()` 读到的格式一致）；用 `Cow::Borrowed` 借用入参字节，
/// 避免对大图做无谓全量克隆（set_image 同步消费、不需要更长生命周期）。
/// 抽出供 macOS / fallback 两个后端复用，镜像 `write_item_to_clipboard` 的错误处理。
/// 同样在写入成功后追加自产标记（图片回读字节不可比，但"是不是我写的"仍可判定）。
/// macOS 上的串行化语义同 [`write_item_to_clipboard`]。
///
/// # Errors
/// arboard `set_image` 失败时返回错误字符串（由调用方按需降级，不 panic）。
pub(crate) fn write_image_to_clipboard(
    clipboard: &mut arboard::Clipboard,
    width: usize,
    height: usize,
    rgba: &[u8],
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = clipboard;
        write_image_serialized(width, height, rgba)
    }

    #[cfg(not(target_os = "macos"))]
    {
        write_image_now(clipboard, width, height, rgba)
    }
}

/// 在当前线程立即写入图片并追加自产标记（调用方负责线程约束）。
fn write_image_now(
    clipboard: &mut arboard::Clipboard,
    width: usize,
    height: usize,
    rgba: &[u8],
) -> Result<(), String> {
    use std::borrow::Cow;
    let image_data = arboard::ImageData {
        width,
        height,
        bytes: Cow::Borrowed(rgba),
    };
    clipboard.set_image(image_data).map_err(|e| e.to_string())?;
    mark_self_write_or_log();
    Ok(())
}

/// macOS：经串行通道写入文本载荷（写入 + 标记同一任务）。
#[cfg(target_os = "macos")]
pub(crate) fn write_item_serialized(item: &CapturedItem) -> Result<(), String> {
    let item = item.clone();
    crate::pasteboard_serial::run_global(crate::pasteboard_serial::WRITE_TIMEOUT, move || {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| format!("剪贴板初始化失败: {e}"))?;
        write_item_now(&mut clipboard, &item)
    })
    .map_err(|e| e.to_string())?
}

/// macOS：经串行通道写入图片（写入 + 标记同一任务）。
#[cfg(target_os = "macos")]
pub(crate) fn write_image_serialized(
    width: usize,
    height: usize,
    rgba: &[u8],
) -> Result<(), String> {
    let rgba = rgba.to_vec();
    crate::pasteboard_serial::run_global(crate::pasteboard_serial::WRITE_TIMEOUT, move || {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| format!("剪贴板初始化失败: {e}"))?;
        write_image_now(&mut clipboard, width, height, &rgba)
    })
    .map_err(|e| e.to_string())?
}

/// 追加自产标记；失败只记日志，不影响本次写入结果。
///
/// 标记失败的实际后果是"这次写入可能被自己再捕获一次"，比让粘贴整体失败轻得多，
/// 故不向上传播错误。
fn mark_self_write_or_log() {
    if !crate::pasteboard_meta::mark_self_write() {
        eprintln!("[clipboard] 自产标记写入失败：本轮写回可能被自身轮询重复捕获一次");
    }
}

// macOS 平台实现
#[cfg(target_os = "macos")]
mod macos_impl {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        write_image_serialized, write_item_serialized, AccessibilityProbe, CapturedItem,
        PasteBackend,
    };
    use crate::pasteboard_serial::{run_global, SCALAR_READ_TIMEOUT};

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

    /// macOS 生产粘贴后端。
    ///
    /// 全部 `NSPasteboard` 访问都经串行通道：
    /// - `change_count`：串行线程上读 `NSPasteboard.changeCount()`
    /// - `write_with_marker` / `write_image`：串行线程上写正文并追加自产标记
    /// - `current_text`：串行线程上读文本
    /// - `send_paste`：CGEvent 合成 Cmd+V 并 post 到 HID 事件流（不碰 pasteboard）
    pub struct MacOsPasteBackend {
        /// 最近一次成功读到的计数：串行线程忙或超时时回放，粘贴确认逻辑据此判断「未递增」
        last_count: AtomicU64,
    }

    impl MacOsPasteBackend {
        /// 创建实例；剪贴板不可用（如 launchd 守护进程下取不到 generalPasteboard）时返回 `Err`。
        ///
        /// # Errors
        /// 串行线程上 `arboard::Clipboard::new()` 失败，或串行通道不可用时返回错误字符串。
        pub fn new() -> Result<Self, String> {
            run_global(SCALAR_READ_TIMEOUT, || {
                arboard::Clipboard::new()
                    .map(|_| ())
                    .map_err(|e| format!("MacOsPasteBackend: 剪贴板初始化失败: {e}"))
            })
            .map_err(|e| format!("MacOsPasteBackend: 剪贴板初始化失败: {e}"))??;
            Ok(Self {
                last_count: AtomicU64::new(0),
            })
        }
    }

    impl PasteBackend for MacOsPasteBackend {
        fn change_count(&self) -> u64 {
            match run_global(
                SCALAR_READ_TIMEOUT,
                crate::pasteboard_serial::read_change_count,
            ) {
                Ok(count) => {
                    self.last_count.store(count, Ordering::Relaxed);
                    count
                }
                Err(e) => {
                    eprintln!("[MacOsPasteBackend] 读取 changeCount 失败，沿用上次值: {e}");
                    self.last_count.load(Ordering::Relaxed)
                }
            }
        }

        /// 将条目写入剪贴板（arboard），写入会使 NSPasteboard.changeCount 递增。
        ///
        /// 有非空 html 时写富文本（HTML + 纯文本兜底 alt），否则写纯文本，
        /// 让粘贴还原保真格式（设计§四任务4 决策2：粘贴带富文本）。
        fn write_with_marker(&mut self, item: &CapturedItem) {
            if let Err(e) = write_item_serialized(item) {
                eprintln!("[MacOsPasteBackend] write_with_marker 失败: {e}");
            }
        }

        fn current_text(&self) -> Option<String> {
            run_global(SCALAR_READ_TIMEOUT, || {
                arboard::Clipboard::new()
                    .ok()
                    .and_then(|mut cb| cb.get_text().ok())
            })
            .unwrap_or_else(|e| {
                eprintln!("[MacOsPasteBackend] 读取当前文本失败: {e}");
                None
            })
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
                let ev = match CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_V, keydown)
                {
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

        /// 将图片（裸 RGBA 字节）写入剪贴板（arboard set_image），返回写入结果。
        ///
        /// 失败时返回 Err（不吞错），让调用方据此跳过 send_paste；图片无 marker 回读确认（见 trait 文档）。
        fn write_image(&mut self, width: usize, height: usize, rgba: &[u8]) -> Result<(), String> {
            write_image_serialized(width, height, rgba)
        }
    }
}

// 非 macOS 降级实现
#[cfg(not(target_os = "macos"))]
mod fallback_impl {
    use super::{
        write_image_to_clipboard, write_item_to_clipboard, AccessibilityProbe, CapturedItem,
        PasteBackend,
    };

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
            if let Err(e) = write_item_to_clipboard(&mut self.clipboard, item) {
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

        /// 将图片（裸 RGBA 字节）写入剪贴板（arboard set_image），返回写入结果。
        ///
        /// 失败时返回 Err（不吞错），让调用方据此跳过 send_paste；图片无 marker 回读确认（见 trait 文档）。
        fn write_image(&mut self, width: usize, height: usize, rgba: &[u8]) -> Result<(), String> {
            write_image_to_clipboard(&mut self.clipboard, width, height, rgba)
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
