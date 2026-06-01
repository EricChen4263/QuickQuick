//! 剪贴板捕获引擎（V1-F1-S01 / S03 / V5-F1-S02）
//!
//! 设计对齐：设计文档§三#2/#3/#5/#6，V5 图片捕获层
//!
//! 核心抽象：
//! - `ClipboardBackend` trait    — 抽象 OS 剪贴板，使逻辑层可脱离 OS 测试
//! - `ClipboardSnapshot`         — 单次读取结果：纯文本 + HTML + 图片 + 自写标记 + 隐私字段
//! - `CapturedItem`              — 双字段捕获结果：text（纯文本键）+ html（富文本）
//! - `CapturedClip`              — 捕获结果枚举：文本条目或图片条目
//! - `RawImageData`              — 剪贴板图片原始 RGBA 字节
//! - `poll_once`                 — 核心判定逻辑（返回 Vec<CapturedClip>）
//! - `poll_once_with_policy`     — 带隐私策略的捕获（返回 Vec<CapturedClip>）
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

/// 单次剪贴板读取结果。
///
/// `text` 与 `html` 均为 `Option`，因为用户可能只复制了其中一种格式。
/// `image` 为原始 RGBA 图片数据；纯文本复制时为 `None`。
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
    /// 原始图片数据（RGBA 字节）；无图片时为 None
    pub image: Option<RawImageData>,
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

/// 将快照的文本/图片字段拆分为 `Vec<CapturedClip>`。
///
/// 判定顺序：
/// 1. `snapshot.text` 有值 → push `CapturedClip::Text`
/// 2. `snapshot.image` 有值且 `rgba_to_png` 成功 → push `CapturedClip::Image`
///
/// 结果：纯文本=[Text]，纯图=[Image]，图文=[Text,Image]，两者皆无=[]。
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
/// [`crate::privacy::should_skip`] 进行隐私门控。命中任意跳过规则时，
/// 仍推进 `last_seen_count`（防止下次重复触发），但返回空 Vec。
///
/// # 判定顺序
///
/// 1. `current <= *last_seen_count` → 无递增，返回空 Vec（同 poll_once）
/// 2. 读取快照，推进 `last_seen_count`
/// 3. `privacy::should_skip(snapshot, policy)` 命中 → 返回空 Vec（跳过不记）
/// 4. 按 snapshot_to_clips 拆分文本/图片，返回结果 Vec
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

    if crate::privacy::should_skip(&snapshot, policy).is_some() {
        return Vec::new();
    }

    snapshot_to_clips(snapshot)
}
