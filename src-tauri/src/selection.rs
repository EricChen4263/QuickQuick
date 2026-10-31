//! 取词通道：读取当前前台应用的选中文字。
//!
//! 为什么独立成模块：取词是「热键即译」的前置条件，且必须在唤起浮窗、激活本进程之前
//! 完成——QuickQuick 一旦成为前台应用，键盘焦点离开源应用，AX 就读不到选区了。
//! 这条通道还有一条硬约束：**绝不模拟 ⌘C、绝不改动用户剪贴板**（降级只读剪贴板），
//! 否则用户复制过的内容会被一次翻译悄悄顶掉。
//!
//! 分两层，让核心判据不依赖 GUI：
//! - [`SelectionOutcome`] + [`classify`]：纯判定，输入是「平台是否支持 / 是否已授权 /
//!   AX 读到了什么」，可在 headless 环境把每个分支都断言到；
//! - [`read_selection`]：平台入口。macOS 走 Accessibility（AXUIElement）读取焦点元素的
//!   `AXSelectedText`；其余平台恒返回 [`SelectionOutcome::Unsupported`]（本期不做 Windows 取词）。

/// 单次取词尝试的结局。
///
/// 区分四种而不是「有没有值」：降级到剪贴板是同一件事，但**为什么降级**要能如实告诉用户
/// （未授权 vs 平台不支持 vs 用户没选中），否则浮窗只能说一句含糊的「来自剪贴板」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionOutcome {
    /// 读到非空选中文字（已去掉首尾空白）。
    Text(String),
    /// 已授权但没拿到选中文字：没有焦点元素、焦点元素不暴露选区、或选区本身就是空的。
    NoSelection,
    /// 当前平台没有取词通道。
    Unsupported,
    /// 未获得辅助功能（Accessibility）授权。
    Untrusted,
}

/// 判定一次取词尝试的结局（纯函数）。
///
/// 判据顺序即优先级：
/// 1. 平台没有取词通道 → `Unsupported`（与是否授权无关，先说清能力边界）；
/// 2. 未授权 → `Untrusted`：不采信任何读取结果，未授权时拿到的值不该当真；
/// 3. 授权且有非空选中文字 → `Text`（trim 后）；
/// 4. 其余（读取失败 / 无焦点元素 / 选中为空）→ `NoSelection`。
///
/// 把「平台是否支持」也做成入参，是为了让非 macOS 分支能在 macOS 宿主上被断言到：
/// 否则那条路径只有 Linux CI 才跑到，本机改坏它也不会红。
#[must_use]
pub fn classify(is_supported: bool, is_trusted: bool, ax_read: Option<String>) -> SelectionOutcome {
    if !is_supported {
        return SelectionOutcome::Unsupported;
    }
    if !is_trusted {
        return SelectionOutcome::Untrusted;
    }
    match ax_read {
        Some(text) if !text.trim().is_empty() => SelectionOutcome::Text(text.trim().to_string()),
        _ => SelectionOutcome::NoSelection,
    }
}

/// 读取当前前台应用的选中文字。
///
/// 任何失败都收敛成 [`SelectionOutcome`] 的一种，**不 panic、不向上抛**：热键回调无论取词
/// 成败都必须继续走到剪贴板降级，取词失败不该让热键失灵。
#[must_use]
pub fn read_selection() -> SelectionOutcome {
    #[cfg(target_os = "macos")]
    {
        macos::read_selection()
    }

    #[cfg(not(target_os = "macos"))]
    {
        // 本期只有 macOS 有取词通道：Windows 走剪贴板降级并标注「暂不支持选中取词」。
        SelectionOutcome::Unsupported
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use core_foundation::base::{CFType, CFTypeID, CFTypeRef, TCFType};
    use core_foundation::string::{CFString, CFStringRef};

    use super::{classify, SelectionOutcome};

    /// `kAXFocusedApplicationAttribute` 的属性名（在 system-wide 元素上读取，得到焦点应用）。
    const FOCUSED_APPLICATION_ATTRIBUTE: &str = "AXFocusedApplication";

    /// `kAXFocusedUIElementAttribute` 的属性名（在焦点应用上读取，得到焦点元素）。
    ///
    /// 用字面量而不是链接 ApplicationServices 的 `CFStringRef` 全局量：属性名是 ABI 稳定的
    /// 字符串常量，链接外部静态变量只会多一层链接期依赖，还拿不到 const 语义。
    const FOCUSED_ELEMENT_ATTRIBUTE: &str = "AXFocusedUIElement";

    /// `kAXSelectedTextAttribute` 的属性名。
    const SELECTED_TEXT_ATTRIBUTE: &str = "AXSelectedText";

    /// 单次 AX 跨进程消息的超时（秒）。
    ///
    /// 热键回调跑在主线程上，目标应用无响应时 AX 调用会按系统默认超时阻塞数秒，
    /// 把热键和整个 UI 一起拖住。0.3s 足够一次健康的跨进程问答；超时即放弃取词、
    /// 降级读剪贴板，用户感知到的是「译了剪贴板」，而不是「按了热键没反应」。
    const AX_MESSAGING_TIMEOUT_SECS: f32 = 0.3;

    /// `kAXErrorSuccess`：AX 调用成功的返回码。
    const AX_ERROR_SUCCESS: i32 = 0;

    // 链接 ApplicationServices 框架取几组 AX 符号：
    // - AXIsProcessTrusted：授权检测（与 macos_paste.rs 各自声明同一符号，互不冲突）
    // - AXUIElementCreateSystemWide / AXUIElementGetTypeID：取系统级元素并校验类型
    // - AXUIElementSetMessagingTimeout / AXUIElementCopyAttributeValue：设超时、读属性
    // - AXUIElementGetPid：只读元素所属 pid，不向目标应用发消息（用于判断焦点是否在本进程）
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXUIElementCreateSystemWide() -> CFTypeRef;
        fn AXUIElementGetTypeID() -> CFTypeID;
        fn AXUIElementSetMessagingTimeout(element: CFTypeRef, timeout_in_seconds: f32) -> i32;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
        fn AXUIElementGetPid(element: CFTypeRef, pid: *mut i32) -> i32;
    }

    /// 读取选中文字；未授权时连一次 AX 调用都不发。
    ///
    /// 为什么未授权要提前返回：AX 调用在未授权下必然失败，还可能触发系统的授权提示，
    /// 而调用方随后就降级读剪贴板了，多这一次跨进程往返只是白等。
    pub(super) fn read_selection() -> SelectionOutcome {
        // SAFETY: AXIsProcessTrusted 是无状态、无副作用的稳定 C API（10.9+）。
        let is_trusted = unsafe { AXIsProcessTrusted() };
        if !is_trusted {
            return classify(true, false, None);
        }
        classify(true, true, read_focused_selection())
    }

    /// 取焦点元素的 `AXSelectedText`；任何一步拿不到都返回 `None`。
    ///
    /// `None` 是正常结局而非错误：大量应用（网页内容、Electron、终端）根本不暴露选区属性。
    ///
    /// 先取「焦点应用」再取它的焦点元素，而不是从 system-wide 直接取焦点元素：前者能在
    /// 调用任何目标应用之前先看清它的 pid。焦点应用若是本进程（浮窗已开、主窗口在前时再按
    /// 热键），就直接放弃——热键回调跑在主线程，对本进程发 AX 请求会等主线程自己，同步阻塞到超时。
    fn read_focused_selection() -> Option<String> {
        // SAFETY: 返回 +1 引用计数的系统级元素；下面立刻交给 create rule 包装，
        // 由类型自身的 Drop 释放，不手工 CFRelease。
        let raw = unsafe { AXUIElementCreateSystemWide() };
        if raw.is_null() {
            eprintln!("[QuickQuick] 取词: AXUIElementCreateSystemWide 返回空，跳过取词");
            return None;
        }
        // SAFETY: raw 是上面刚拿到的 +1 引用，且已排除空指针；create rule 接管这份所有权。
        let system_wide = unsafe { CFType::wrap_under_create_rule(raw) };

        let focused_app =
            copy_attribute(system_wide.as_CFTypeRef(), FOCUSED_APPLICATION_ATTRIBUTE)?;
        if application_pid(focused_app.as_CFTypeRef()) == Some(std::process::id() as i32) {
            return None;
        }

        let focused = copy_attribute(focused_app.as_CFTypeRef(), FOCUSED_ELEMENT_ATTRIBUTE)?;
        // 类型校验：属性值不保证是 AXUIElement（有的应用返回别的类型），
        // 当元素用会拿到 kAXErrorIllegalArgument。
        // SAFETY: AXUIElementGetTypeID 是无副作用的类型查询。
        if focused.type_of() != unsafe { AXUIElementGetTypeID() } {
            return None;
        }

        let text = copy_attribute(focused.as_CFTypeRef(), SELECTED_TEXT_ATTRIBUTE)?;
        // 同理：选区值不保证是字符串（也可能是数组或多值属性），先校验再转换。
        if !text.instance_of::<CFString>() {
            return None;
        }
        // SAFETY: 上面已校验 text 的动态类型是 CFString，转换指针合法；get rule 使 value 自增一次引用，
        // 随后 text 自身 Drop 归还它持有的那份，净额正确。
        let value: CFString =
            unsafe { CFString::wrap_under_get_rule(text.as_concrete_TypeRef() as CFStringRef) };
        Some(value.to_string())
    }

    /// 读元素所属进程的 pid；拿不到返回 `None`。
    ///
    /// `AXUIElementGetPid` 只读元素自带的 pid，不向元素所在应用发消息，因此即使目标是本进程
    /// 也不会造成自阻塞。
    fn application_pid(element: CFTypeRef) -> Option<i32> {
        let mut pid: i32 = -1;
        // SAFETY: element 是有效的 AX 元素（调用方保证）；pid 是本地可写的 i32，地址在调用期间有效。
        let error = unsafe { AXUIElementGetPid(element, &mut pid) };
        (error == AX_ERROR_SUCCESS && pid >= 0).then_some(pid)
    }

    /// 读一个 AX 属性，返回按 create rule 持有所有权的 [`CFType`]。
    ///
    /// 超时注意：`AXUIElementSetMessagingTimeout` 对 system-wide 元素调用时，
    /// 按 Apple 文档是**进程全局**的默认值，会影响本进程之后所有 AX 调用。
    /// 当前仓库没有其他 AX 读取用法，所以不构成问题；日后若新增 AX 调用，需先考虑这一点。
    /// 这里对每次读取都设一次，是为了让目标元素自身的超时也确定生效，代价只是一次冗余调用。
    fn copy_attribute(element: CFTypeRef, attribute: &str) -> Option<CFType> {
        // SAFETY: element 由调用方保证是有效的 AX 元素；超时设置只影响后续消息的等待时长。
        let timeout_error =
            unsafe { AXUIElementSetMessagingTimeout(element, AX_MESSAGING_TIMEOUT_SECS) };
        if timeout_error != AX_ERROR_SUCCESS {
            // 设不上超时不是致命错误（继续读，最坏退到系统默认超时），
            // 但它是「热键可能卡住」的唯一征兆，必须留在日志里。
            eprintln!(
                "[QuickQuick] 取词: AXUIElementSetMessagingTimeout({attribute}) 失败，返回码 {timeout_error}"
            );
        }

        let name = CFString::new(attribute);
        let mut value: CFTypeRef = std::ptr::null();
        // SAFETY: 属性名是有效 CFString；成功时 value 为 +1 引用，下面交给 create rule 包装。
        let copied = unsafe {
            AXUIElementCopyAttributeValue(element, name.as_concrete_TypeRef(), &mut value)
        };
        if copied != AX_ERROR_SUCCESS {
            // 错误码必须留日志：kAXErrorAPIDisabled（授权失效）与 kAXErrorCannotComplete（超时）
            // 对用户表现都是「没有选中」，只有日志能区分两者。日志只含错误码与属性名，不含文本。
            eprintln!("[QuickQuick] 取词: 读取 {attribute} 失败，返回码 {copied}");
            return None;
        }
        if value.is_null() {
            return None;
        }
        // SAFETY: copied 为成功且 value 非空，按 AX 约定它是 +1 引用，create rule 接管所有权。
        Some(unsafe { CFType::wrap_under_create_rule(value) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 未授权：无论 AX 读到什么（理论上读不到）都不能采信。
    #[test]
    fn classify_reports_untrusted_when_not_authorized() {
        assert_eq!(classify(true, false, None), SelectionOutcome::Untrusted);
    }

    /// 未授权时必须优先判成 Untrusted，而不是把读到的值当选中文字。
    #[test]
    fn classify_ignores_read_result_when_not_authorized() {
        assert_eq!(
            classify(true, false, Some("SENTINEL_TEXT".to_string())),
            SelectionOutcome::Untrusted,
            "未授权时不得采信任何读取结果"
        );
    }

    /// 已授权但没有焦点元素（AX 返回空）：不是错误，是没有可译选区。
    #[test]
    fn classify_reports_no_selection_when_ax_read_returns_nothing() {
        assert_eq!(classify(true, true, None), SelectionOutcome::NoSelection);
    }

    /// 已授权、焦点元素有选区但内容纯空白：等价于没有选中文字，不能拿去翻译。
    #[test]
    fn classify_reports_no_selection_for_blank_text() {
        assert_eq!(
            classify(true, true, Some("  \n\t ".to_string())),
            SelectionOutcome::NoSelection
        );
    }

    /// 非空选中文字：去掉首尾空白后作为待译文本。
    #[test]
    fn classify_returns_trimmed_selection_text() {
        assert_eq!(
            classify(true, true, Some("  hello world\n".to_string())),
            SelectionOutcome::Text("hello world".to_string())
        );
    }

    /// 平台不支持取词（本期仅 macOS 实现）：优先于授权判定，直接标 Unsupported。
    #[test]
    fn classify_reports_unsupported_when_platform_has_no_channel() {
        assert_eq!(
            classify(false, true, Some("hello".to_string())),
            SelectionOutcome::Unsupported
        );
        assert_eq!(classify(false, false, None), SelectionOutcome::Unsupported);
    }

    /// 非 macOS 构建的入口恒返回 Unsupported（macOS 宿主上不编译，由 Linux CI 覆盖）。
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn read_selection_reports_unsupported_on_non_macos() {
        assert_eq!(read_selection(), SelectionOutcome::Unsupported);
    }
}
