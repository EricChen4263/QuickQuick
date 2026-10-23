//! 剪贴板平台标记读取（敏感内容）
//!
//! 分两层，使判定逻辑可在任意平台上单测：
//! - **纯谓词层**：只接收"剪贴板类型名集合 / 平台标志位"这类已取值，不含任何 FFI；
//! - **FFI 薄层**：`#[cfg(target_os = ...)]`，只负责把平台数据类型收集成 `Vec<String>`
//!   或几个布尔/整型，再交给纯谓词判定。
//!
//! 平台依据：
//! - macOS：`NSPasteboard` 的类型集合含 `org.nspasteboard.ConcealedType` 或
//!   `org.nspasteboard.TransientType` 即视为敏感。这是密码管理器（1Password / Bitwarden
//!   等）与一次性内容的通行约定，工具侧不需要（也不应该）猜测内容本身。
//! - Windows：剪贴板格式 `ExcludeClipboardContentFromMonitorProcessing` 存在，
//!   或 `CanIncludeInClipboardHistory` 的 DWORD 值为 0，即视为敏感。
//!
//! 本模块只回答"平台是否声明了敏感标记"，不做任何内容启发式分析。

/// macOS 敏感内容类型：密码管理器填充、一次性口令等会显式置此类型。
pub const CONCEALED_TYPE: &str = "org.nspasteboard.ConcealedType";
/// macOS 一次性内容类型：内容短暂存在，按约定不应进入历史。
pub const TRANSIENT_TYPE: &str = "org.nspasteboard.TransientType";

/// Windows：要求剪贴板监控器忽略本次内容的注册格式名。
#[cfg(target_os = "windows")]
const WIN_EXCLUDE_FORMAT: &str = "ExcludeClipboardContentFromMonitorProcessing";
/// Windows：值为 0 表示不允许进入剪贴板历史（密码管理器常用语义）。
#[cfg(target_os = "windows")]
const WIN_CAN_INCLUDE_HISTORY_FORMAT: &str = "CanIncludeInClipboardHistory";

/// 比较两个剪贴板类型名是否等价。
///
/// 类型名（UTI / 注册格式名）是 ASCII 标识符，故只做**ASCII 大小写折叠 + 去首尾空白**，
/// 不做 Unicode 折叠：这既覆盖了系统的实际比较语义，也避免中文等类型名被误折叠。
fn same_type_name(candidate: &str, expected: &str) -> bool {
    candidate.trim().eq_ignore_ascii_case(expected)
}

/// 判断类型名集合是否命中平台敏感标记。
///
/// 接受任意 `AsRef<str>` 迭代器：生产侧传 `&Vec<String>`，单测可传 `&["..."]`。
#[must_use]
pub fn is_concealed_types<I, S>(types: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    types.into_iter().any(|name| {
        let name = name.as_ref();
        same_type_name(name, CONCEALED_TYPE) || same_type_name(name, TRANSIENT_TYPE)
    })
}

/// 判断 Windows 两个平台信号是否构成"敏感内容"。
///
/// - `exclude_format_present`：`ExcludeClipboardContentFromMonitorProcessing` 格式存在
/// - `can_include_history`：`CanIncludeInClipboardHistory` 的 DWORD 值；格式不存在时为 `None`
///
/// 二者任一命中即敏感。纯函数，故在 macOS 开发机上也能覆盖该分支。
#[must_use]
pub fn is_concealed_windows_signals(
    exclude_format_present: bool,
    can_include_history: Option<u32>,
) -> bool {
    exclude_format_present || can_include_history == Some(0)
}

/// 读取当前剪贴板是否被平台标记为敏感内容。
///
/// 各平台失败路径一律降级为 `false`（宁可记录，也不误删用户内容）：
/// 平台调用失败时报 false，隐私门控仍可用排除名单兜底。
#[must_use]
pub fn read_is_concealed() -> bool {
    #[cfg(target_os = "macos")]
    {
        let types = macos_ffi::pasteboard_type_names();
        is_concealed_types(&types)
    }

    #[cfg(target_os = "windows")]
    {
        windows_ffi::read_is_concealed()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

// macOS 平台 FFI 薄层
#[cfg(target_os = "macos")]
mod macos_ffi {
    use objc2_app_kit::NSPasteboard;

    /// 读取 `NSPasteboard.generalPasteboard()` 当前声明的全部类型名。
    ///
    /// 只收集字符串，不判断含义——判定交给纯谓词层。
    /// 剪贴板为空或无类型时返回空 Vec。
    pub(super) fn pasteboard_type_names() -> Vec<String> {
        let Some(types) = NSPasteboard::generalPasteboard().types() else {
            return Vec::new();
        };
        types.to_vec().iter().map(|name| name.to_string()).collect()
    }
}

// Windows 平台 FFI 薄层
#[cfg(target_os = "windows")]
mod windows_ffi {
    use windows_sys::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    use super::{is_concealed_windows_signals, WIN_CAN_INCLUDE_HISTORY_FORMAT, WIN_EXCLUDE_FORMAT};

    /// 剪贴板打开守卫：持有期间剪贴板保持打开，析构时必定 `CloseClipboard`。
    ///
    /// arboard 在 Windows 上按调用粒度开合剪贴板，二者不会重叠；
    /// 打开失败（其他进程正持有）返回 `None`，由调用方降级。
    struct ClipboardGuard;

    impl ClipboardGuard {
        fn open() -> Option<Self> {
            // 传空 HWND：关联当前任务，不绑定具体窗口。
            let opened = unsafe { OpenClipboard(std::ptr::null_mut()) };
            (opened != 0).then_some(Self)
        }
    }

    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe { CloseClipboard() };
        }
    }

    /// 把 ASCII 格式名转成以 NUL 结尾的 UTF-16 缓冲，供 `RegisterClipboardFormatW` 使用。
    fn wide_format_name(name: &str) -> Vec<u16> {
        name.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 注册（或查询已注册的）剪贴板格式，返回格式 ID；注册失败返回 0。
    ///
    /// `RegisterClipboardFormat` 对同名格式是幂等的：同一次会话内多次调用返回同一 ID。
    fn register_format(name: &str) -> u32 {
        let wide = wide_format_name(name);
        unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
    }

    /// 读取指定格式的 DWORD 值；格式不存在或读取失败返回 `None`。
    fn read_dword_format(format_id: u32) -> Option<u32> {
        if format_id == 0 {
            return None;
        }
        let data: HANDLE = unsafe { GetClipboardData(format_id) };
        if data.is_null() {
            return None;
        }
        // 非 GDI 类型的剪贴板数据是全局内存句柄，需 GlobalLock 后才能解引用。
        let handle: HGLOBAL = data;
        if unsafe { GlobalSize(handle) } < std::mem::size_of::<u32>() {
            return None;
        }
        let pointer = unsafe { GlobalLock(handle) }.cast::<u32>();
        if pointer.is_null() {
            return None;
        }
        let value = unsafe { pointer.read() };
        unsafe { GlobalUnlock(handle) };
        Some(value)
    }

    /// 读取 Windows 平台的敏感标记。
    ///
    /// `IsClipboardFormatAvailable` 不需要先打开剪贴板，故即便随后 `OpenClipboard` 失败，
    /// "要求监控器忽略"这一最强信号仍能读到；DWORD 语义则需要打开剪贴板。
    pub(super) fn read_is_concealed() -> bool {
        let exclude_id = register_format(WIN_EXCLUDE_FORMAT);
        let exclude_present =
            exclude_id != 0 && unsafe { IsClipboardFormatAvailable(exclude_id) } != 0;

        let Some(_guard) = ClipboardGuard::open() else {
            return is_concealed_windows_signals(exclude_present, None);
        };

        let history_id = register_format(WIN_CAN_INCLUDE_HISTORY_FORMAT);
        is_concealed_windows_signals(exclude_present, read_dword_format(history_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concealed_type_hit_returns_true() {
        // Arrange：类型集合含密码管理器约定的 ConcealedType
        let types = ["public.utf8-plain-text", CONCEALED_TYPE, "public.html"];

        // Act & Assert
        assert!(
            is_concealed_types(types),
            "含 {CONCEALED_TYPE} 时应判定为敏感"
        );
    }

    #[test]
    fn transient_type_hit_returns_true() {
        // Arrange：一次性内容走 TransientType 别名
        let types = vec![
            TRANSIENT_TYPE.to_owned(),
            "public.utf8-plain-text".to_owned(),
        ];

        // Act & Assert
        assert!(
            is_concealed_types(&types),
            "含 {TRANSIENT_TYPE} 时应判定为敏感"
        );
    }

    #[test]
    fn ordinary_types_return_false() {
        // Arrange：普通复制只有正文类型，无任何敏感标记
        let types = [
            "public.utf8-plain-text",
            "public.html",
            "public.png",
            "dyn.ah62d4rv4gu8y",
        ];

        // Act & Assert
        assert!(
            !is_concealed_types(types),
            "普通类型集合不应判定为敏感（不做内容启发式）"
        );
    }

    #[test]
    fn empty_type_list_returns_false() {
        // Arrange：空类型集合
        let types: [&str; 0] = [];

        // Act & Assert
        assert!(!is_concealed_types(types), "空集合不应判定为敏感");
    }

    #[test]
    fn case_and_whitespace_variants_hit() {
        // Arrange：类型名大小写与首尾空白变体（平台/第三方实现不保证规范书写）
        let types = [
            "  org.nspasteboard.concealedtype  ",
            "ORG.NSPASTEBOARD.TRANSIENTTYPE",
        ];

        // Act & Assert
        assert!(
            is_concealed_types(types),
            "大小写与首尾空白变体也应命中敏感标记"
        );
    }

    #[test]
    fn similar_but_different_type_does_not_hit() {
        // Arrange：前缀相同但不是敏感标记的类型（防止子串/前缀误判）
        let types = [
            "org.nspasteboard.ConcealedType.extra",
            "org.nspasteboard.Transient",
            "com.quickquick.self-marker",
        ];

        // Act & Assert
        assert!(
            !is_concealed_types(types),
            "仅前缀相似的类型不应命中（等价比较，非包含比较）"
        );
    }

    #[test]
    fn windows_exclude_format_presence_is_concealed() {
        // Act & Assert：只要"要求监控器忽略"格式存在即敏感，与 DWORD 值无关
        assert!(is_concealed_windows_signals(true, None));
        assert!(is_concealed_windows_signals(true, Some(1)));
    }

    #[test]
    fn windows_history_dword_zero_is_concealed() {
        // Act & Assert：CanIncludeInClipboardHistory = 0 表示不允许进历史
        assert!(
            is_concealed_windows_signals(false, Some(0)),
            "DWORD 为 0 时应判定为敏感"
        );
        assert!(
            !is_concealed_windows_signals(false, Some(1)),
            "DWORD 为 1 时不应判定为敏感"
        );
        assert!(
            !is_concealed_windows_signals(false, None),
            "格式不存在且无排除格式时不应判定为敏感"
        );
    }

    /// macOS FFI 取值层的只读冒烟：能连上 pasteboard server 时，
    /// 收集到的每个类型名都必须是非空字符串（空名说明 FFI 取值/转换写错了）。
    ///
    /// 只读、不写、不清理剪贴板，故可在开发机直接跑；无 GUI 会话时
    /// `types()` 返回 None，此时得到空集合，断言同样成立。
    #[cfg(target_os = "macos")]
    #[test]
    fn pasteboard_type_names_are_non_empty_when_readable() {
        // Act
        let names = macos_ffi::pasteboard_type_names();

        // Assert
        assert!(
            names.iter().all(|name| !name.trim().is_empty()),
            "收集到的类型名不应为空字符串，实际为 {names:?}"
        );
    }
}
