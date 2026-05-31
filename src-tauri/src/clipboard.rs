//! 剪贴板捕获引擎（V1-F1-S01 / S03）
//!
//! 设计对齐：设计文档§三#2/#3/#5/#6
//!
//! 核心抽象：
//! - `ClipboardBackend` trait    — 抽象 OS 剪贴板，使逻辑层可脱离 OS 测试
//! - `ClipboardSnapshot`         — 单次读取结果：纯文本 + HTML + 自写标记 + 隐私字段
//! - `CapturedItem`              — 双字段捕获结果：text（纯文本键）+ html（富文本）
//! - `poll_once`                 — 核心判定逻辑：变化检测 + 防自污染 + 双字段构造
//! - `poll_once_with_policy`     — 带隐私策略的捕获：在 poll_once 基础上叠加隐私门控
//!
//! 私有 UTI 标记机制（防自污染）：
//! 工具自写剪贴板时，OS 后端在系统剪贴板中附加一个私有类型标记
//! （macOS: 自定义 NSPasteboard type；Windows: 自定义剪贴板格式 CF_*）。
//! 逻辑层只读取 `has_self_marker` 布尔，不感知具体 OS 实现细节。
//! 这样可在不检查内容的情况下安全跳过工具自身的写入，避免循环捕获。
//!
//! 轮询周期：
//! 实际运行时由后台线程以 `POLL_INTERVAL_MS` 为间隔循环调用 `poll_once`。
//! 单元测试只测 `poll_once` / `poll_once_with_policy` 的判定逻辑，
//! 通过 FakeBackend 构造计数序列驱动，无 sleep、无 OS 依赖。

/// 轮询间隔（毫秒）。运行期 sleep 循环使用，单测不依赖此值。
pub const POLL_INTERVAL_MS: u64 = 500;

/// 单次剪贴板读取结果。
///
/// `text` 与 `html` 均为 `Option`，因为用户可能只复制了其中一种格式。
/// `has_self_marker` 由 OS 后端检测私有 UTI 标记后置为 true；
/// 逻辑层见此标记即跳过捕获，实现防自污染（设计§三#2）。
///
/// `is_concealed`：OS 平台的敏感/一次性标记（二者合并为"敏感"语义）：
/// - macOS：`org.nspasteboard.ConcealedType` 或 `org.nspasteboard.TransientType`
/// - Windows：`ExcludeClipboardContentFromMonitorProcessing` 格式
///
/// OS 后端置位；逻辑层不分析内容，只读取此布尔（设计§三#6）。
///
/// `source_app`：复制动作的来源应用标识（OS 后端提供，如 bundle ID），
/// 供 App 排除名单匹配；后端无法获取时为 `None`。
#[derive(Debug, Clone)]
pub struct ClipboardSnapshot {
    /// 纯文本内容（`text/plain`）
    pub text: Option<String>,
    /// 富文本/HTML 内容（`text/html`）
    pub html: Option<String>,
    /// 是否带本工具私有 UTI 标记（true = 本工具自写，应跳过）
    pub has_self_marker: bool,
    /// 是否携带平台 concealed/transient 标记（true = 敏感，应跳过，不做内容启发式）
    pub is_concealed: bool,
    /// 复制来源应用标识（如 macOS bundle ID），OS 后端无法获取时为 None
    pub source_app: Option<String>,
}

/// 捕获成功后的双字段结果。
///
/// `text` 为纯文本键，用于显示、搜索、判重（设计§三#5）。
/// `html` 为同一次复制的富文本（若有），供预览/粘贴还原格式使用。
#[derive(Debug, Clone, PartialEq)]
pub struct CapturedItem {
    /// 纯文本键（显示/搜索/判重基础字段，必填）
    pub text: String,
    /// 富文本/HTML（同一次复制时存在，供格式保真）
    pub html: Option<String>,
}

/// 抽象 OS 剪贴板后端，使捕获引擎逻辑层与平台解耦、可测。
///
/// 实现者：
/// - 生产：macOS `NSPasteboard` 封装、Windows `OpenClipboard` 封装（s06 实现）
/// - 测试：`FakeBackend`（在测试文件中内联定义）
pub trait ClipboardBackend {
    /// 返回当前剪贴板单调递增变化计数。
    ///
    /// macOS 对应 `NSPasteboard.changeCount`；Windows 对应
    /// `GetClipboardSequenceNumber()`。每次内容变化单调递增，
    /// 轮询逻辑通过比较相邻两次计数是否变化来判断是否需要读取内容。
    fn change_count(&self) -> u64;

    /// 读取当前剪贴板快照（纯文本 + HTML + 自写标记）。
    ///
    /// 仅在 `change_count` 递增时调用，避免频繁 IPC 开销。
    fn read(&self) -> ClipboardSnapshot;
}

/// 单次轮询判定：检查计数变化 → 防自污染过滤 → 构造双字段结果。
///
/// # 判定逻辑（设计§三#3）
///
/// 1. `current <= *last_seen_count`
///    → 无递增，返回 `None`（不读内容，零开销）
///    - 若 `current < *last_seen_count`，说明 OS 计数发生重置
///      （如 Windows `GetClipboardSequenceNumber` 在进程重启后归零），
///      此时将基线下调为 `current`，避免下次计数恢复原值时重复捕获。
/// 2. 计数严格递增（`current > *last_seen_count`）→ 读快照
///    - `snapshot.has_self_marker == true`
///      → 本工具自写，跳过不记；但仍更新 `last_seen_count`（防重复触发）
///    - `snapshot.text` 有值
///      → 返回 `CapturedItem { text, html }`（双字段同存，A01）
///    - `snapshot.text` 为 `None`
///      → 无纯文本内容，跳过（非文本格式留待后续 s 实现）
///
/// # 参数
/// - `backend`         — 剪贴板后端（可为 fake）
/// - `last_seen_count` — 上次已处理的计数（in/out，函数内更新）
///
/// # 返回
/// 捕获到新文本内容时返回 `Some(CapturedItem)`，否则 `None`。
pub fn poll_once(
    backend: &dyn ClipboardBackend,
    last_seen_count: &mut u64,
) -> Option<CapturedItem> {
    let current = backend.change_count();

    if current <= *last_seen_count {
        if current < *last_seen_count {
            // OS 计数重置（如 Windows GetClipboardSequenceNumber 进程重启），
            // 将基线下调为 current，避免计数恢复后误判为变化而重复捕获。
            *last_seen_count = current;
        }
        return None;
    }

    // 计数已递增：读取快照并判定
    let snapshot = backend.read();

    // 无论是否跳过，均推进 last_seen_count，防止下次轮询重复触发
    *last_seen_count = current;

    if snapshot.has_self_marker {
        // 本工具自写剪贴板（私有 UTI 标记），跳过不记（防自污染，A03）
        return None;
    }

    // 纯文本为必填键；无文本则跳过（非文本格式后续处理）
    let text = snapshot.text?;

    Some(CapturedItem {
        text,
        html: snapshot.html,
    })
}

/// 带隐私策略的单次轮询判定。
///
/// 在 `poll_once` 的"严格递增计数"判定基础上，读取快照后调用
/// [`crate::privacy::should_skip`] 进行隐私门控。命中任意跳过规则时，
/// 仍推进 `last_seen_count`（防止下次重复触发），但返回 `None`。
///
/// # 判定顺序
///
/// 1. `current <= *last_seen_count` → 无递增，返回 `None`（同 poll_once）
/// 2. 读取快照，推进 `last_seen_count`
/// 3. `privacy::should_skip(snapshot, policy)` 命中 → 返回 `None`（跳过不记）
/// 4. `snapshot.text` 有值 → 返回 `Some(CapturedItem)`
/// 5. 无纯文本 → 返回 `None`
///
/// # 参数
/// - `backend`         — 剪贴板后端（可为 fake）
/// - `last_seen_count` — 上次已处理的计数（in/out）
/// - `policy`          — 隐私捕获策略（暂停开关 + 排除名单）
pub fn poll_once_with_policy(
    backend: &dyn ClipboardBackend,
    last_seen_count: &mut u64,
    policy: &crate::privacy::CapturePolicy<'_>,
) -> Option<CapturedItem> {
    let current = backend.change_count();

    if current <= *last_seen_count {
        if current < *last_seen_count {
            *last_seen_count = current;
        }
        return None;
    }

    let snapshot = backend.read();
    *last_seen_count = current;

    if crate::privacy::should_skip(&snapshot, policy).is_some() {
        return None;
    }

    let text = snapshot.text?;
    Some(CapturedItem {
        text,
        html: snapshot.html,
    })
}
