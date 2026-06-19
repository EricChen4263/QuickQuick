//! Windows 生产粘贴后端与 Accessibility 探针（Windows 粘贴落空修复）。
//!
//! 对齐 macos_paste.rs：提供实现 `AccessibilityProbe` / `PasteBackend` 的生产后端，
//! 复用 paste.rs 的 write_and_confirm 确认时序，让 Windows 走与 macOS 对称的粘贴流程。
//!
//! 与 macOS 的差异：
//! - Windows 无"辅助功能授权"概念，`WindowsAccessibilityProbe::is_trusted()` 恒 true。
//! - changeCount 用 `GetClipboardSequenceNumber()`（系统级剪贴板序号，写入后递增）。
//! - send_paste 用 `SendInput` 合成 Ctrl+V（四个键盘事件：Ctrl↓ V↓ V↑ Ctrl↑）。
//! - 写剪贴板复用 macos_paste 里跨平台的 arboard 函数（write_item_to_clipboard 等）。
//!
//! # 不可本机测说明
//! 本模块整体 `#[cfg(target_os = "windows")]`，FFI（SendInput / GetClipboardSequenceNumber）
//! 只能在 Windows 真机验证，无法在 macOS 开发机 cargo test 红绿；纯决策逻辑
//! （should_record_hwnd 等）已在 frontmost 模块单测覆盖。行为由用户拉 CI 包手测。

use crate::clipboard::CapturedItem;
use crate::macos_paste::{write_image_to_clipboard, write_item_to_clipboard};
use crate::onboarding::AccessibilityProbe;
use crate::paste::PasteBackend;

use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VK_CONTROL,
};

/// V 键的虚拟键码。windows-sys 未提供 `VK_V` 具名常量，按 Win32 文档取 0x56。
/// 依据：Microsoft Virtual-Key Codes，V = 0x56。
const VK_V: u16 = 0x56;

/// Windows 生产 Accessibility 探针：恒已授权。
///
/// Windows 注入键盘事件（SendInput）不需要 macOS 那样的辅助功能授权，
/// 故 `is_trusted()` 恒返回 true，让粘贴流程始终走完整路径（write_and_confirm + send_paste）。
pub struct WindowsAccessibilityProbe;

impl AccessibilityProbe for WindowsAccessibilityProbe {
    fn is_trusted(&self) -> bool {
        true
    }
}

/// Windows 生产粘贴后端。
///
/// - `change_count`：`GetClipboardSequenceNumber()`（写入后系统递增，对齐 NSPasteboard.changeCount）
/// - `write_with_marker` / `write_image`：复用 arboard 跨平台写入
/// - `current_text`：arboard 读文本
/// - `send_paste`：`SendInput` 合成 Ctrl+V
pub struct WindowsPasteBackend {
    clipboard: arboard::Clipboard,
}

impl WindowsPasteBackend {
    /// 创建实例；arboard 初始化失败时返回 `Err(String)`。
    ///
    /// # Errors
    /// arboard::Clipboard::new() 失败时返回包含原因的错误字符串。
    pub fn new() -> Result<Self, String> {
        let clipboard = arboard::Clipboard::new()
            .map_err(|e| format!("WindowsPasteBackend: 剪贴板初始化失败: {e}"))?;
        Ok(Self { clipboard })
    }
}

impl PasteBackend for WindowsPasteBackend {
    /// 读取系统剪贴板序号（GetClipboardSequenceNumber），写入后递增。
    ///
    /// 该序号是进程无关的全局计数，任何应用改剪贴板都会递增；用作 write_and_confirm
    /// 轮询确认"写入已被系统接受"的依据，对齐 macOS changeCount 语义。
    fn change_count(&self) -> u64 {
        // GetClipboardSequenceNumber 无参、无副作用，返回 u32 序号；转 u64 与 trait 对齐。
        u64::from(unsafe { GetClipboardSequenceNumber() })
    }

    /// 将条目写入剪贴板（arboard 跨平台实现），写入会使剪贴板序号递增。
    fn write_with_marker(&mut self, item: &CapturedItem) {
        if let Err(e) = write_item_to_clipboard(&mut self.clipboard, item) {
            eprintln!("[WindowsPasteBackend] write_with_marker 失败: {e}");
        }
    }

    fn current_text(&self) -> Option<String> {
        // 与 MacOsPasteBackend 一致：get_text 需 &mut，故用临时实例满足 &self 签名。
        arboard::Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_text().ok())
    }

    /// 用一次 SendInput 发送 Ctrl+V（四个键盘事件：Ctrl↓ V↓ V↑ Ctrl↑），模拟粘贴。
    ///
    /// 一次 SendInput 提交整组事件保证内核按序、原子注入，避免与其他输入交错。
    /// 失败（返回注入条数 != 4）仅记录日志，不 panic。
    fn send_paste(&mut self) {
        let inputs = [
            keyboard_input(VK_CONTROL, false),
            keyboard_input(VK_V, false),
            keyboard_input(VK_V, true),
            keyboard_input(VK_CONTROL, true),
        ];
        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
        if sent as usize != inputs.len() {
            eprintln!(
                "[WindowsPasteBackend] send_paste: SendInput 仅注入 {sent}/{} 个事件",
                inputs.len()
            );
        }
    }

    /// 将图片（裸 RGBA 字节）写入剪贴板（arboard set_image），返回写入结果。
    ///
    /// 失败时返回 Err（不吞错），让调用方据此跳过 send_paste；图片无 marker 回读确认。
    fn write_image(&mut self, width: usize, height: usize, rgba: &[u8]) -> Result<(), String> {
        write_image_to_clipboard(&mut self.clipboard, width, height, rgba)
    }
}

/// 构造一个键盘 `INPUT`（按下或抬起指定虚拟键）。
///
/// `key_up == true` 时设 `KEYEVENTF_KEYUP` 标志（抬起），否则为按下（标志 0）。
/// 抽出避免 send_paste 内重复构造四个 INPUT，降低嵌套。
fn keyboard_input(vk: u16, key_up: bool) -> INPUT {
    let dwflags: KEYBD_EVENT_FLAGS = if key_up { KEYEVENTF_KEYUP } else { 0 };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: dwflags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
