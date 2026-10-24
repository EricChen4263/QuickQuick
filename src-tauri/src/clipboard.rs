//! 剪贴板捕获引擎（V1-F1-S01 / S03 / V5-F1-S02）
//!
//! 设计对齐：设计文档§三#2/#3/#5/#6，V5 图片捕获层
//!
//! 核心抽象：
//! - `ClipboardBackend` trait    — 抽象 OS 剪贴板，使逻辑层可脱离 OS 测试
//! - `ClipboardSnapshot`         — 单次读取结果：纯文本 + HTML + 图片 + 自写标记 + 隐私字段
//! - `SourceApp`                 — 复制来源应用身份（bundle id + 本地化显示名）
//! - `CapturedItem`              — 双字段捕获结果：text（纯文本键）+ html（富文本）
//! - `CapturedClip`              — 捕获结果枚举：文本条目或图片条目
//! - `RawImageData`              — 剪贴板图片原始 RGBA 字节
//! - `poll_once`                 — 核心判定逻辑（返回 Vec<CapturedClip>）
//! - `poll_once_with_policy`     — 带隐私策略的捕获（返回 Vec<CapturedClip>）
//! - `observe_backend`           — 无锁观察：计数变了才读快照，得到 `ObservedClipboard`
//!
//! 私有 UTI 标记机制（防自污染）：
//! 工具自写剪贴板时，在系统剪贴板中留下"这是 QuickQuick 写的"标记
//! （macOS: 追加自定义 NSPasteboard 类型 `com.quickquick.self-marker`；
//! Windows/Linux: arboard 独占写剪贴板、无法追加自定义格式，改用写入门闩，
//! 让下一次变化被识别为本进程写入）。
//! 逻辑层只读取 `has_self_marker` 布尔，不感知具体 OS 实现细节。
//! 这样可在不检查内容的情况下安全跳过工具自身的写入，避免循环捕获。
//!
//! 轮询周期：
//! 实际运行时由后台线程以 `POLL_INTERVAL_MS` 为间隔循环：先 `observe_backend`
//! （无锁，只在计数变化时读一次快照），再把得到的 `ObservedClipboard` 交给
//! `poll_once_with_policy` / `capture_and_ingest` 完成隐私门控与入库。
//! 单元测试只测这些函数的判定逻辑，
//! 通过 FakeBackend 构造计数序列驱动，无 sleep、无 OS 依赖。

use std::cell::RefCell;
use std::io::Cursor;

use image::ImageEncoder;

/// 轮询间隔（毫秒）。运行期 sleep 循环使用，单测不依赖此值。
pub const POLL_INTERVAL_MS: u64 = 500;

/// 剪贴板图片的原始 RGBA 字节。
///
/// `bytes` 长度必须等于 `width * height * 4`（每像素 4 字节：R, G, B, A）。
/// 由 OS 后端填充，逻辑层负责转换为 PNG 后写库。
#[derive(Debug, Clone)]
pub struct RawImageData {
    /// 图片宽度（像素）
    pub width: usize,
    /// 图片高度（像素）
    pub height: usize,
    /// 原始 RGBA 像素字节，len = width * height * 4
    pub bytes: Vec<u8>,
}

/// 复制来源应用的身份。
///
/// 两个字段都可能缺失，取决于平台与运行环境：
/// - `bundle_id`：macOS bundle identifier（如 `com.1password.1password`），最稳定；
/// - `display_name`：应用本地化显示名（如 `1Password`），也用于 Windows 可执行文件名
///   （`1Password`，已去目录与 `.exe` 后缀）。
///
/// 为什么两者都要：设置页让用户输入的是**人类可读名称**，而历史匹配需要稳定的 bundle id；
/// 只提供其中一个都会让「用户照提示输入却永不命中」。排除名单匹配两者任一项即可
/// （见 [`crate::privacy::ExcludeList::matches_source`]）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceApp {
    /// bundle identifier（macOS 提供；Windows 无此概念时为 `None`）
    pub bundle_id: Option<String>,
    /// 本地化显示名（macOS）或可执行文件名主干（Windows）
    pub display_name: Option<String>,
}

impl SourceApp {
    /// 返回用于写入 [`ClipboardSnapshot::source_app`] 的单一标识（优先 bundle id）。
    ///
    /// `ClipboardSnapshot` 只保留单值字段以维持既有契约；需要显示名参与匹配的场景
    /// 走 [`ClipboardBackend::source_app_identity`] 取得完整身份。
    #[must_use]
    pub fn primary_identifier(&self) -> Option<String> {
        self.bundle_id.clone().or_else(|| self.display_name.clone())
    }
}

/// 单次剪贴板读取结果。
///
/// `text` 与 `html` 均为 `Option`，因为用户可能只复制了其中一种格式。
/// `image` 为原始 RGBA 图片数据；纯文本复制时为 `None`。
/// `has_self_marker` 由 OS 后端检测私有 UTI 标记后置为 true；
/// 逻辑层见此标记即跳过捕获，实现防自污染（设计§三#2）。
///
/// `is_concealed`：OS 平台的敏感/一次性标记（二者合并为"敏感"语义）：
/// - macOS：`org.nspasteboard.ConcealedType` 或 `org.nspasteboard.TransientType`
/// - Windows：`ExcludeClipboardContentFromMonitorProcessing` 格式存在，
///   或 `CanIncludeInClipboardHistory` 的 DWORD 值为 0
///
/// OS 后端置位（判定与平台取值都在 [`crate::pasteboard_meta`]）；
/// 逻辑层不分析内容，只读取此布尔（设计§三#6）。
///
/// `source_app`：复制动作的来源应用标识（macOS 为 bundle ID，Windows 为可执行文件名主干），
/// 供 App 排除名单匹配；后端无法获取时为 `None`。需要「bundle id + 显示名」双匹配时
/// 另用 [`ClipboardBackend::source_app_identity`]。
#[derive(Debug, Clone)]
pub struct ClipboardSnapshot {
    /// 纯文本内容（`text/plain`）
    pub text: Option<String>,
    /// 富文本/HTML 内容（`text/html`）
    pub html: Option<String>,
    /// 原始图片数据（RGBA 字节）；无图片时为 None
    pub image: Option<RawImageData>,
    /// 是否带本工具私有 UTI 标记（true = 本工具自写，应跳过）
    pub has_self_marker: bool,
    /// 是否携带平台 concealed/transient 标记（true = 敏感，应跳过，不做内容启发式）
    pub is_concealed: bool,
    /// 复制来源应用标识（如 macOS bundle ID），OS 后端无法获取时为 None
    pub source_app: Option<String>,
}

impl ClipboardSnapshot {
    /// 全空快照：没有任何内容、也没有任何标记。
    #[must_use]
    pub fn empty() -> Self {
        Self {
            text: None,
            html: None,
            image: None,
            has_self_marker: false,
            is_concealed: false,
            source_app: None,
        }
    }
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

/// 复制/粘贴写回剪贴板的载荷，区分文本与图片两态。
///
/// 为何不直接扩 `CapturedItem`：后者是捕获/入库链路（ingest、db、pipeline）
/// 广泛复用的核心结构，只承载 text+html。复制/粘贴是写回剪贴板的边界场景，
/// 图片需携带宽高与裸 RGBA 字节——单独建模避免把图片字段污染进入库主路径，
/// 也让写剪贴板的两条分支（文本走 set/html，图片走 set_image）类型清晰。
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardPayload {
    /// 文本载荷（含可选 HTML 富文本），写回走 arboard set_text / set_html。
    Text(CapturedItem),
    /// 图片载荷（裸 RGBA 字节），写回走 arboard set_image。
    Image {
        /// 图片宽度（像素）
        width: usize,
        /// 图片高度（像素）
        height: usize,
        /// 裸 RGBA 像素字节，len = width * height * 4
        rgba: Vec<u8>,
    },
}

/// 单次捕获结果枚举，区分文本条目与图片条目。
///
/// 同一次复制可能既含文本也含图片（混合格式），`poll_once_with_policy`
/// 返回 `Vec<CapturedClip>`，每条独立入库。
#[derive(Debug, Clone, PartialEq)]
pub enum CapturedClip {
    /// 文本（含可选 HTML 富文本）
    Text(CapturedItem),
    /// 图片（已编码为 PNG 字节）
    Image {
        /// 图片宽度（像素）
        width: usize,
        /// 图片高度（像素）
        height: usize,
        /// PNG 编码字节
        png_bytes: Vec<u8>,
    },
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

    /// 读取当前剪贴板快照（纯文本 + HTML + 图片 + 自写标记）。
    ///
    /// 仅在 `change_count` 递增时调用，避免频繁 IPC 开销。
    fn read(&self) -> ClipboardSnapshot;

    /// 在一次访问里取「读取时刻的计数 + 快照」；读取失败返回 `None`。
    ///
    /// `seen_count` 是调用方刚从 [`ClipboardBackend::change_count`] 拿到的值。
    /// 默认实现把它原样当作读取时刻的计数，并调用 [`ClipboardBackend::read`]——
    /// 对计数与读取各自独立的后端（测试 fake、Windows 的内容指纹后端）等价于旧行为。
    ///
    /// 必须覆盖它的后端：计数与读取要在**同一次**平台访问里取（macOS 串行线程），
    /// 否则「读到计数」与「读到内容」之间剪贴板再变一次，会让下一轮把同一内容再读一遍。
    /// 返回 `None` 表示这次没读到（串行线程忙或超时）：调用方**不得推进**基线，
    /// 下一轮自然重试，而不是把一次读取失败当成「剪贴板为空」吞掉。
    fn read_observed(&self, seen_count: u64) -> Option<(u64, ClipboardSnapshot)> {
        Some((seen_count, self.read()))
    }

    /// 返回本次变化的来源应用完整身份（bundle id + 本地化显示名）。
    ///
    /// 默认 `None`：测试后端与不提供应用身份的平台无需实现。
    /// 生产后端实现的是**缓存读取**：身份在 [`ClipboardBackend::read`] 里随平台元数据
    /// 一次取回并缓存，本方法只返回缓存值，故必须在同一次 `read` 之后调用
    /// （`poll_once_with_policy` 的调用顺序保证了这一点）。
    ///
    /// 为什么不放进 [`ClipboardSnapshot`]：该结构被跨 crate 的测试直接穷尽构造，
    /// 新增字段会连带破坏所有构造点；身份作为独立的可选通道更小侵入。
    fn source_app_identity(&self) -> Option<SourceApp> {
        None
    }
}

/// 一次「无锁观察」的结果：某一时刻的计数，以及（计数前进时）当时读到的整份快照。
///
/// 它本身是一个只回放既成事实的 [`ClipboardBackend`]：`change_count` 返回观察时的计数，
/// `read` 交出观察时读到的快照（**只能取一次**，避免对大图再克隆一份）。
/// 于是轮询循环可以把「碰剪贴板」（可能要等主线程）与「取数据库锁、套隐私策略、入库」
/// 拆成两个阶段，且阶段二照旧调用 [`poll_once_with_policy`] / `capture_and_ingest`，
/// 判定语义（基线推进、计数回退、隐私门控、去重）一行都不用复制。
pub struct ObservedClipboard {
    count: u64,
    snapshot: RefCell<Option<ClipboardSnapshot>>,
    source_app: Option<SourceApp>,
}

impl ObservedClipboard {
    /// 只带计数、不带快照：用于计数回退——轮询逻辑见到回退只会下调基线，不会读内容。
    #[must_use]
    pub fn count_only(count: u64) -> Self {
        Self {
            count,
            snapshot: RefCell::new(None),
            source_app: None,
        }
    }

    /// 带快照与来源应用身份。
    #[must_use]
    pub fn with_snapshot(
        count: u64,
        snapshot: ClipboardSnapshot,
        source_app: Option<SourceApp>,
    ) -> Self {
        Self {
            count,
            snapshot: RefCell::new(Some(snapshot)),
            source_app,
        }
    }
}

impl ClipboardBackend for ObservedClipboard {
    fn change_count(&self) -> u64 {
        self.count
    }

    /// 交出观察时读到的快照；已被取走（或本就没读）时返回全空快照。
    fn read(&self) -> ClipboardSnapshot {
        self.snapshot
            .borrow_mut()
            .take()
            .unwrap_or_else(ClipboardSnapshot::empty)
    }

    fn source_app_identity(&self) -> Option<SourceApp> {
        self.source_app.clone()
    }
}

/// 无锁观察：只在计数相对基线发生变化时才读内容。
///
/// - 计数等于基线 → `None`：本轮什么都不用做，调用方连数据库锁都不必取；
/// - 计数回退 → 只带计数返回，交给后续判定下调基线（不读内容）；
/// - 计数前进 → 经 [`ClipboardBackend::read_observed`] 读一份快照；读取失败返回 `None`，
///   **基线保持不动**，下一轮自然重试。
///
/// 调用方在整个观察期间不得持有主线程也可能争用的锁，见 `pasteboard_serial` 模块文档。
#[must_use]
pub fn observe_backend(
    backend: &dyn ClipboardBackend,
    last_seen_count: u64,
) -> Option<ObservedClipboard> {
    let current = backend.change_count();
    if current == last_seen_count {
        return None;
    }
    if current < last_seen_count {
        return Some(ObservedClipboard::count_only(current));
    }

    let (count, snapshot) = backend.read_observed(current)?;
    let source_app = backend.source_app_identity();
    Some(ObservedClipboard::with_snapshot(
        count, snapshot, source_app,
    ))
}

/// 将 RGBA 原始字节编码为 PNG。
///
/// 字节长度不等于 `width * height * 4` 或编码失败时返回 `None`，
/// 并 eprintln 警告，不 panic。
fn rgba_to_png(width: usize, height: usize, rgba: &[u8]) -> Option<Vec<u8>> {
    let expected_len = width * height * 4;
    if rgba.len() != expected_len {
        eprintln!(
            "[clipboard] rgba_to_png: 字节长度不符，期望 {expected_len}，实际 {}",
            rgba.len()
        );
        return None;
    }

    let img = image::RgbaImage::from_raw(width as u32, height as u32, rgba.to_vec())?;

    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(Cursor::new(&mut buf));
    if let Err(e) = encoder.write_image(
        img.as_raw(),
        width as u32,
        height as u32,
        image::ExtendedColorType::Rgba8,
    ) {
        eprintln!("[clipboard] rgba_to_png: PNG 编码失败：{e}");
        return None;
    }

    Some(buf)
}

/// 测试专用导出：暴露私有 `rgba_to_png`，验证编码正确性。
///
/// 仅供集成测试调用，不用于生产路径。
/// 函数本身无副作用，公开导出不影响安全性。
#[doc(hidden)]
pub fn rgba_to_png_for_test(width: usize, height: usize, rgba: &[u8]) -> Option<Vec<u8>> {
    rgba_to_png(width, height, rgba)
}

/// 将 PNG 字节解码为 (宽, 高, RGBA 字节)。
///
/// 复制/粘贴图片到剪贴板时，DB 里存的是 PNG BLOB，而 arboard `set_image`
/// 需要的是裸 RGBA 字节（与捕获侧 `get_image()` 读到的 RGBA 一致），
/// 故写回前需把 PNG 解码回 RGBA。本函数为纯函数、无副作用，便于单测。
///
/// 解码失败（非法字节、非图片数据等）返回带原因的错误字符串，不 panic——
/// 调用方（IPC 取数层）据此向前端报"图片无法写回剪贴板"。
///
/// # Errors
/// PNG 解析失败时返回 `Err(String)`，包含 `image` crate 的原始错误说明。
pub fn png_to_rgba(png: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    let img = image::load_from_memory(png).map_err(|e| format!("PNG 解码失败：{e}"))?;
    let rgba = img.to_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
    Ok((width, height, rgba.into_raw()))
}

/// 将快照的文本/图片字段拆分为 `Vec<CapturedClip>`。
///
/// 判定顺序：
/// 1. `snapshot.text` 有值 → push `CapturedClip::Text`
/// 2. `snapshot.image` 有值且 `rgba_to_png` 成功 → push `CapturedClip::Image`
///
/// 结果：纯文本=[Text]，纯图=[Image]，图文=[Text,Image]，两者皆无=[]。
/// 测试专用导出：暴露私有 `snapshot_to_clips`，锚定 html 透传不丢。
///
/// 仅供集成测试调用，不用于生产路径。函数无副作用，公开导出不影响安全性。
#[doc(hidden)]
#[must_use]
pub fn snapshot_to_clips_for_test(snapshot: ClipboardSnapshot) -> Vec<CapturedClip> {
    snapshot_to_clips(snapshot)
}

fn snapshot_to_clips(snapshot: ClipboardSnapshot) -> Vec<CapturedClip> {
    let mut clips = Vec::new();

    if let Some(text) = snapshot.text {
        clips.push(CapturedClip::Text(CapturedItem {
            text,
            html: snapshot.html,
        }));
    }

    if let Some(img) = snapshot.image {
        if let Some(png_bytes) = rgba_to_png(img.width, img.height, &img.bytes) {
            clips.push(CapturedClip::Image {
                width: img.width,
                height: img.height,
                png_bytes,
            });
        }
    }

    clips
}

/// 单次轮询判定：检查计数变化 → 防自污染过滤 → 构造捕获结果列表。
///
/// # 判定逻辑（设计§三#3，V5 扩展）
///
/// 1. `current <= *last_seen_count`
///    → 无递增，返回空 Vec（不读内容，零开销）
///    - 若 `current < *last_seen_count`，说明 OS 计数发生重置，
///      将基线下调为 `current`，避免下次计数恢复原值时重复捕获。
/// 2. 计数严格递增（`current > *last_seen_count`）→ 读快照，推进 `last_seen_count`
///    - `snapshot.has_self_marker == true` → 跳过，返回空 Vec
///    - 否则：text 有值 → push Text；image 有值且编码成功 → push Image
///
/// # 参数
/// - `backend`         — 剪贴板后端（可为 fake）
/// - `last_seen_count` — 上次已处理的计数（in/out，函数内更新）
///
/// # 返回
/// 捕获到内容时返回非空 Vec，否则空 Vec。
pub fn poll_once(backend: &dyn ClipboardBackend, last_seen_count: &mut u64) -> Vec<CapturedClip> {
    let current = backend.change_count();

    if current <= *last_seen_count {
        if current < *last_seen_count {
            // OS 计数重置（如 Windows GetClipboardSequenceNumber 进程重启），
            // 将基线下调为 current，避免计数恢复后误判为变化而重复捕获。
            *last_seen_count = current;
        }
        return Vec::new();
    }

    let snapshot = backend.read();
    // 无论是否跳过，均推进 last_seen_count，防止下次轮询重复触发
    *last_seen_count = current;

    if snapshot.has_self_marker {
        // 本工具自写剪贴板（私有 UTI 标记），跳过不记（防自污染，A03）
        return Vec::new();
    }

    snapshot_to_clips(snapshot)
}

/// 带隐私策略的单次轮询判定。
///
/// 在 `poll_once` 的"严格递增计数"判定基础上，读取快照后调用
/// [`crate::privacy::should_skip_with_source`] 进行隐私门控。命中任意跳过规则时，
/// 仍推进 `last_seen_count`（防止下次重复触发），但返回空 Vec。
///
/// # 判定顺序
///
/// 1. `current <= *last_seen_count` → 无递增，返回空 Vec（同 poll_once）
/// 2. 读取快照，推进 `last_seen_count`
/// 3. 取来源应用完整身份（用于显示名匹配）
/// 4. `privacy::should_skip_with_source(...)` 命中 → 返回空 Vec（跳过不记）
/// 5. 按 snapshot_to_clips 拆分文本/图片，返回结果 Vec
///
/// # 参数
/// - `backend`         — 剪贴板后端（可为 fake）
/// - `last_seen_count` — 上次已处理的计数（in/out）
/// - `policy`          — 隐私捕获策略（暂停开关 + 排除名单）
pub fn poll_once_with_policy(
    backend: &dyn ClipboardBackend,
    last_seen_count: &mut u64,
    policy: &crate::privacy::CapturePolicy<'_>,
) -> Vec<CapturedClip> {
    let current = backend.change_count();

    if current <= *last_seen_count {
        if current < *last_seen_count {
            *last_seen_count = current;
        }
        return Vec::new();
    }

    let snapshot = backend.read();
    *last_seen_count = current;

    // 排除名单为空时 should_skip_with_source 内部直接短路，不会碰这里的身份查询结果
    let source_app = backend.source_app_identity();
    if crate::privacy::should_skip_with_source(&snapshot, policy, source_app.as_ref()).is_some() {
        return Vec::new();
    }

    snapshot_to_clips(snapshot)
}
