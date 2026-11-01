//! 取词通道：读取当前前台应用的选中文字及其选区边界。
//!
//! 为什么独立成模块：取词是「热键即译」的前置条件，且必须在唤起浮窗、激活本进程之前
//! 完成——QuickQuick 一旦成为前台应用，键盘焦点离开源应用，AX 就读不到选区了。
//! 这条通道还有一条硬约束：**绝不模拟 ⌘C、绝不改动用户剪贴板**（降级只读剪贴板），
//! 否则用户复制过的内容会被一次翻译悄悄顶掉。
//!
//! 分两层，让核心判据不依赖 GUI：
//! - [`SelectionOutcome`] + [`classify`]：纯判定，输入是「平台是否支持 / 是否已授权 /
//!   AX 读到了什么」，可在 headless 环境把每个分支都断言到；
//! - [`read_selection_snapshot`]：平台入口。macOS 走 Accessibility（AXUIElement）读取焦点
//!   元素的 `AXSelectedText`，并在**同一次**读取里顺带取选区边界 `AXBoundsForRange`，
//!   供浮窗贴近选区定位；其余平台恒返回 [`SelectionOutcome::Unsupported`]（本期不做
//!   Windows 取词）。
//!
//! 边界为什么必须与文字同一次读：源应用在本进程 show 之后就不再是焦点应用，第二次
//! 跨进程读取只能拿到空选区；因此文字与几何是同一次 AX 会话的产物。

/// AX 返回的屏幕矩形（逻辑点，左上原点，全局显示器坐标空间）。
///
/// 为什么手写而不是复用 `core-graphics` 的 `CGRect`：那个类型只在 macOS 目标下存在，
/// 而本类型要出现在 `window_pos` 的纯函数签名与单测里，必须在所有平台可见。
/// 字段语义与 `CGRect` 一致；换算（逻辑点 → 物理像素）由定位模块负责，
/// 本模块只做「这个矩形能不能当锚点」的校验。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    /// 选区左边界（逻辑点）。
    pub x: f64,
    /// 选区上边界（逻辑点，从主显示器顶边算起）。
    pub y: f64,
    /// 选区宽度（逻辑点）。
    pub width: f64,
    /// 选区高度（逻辑点）。
    pub height: f64,
}

impl ScreenRect {
    /// 从 AX 的矩形分量构造；尺寸非正或含非有限值时返回 `None`。
    ///
    /// 零尺寸矩形代表「只有插入点、没有可见选区」：部分应用在 caret 状态下也返回
    /// range 边界。拿它当锚点会把浮窗贴到光标尖上，所以按「没有边界」处理，
    /// 由调用方回退到光标定位。
    #[must_use]
    pub fn from_ax_rect(x: f64, y: f64, width: f64, height: f64) -> Option<Self> {
        let is_usable = [x, y, width, height].iter().all(|value| value.is_finite())
            && width > 0.0
            && height > 0.0;
        is_usable.then_some(Self {
            x,
            y,
            width,
            height,
        })
    }
}

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

/// 一次取词的完整结果：判据 + 选区边界。
///
/// 把边界做成 [`SelectionOutcome`] 的并行字段，而不是塞进 `Text(String)` 变体：
/// 判据（该译什么、为什么降级）与几何（浮窗放哪儿）是两件事，前者已有单测锚定语义，
/// 后者只在 `Text` 下才有意义。分开携带后，[`classify`] 的签名与判据完全不用动。
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionSnapshot {
    /// 取词结局，判据见 [`classify`]。
    pub outcome: SelectionOutcome,
    /// 选区边界（逻辑点，左上原点）；只有读到非空选中文字时才可能为 `Some`。
    pub bounds: Option<ScreenRect>,
}

impl SelectionSnapshot {
    /// 可用于窗口定位的选区边界。
    ///
    /// 只有 `outcome` 是 [`SelectionOutcome::Text`] 时才采信 `bounds`：没读到选中文字时
    /// AX 仍可能返回上一次的、或 caret 位置的 range 边界，拿它定位会把浮窗放到与内容
    /// 无关的位置。
    #[must_use]
    pub fn usable_bounds(&self) -> Option<ScreenRect> {
        match self.outcome {
            SelectionOutcome::Text(_) => self.bounds,
            _ => None,
        }
    }
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

/// 读取当前前台应用的选中文字与选区边界。
///
/// 任何失败都收敛成 [`SelectionSnapshot`] 的一种，**不 panic、不向上抛**：热键回调无论
/// 取词成败都必须继续走到剪贴板降级，取词失败不该让热键失灵。
#[must_use]
pub fn read_selection_snapshot() -> SelectionSnapshot {
    #[cfg(target_os = "macos")]
    {
        macos::read_selection_snapshot()
    }

    #[cfg(not(target_os = "macos"))]
    {
        // 本期只有 macOS 有取词通道：Windows 走剪贴板降级并标注「暂不支持选中取词」。
        // 没有取词也就没有选区边界，浮窗定位回退到光标所在屏。
        SelectionSnapshot {
            outcome: SelectionOutcome::Unsupported,
            bounds: None,
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::ptr;

    use core_foundation::base::{CFType, CFTypeID, CFTypeRef, TCFType};
    use core_foundation::string::{CFString, CFStringRef};

    use super::{classify, ScreenRect, SelectionSnapshot};

    /// `kAXFocusedApplicationAttribute` 的属性名（在 system-wide 元素上读取，得到焦点应用）。
    const FOCUSED_APPLICATION_ATTRIBUTE: &str = "AXFocusedApplication";

    /// `kAXFocusedUIElementAttribute` 的属性名（在焦点应用上读取，得到焦点元素）。
    ///
    /// 用字面量而不是链接 ApplicationServices 的 `CFStringRef` 全局量：属性名是 ABI 稳定的
    /// 字符串常量，链接外部静态变量只会多一层链接期依赖，还拿不到 const 语义。
    const FOCUSED_ELEMENT_ATTRIBUTE: &str = "AXFocusedUIElement";

    /// `kAXSelectedTextAttribute` 的属性名。
    const SELECTED_TEXT_ATTRIBUTE: &str = "AXSelectedText";

    /// `kAXSelectedTextRangeAttribute` 的属性名：焦点元素的选区范围（CFRange）。
    const SELECTED_TEXT_RANGE_ATTRIBUTE: &str = "AXSelectedTextRange";

    /// `kAXBoundsForRangeParameterizedAttribute` 的属性名：把 CFRange 换成屏幕矩形。
    const BOUNDS_FOR_RANGE_PARAMETERIZED_ATTRIBUTE: &str = "AXBoundsForRange";

    /// `kAXValueTypeCGRect`：AXValue 承载的是 CGRect。
    const AX_VALUE_TYPE_CG_RECT: i32 = 3;

    /// `kAXValueTypeCFRange`：AXValue 承载的是 CFRange。
    const AX_VALUE_TYPE_CF_RANGE: i32 = 4;

    /// 单次 AX 跨进程消息的超时（秒）。
    ///
    /// 热键回调跑在主线程上，目标应用无响应时 AX 调用会按系统默认超时阻塞数秒，
    /// 把热键和整个 UI 一起拖住。0.3s 足够一次健康的跨进程问答；超时即放弃取词、
    /// 降级读剪贴板，用户感知到的是「译了剪贴板」，而不是「按了热键没反应」。
    const AX_MESSAGING_TIMEOUT_SECS: f32 = 0.3;

    /// `kAXErrorSuccess`：AX 调用成功的返回码。
    const AX_ERROR_SUCCESS: i32 = 0;

    /// CoreGraphics 的点。手写 `#[repr(C)]` 而不是引 `core-graphics` 的同名类型：
    /// 这里只需要一个 ABI 布局，不值得为它把该 crate 从传递依赖提升为显式依赖。
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    /// CoreGraphics 的尺寸。
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    /// CoreGraphics 的矩形（origin 为左上角，屏幕坐标向下为正）。
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    /// `CFRange`：CFIndex 在 macOS 上是 64 位有符号整数。
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct CFRange {
        location: isize,
        length: isize,
    }

    // 链接 ApplicationServices 框架取几组 AX 符号：
    // - AXIsProcessTrusted：授权检测（与 macos_paste.rs 各自声明同一符号，互不冲突）
    // - AXUIElementCreateSystemWide / AXUIElementGetTypeID：取系统级元素并校验类型
    // - AXUIElementSetMessagingTimeout / AXUIElementCopyAttributeValue：设超时、读属性
    // - AXUIElementCopyParameterizedAttributeValue：读参数化属性（选区边界要带 range 入参）
    // - AXUIElementGetPid：只读元素所属 pid，不向目标应用发消息（用于判断焦点是否在本进程）
    // - AXValueCreate / AXValueGetValue / AXValueGetTypeID：在 CFRange 与 CGRect 之间搬运
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
        fn AXUIElementCopyParameterizedAttributeValue(
            element: CFTypeRef,
            parameterized_attribute: CFStringRef,
            parameter: CFTypeRef,
            result: *mut CFTypeRef,
        ) -> i32;
        fn AXUIElementGetPid(element: CFTypeRef, pid: *mut i32) -> i32;
        fn AXValueCreate(the_type: i32, value_ptr: *const c_void) -> CFTypeRef;
        fn AXValueGetValue(value: CFTypeRef, the_type: i32, value_ptr: *mut c_void) -> u8;
        fn AXValueGetTypeID() -> CFTypeID;
    }

    /// 读取选中文字与选区边界；未授权时连一次 AX 调用都不发。
    ///
    /// 为什么未授权要提前返回：AX 调用在未授权下必然失败，还可能触发系统的授权提示，
    /// 而调用方随后就降级读剪贴板了，多这一次跨进程往返只是白等。
    pub(super) fn read_selection_snapshot() -> SelectionSnapshot {
        // SAFETY: AXIsProcessTrusted 是无状态、无副作用的稳定 C API（10.9+）。
        let is_trusted = unsafe { AXIsProcessTrusted() };
        if !is_trusted {
            return SelectionSnapshot {
                outcome: classify(true, false, None),
                bounds: None,
            };
        }
        let (text, bounds) = read_focused_selection();
        SelectionSnapshot {
            outcome: classify(true, true, text),
            bounds,
        }
    }

    /// 取焦点元素的选中文字与选区边界；任何一步拿不到都退化成「没有」。
    ///
    /// 焦点元素只取一次并在两条读取之间复用：源应用在本进程 show 之后不再是焦点应用，
    /// 重新从 system-wide 取一遍不只是浪费，还可能直接取空。
    fn read_focused_selection() -> (Option<String>, Option<ScreenRect>) {
        let Some(focused) = focused_element() else {
            return (None, None);
        };
        let Some(text) = read_selected_text(focused.as_CFTypeRef()) else {
            return (None, None);
        };
        // 空白选中没有可译内容，也不需要边界：浮窗会走剪贴板降级，落点用光标。
        if text.trim().is_empty() {
            return (Some(text), None);
        }
        let bounds = read_selection_bounds(focused.as_CFTypeRef());
        (Some(text), bounds)
    }

    /// 取当前进程之外的焦点 UI 元素。
    ///
    /// `None` 是正常结局而非错误：大量应用（网页内容、Electron、终端）根本不暴露选区属性。
    ///
    /// 先取「焦点应用」再取它的焦点元素，而不是从 system-wide 直接取焦点元素：前者能在
    /// 调用任何目标应用之前先看清它的 pid。焦点应用若是本进程（浮窗已开、主窗口在前时再按
    /// 热键），就直接放弃——热键回调跑在主线程，对本进程发 AX 请求会等主线程自己，同步阻塞到超时。
    fn focused_element() -> Option<CFType> {
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
        Some(focused)
    }

    /// 读焦点元素的 `AXSelectedText`；类型不符或读取失败都返回 `None`。
    fn read_selected_text(element: CFTypeRef) -> Option<String> {
        let text = copy_attribute(element, SELECTED_TEXT_ATTRIBUTE)?;
        // 选区值不保证是字符串（也可能是数组或多值属性），先校验再转换。
        if !text.instance_of::<CFString>() {
            return None;
        }
        // SAFETY: 上面已校验 text 的动态类型是 CFString，转换指针合法；get rule 使 value 自增一次引用，
        // 随后 text 自身 Drop 归还它持有的那份，净额正确。
        let value: CFString =
            unsafe { CFString::wrap_under_get_rule(text.as_concrete_TypeRef() as CFStringRef) };
        Some(value.to_string())
    }

    /// 读选区边界：先取选区范围（CFRange），再用它换屏幕矩形。
    ///
    /// 代价是选中文字时多两次跨进程属性读取，都在 0.3s 超时内；任一步失败就返回 `None`，
    /// 由调用方回退光标定位——浮窗贴不上选区只是观感退化，不该影响翻译本身。
    fn read_selection_bounds(element: CFTypeRef) -> Option<ScreenRect> {
        let range_value = copy_attribute(element, SELECTED_TEXT_RANGE_ATTRIBUTE)?;
        let range = cf_range_of(&range_value)?;
        bounds_for_range(element, range)
    }

    /// 用参数化属性把 CFRange 换成屏幕坐标矩形。
    ///
    /// AX 返回的是**逻辑点**（左上原点）；换算成窗口与显示器使用的物理像素由 `window_pos`
    /// 负责：本模块只把 AX 的原始几何如实交出去，不在这里掺入任何缩放假设。
    fn bounds_for_range(element: CFTypeRef, range: CFRange) -> Option<ScreenRect> {
        let mut range_for_parameter = range;
        // SAFETY: AXValueCreate 按 create rule 返回 +1 引用；入参指向调用期间有效的 CFRange，
        // 类型常量与所指类型的布局一致。
        let raw_parameter = unsafe {
            AXValueCreate(
                AX_VALUE_TYPE_CF_RANGE,
                (&mut range_for_parameter as *mut CFRange).cast(),
            )
        };
        if raw_parameter.is_null() {
            return None;
        }
        // SAFETY: raw_parameter 非空且是刚创建的 +1 引用，create rule 接管所有权、由 Drop 释放。
        let parameter = unsafe { CFType::wrap_under_create_rule(raw_parameter) };

        ensure_messaging_timeout(element, BOUNDS_FOR_RANGE_PARAMETERIZED_ATTRIBUTE);
        let name = CFString::new(BOUNDS_FOR_RANGE_PARAMETERIZED_ATTRIBUTE);
        let mut value: CFTypeRef = ptr::null();
        // SAFETY: element 是有效 AX 元素（调用方保证）；name / parameter 是有效 CF 对象；
        // 成功时 value 为 +1 引用。
        let copied = unsafe {
            AXUIElementCopyParameterizedAttributeValue(
                element,
                name.as_concrete_TypeRef(),
                parameter.as_CFTypeRef(),
                &mut value,
            )
        };
        if copied != AX_ERROR_SUCCESS {
            eprintln!(
                "[QuickQuick] 取词: 读取 {BOUNDS_FOR_RANGE_PARAMETERIZED_ATTRIBUTE} 失败，返回码 {copied}"
            );
            return None;
        }
        if value.is_null() {
            return None;
        }
        // SAFETY: copied 为成功且 value 非空，按 AX 约定它是 +1 引用，create rule 接管所有权。
        let bounds_value = unsafe { CFType::wrap_under_create_rule(value) };
        let rect = cg_rect_of(&bounds_value)?;
        ScreenRect::from_ax_rect(
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        )
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

    /// 从 AXValue 中取出 CFRange；动态类型不是 AXValue 或类型不匹配时返回 `None`。
    fn cf_range_of(value: &CFType) -> Option<CFRange> {
        let mut range = CFRange {
            location: 0,
            length: 0,
        };
        ax_value_get(value, AX_VALUE_TYPE_CF_RANGE, &mut range).then_some(range)
    }

    /// 从 AXValue 中取出 CGRect；动态类型不是 AXValue 或类型不匹配时返回 `None`。
    fn cg_rect_of(value: &CFType) -> Option<CGRect> {
        let mut rect = CGRect::default();
        ax_value_get(value, AX_VALUE_TYPE_CG_RECT, &mut rect).then_some(rect)
    }

    /// 按指定 AXValue 类型把值读进 `out`；类型不符返回 `false`。
    ///
    /// 先比 `AXValueGetTypeID`：`AXValueGetValue` 假定入参就是 AXValue，对别的 CF 类型调用
    /// 属于未定义行为，所以类型不合就直接放弃，不赌类型。
    fn ax_value_get<T>(value: &CFType, the_type: i32, out: &mut T) -> bool {
        // SAFETY: AXValueGetTypeID 是无副作用的类型查询。
        if value.type_of() != unsafe { AXValueGetTypeID() } {
            return false;
        }
        // SAFETY: 已校验动态类型是 AXValue；out 指向与 the_type 对应的 #[repr(C)] 结构体，
        // 布局一致；AXValueGetValue 在类型匹配时写入、返回 false 时不读 out。
        let read_ok = unsafe {
            AXValueGetValue(
                value.as_CFTypeRef(),
                the_type,
                (out as *mut T).cast::<c_void>(),
            )
        };
        read_ok != 0
    }

    /// 读一个 AX 属性，返回按 create rule 持有所有权的 [`CFType`]。
    fn copy_attribute(element: CFTypeRef, attribute: &str) -> Option<CFType> {
        ensure_messaging_timeout(element, attribute);

        let name = CFString::new(attribute);
        let mut value: CFTypeRef = ptr::null();
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

    /// 给元素设置跨进程消息超时。
    ///
    /// 注意：对 system-wide 元素调用时，按 Apple 文档这是**进程全局**的默认值，会影响本进程
    /// 之后所有 AX 调用。当前仓库没有其他 AX 读取用法，所以不构成问题；日后若新增 AX 调用，
    /// 需先考虑这一点。这里对每次读取都设一次，是为了让目标元素自身的超时也确定生效，
    /// 代价只是一次冗余调用。设不上超时不是致命错误（继续读，最坏退到系统默认超时），
    /// 但它是「热键可能卡住」的唯一征兆，必须留在日志里。
    fn ensure_messaging_timeout(element: CFTypeRef, context: &str) {
        // SAFETY: element 由调用方保证是有效的 AX 元素；超时设置只影响后续消息的等待时长。
        let error = unsafe { AXUIElementSetMessagingTimeout(element, AX_MESSAGING_TIMEOUT_SECS) };
        if error != AX_ERROR_SUCCESS {
            eprintln!(
                "[QuickQuick] 取词: AXUIElementSetMessagingTimeout({context}) 失败，返回码 {error}"
            );
        }
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

    /// 零尺寸与非法数值的矩形不能当锚点：AX 在只有 caret 时也返回 range 边界。
    #[test]
    fn screen_rect_rejects_empty_and_non_finite_rects() {
        assert_eq!(
            ScreenRect::from_ax_rect(10.0, 20.0, 0.0, 18.0),
            None,
            "零宽矩形不可用"
        );
        assert_eq!(
            ScreenRect::from_ax_rect(10.0, 20.0, 100.0, 0.0),
            None,
            "零高矩形不可用"
        );
        assert_eq!(
            ScreenRect::from_ax_rect(f64::NAN, 20.0, 100.0, 18.0),
            None,
            "NaN 坐标不可用"
        );
        assert_eq!(
            ScreenRect::from_ax_rect(10.0, 20.0, 100.0, f64::INFINITY),
            None,
            "无穷大高度不可用"
        );
    }

    /// 合法矩形原样保留四个分量：小数不取整，换算留给定位模块。
    #[test]
    fn screen_rect_keeps_valid_rect_verbatim() {
        let rect = ScreenRect::from_ax_rect(10.5, 20.25, 100.0, 18.0).expect("合法矩形必须保留");

        assert_eq!(
            rect,
            ScreenRect {
                x: 10.5,
                y: 20.25,
                width: 100.0,
                height: 18.0,
            }
        );
    }

    /// 没有读到选中文字时不采信 bounds：AX 可能返回上一次或 caret 位置的边界。
    #[test]
    fn usable_bounds_ignores_bounds_without_selection_text() {
        let bounds = ScreenRect::from_ax_rect(0.0, 0.0, 100.0, 18.0);

        let no_selection = SelectionSnapshot {
            outcome: SelectionOutcome::NoSelection,
            bounds,
        };
        let untrusted = SelectionSnapshot {
            outcome: SelectionOutcome::Untrusted,
            bounds,
        };

        assert_eq!(
            no_selection.usable_bounds(),
            None,
            "无选中时 bounds 不可用于定位"
        );
        assert_eq!(
            untrusted.usable_bounds(),
            None,
            "未授权时 bounds 不可用于定位"
        );
    }

    /// 读到选中文字时 bounds 可用于定位。
    #[test]
    fn usable_bounds_exposes_bounds_with_selection_text() {
        let bounds = ScreenRect::from_ax_rect(4.0, 8.0, 120.0, 18.0);

        let snapshot = SelectionSnapshot {
            outcome: SelectionOutcome::Text("hi".to_string()),
            bounds,
        };

        assert_eq!(snapshot.usable_bounds(), bounds);
    }

    /// 非 macOS 构建的入口恒返回 Unsupported，且没有选区边界
    /// （macOS 宿主上不编译，由 Linux CI 覆盖）。
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn read_selection_reports_unsupported_on_non_macos() {
        let snapshot = read_selection_snapshot();

        assert_eq!(snapshot.outcome, SelectionOutcome::Unsupported);
        assert_eq!(snapshot.bounds, None, "没有取词通道就没有选区边界");
    }
}
