//! 启动数据管道（V4-F1-S04 / V5-F1-S02 捕获层）
//!
//! 设计对齐：设计文档§六（开库路径）、§三（捕获-入库管道）
//!
//! 职责：
//! - `open_app_db`        — 取密钥 → 开/建加密库（纯函数，可注入 fake KeyProvider）
//! - `capture_and_ingest` — 轮询剪贴板 → 有新内容则写库（纯函数，可注入 fake backend）
//! - `ArboardBackend`     — 生产剪贴板后端（arboard crate 跨平台实现）
//!
//! 设计原则：
//! open_app_db / capture_and_ingest 均为纯依赖注入风格，不持有任何全局状态，
//! 便于单测在不触碰真实 OS 钥匙串、真实剪贴板的情况下驱动完整管道逻辑。
//! 生产接线（ArboardBackend + KeychainKeyProvider + 轮询线程）在 lib.rs 完成。

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::clipboard::{CapturedClip, ClipboardBackend, ClipboardSnapshot, RawImageData};
use crate::db::{self, IngestOutcome};
use crate::keyprovider::KeyProvider;
use crate::privacy::CapturePolicy;

/// FNV-1a 64-bit 素数（与 db.rs text_hash 使用同一常量，保证跨模块算法一致）
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
/// FNV-1a 64-bit 偏移基础值
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// 计算字节切片的 FNV-1a 64-bit 哈希（显式稳定算法，非 Rust 默认 hash）。
///
/// 用于 ArboardBackend 的变化检测：将剪贴板内容哈希与上次比较，
/// 值变化时内部计数单调+1，模拟 macOS NSPasteboard.changeCount 语义。
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// 生产剪贴板后端（arboard 跨平台实现）。
///
/// `change_count` 语义：读取当前剪贴板文本与图片，用复合 FNV-1a 哈希与上次比较；
/// 内容变化时内部单调计数+1，返回该计数。headless 环境无系统剪贴板时
/// arboard 会返回错误，此处降级为返回上次计数（不触发误捕）。
///
/// # 复合指纹
/// 文本字节与图片 RGBA 字节按固定顺序参与 FNV-1a 哈希；
/// 两者皆无时返回旧 count；任一变化则 count+1。
/// 图片直接对 RGBA 字节哈希（不转 PNG，避免编码开销）。
///
/// # 真实运行归 pending-manual
/// arboard 需要 GUI 环境，不为其编写联网/GUI 自动化测试。
pub struct ArboardBackend {
    /// arboard Clipboard 实例，Mutex 保护跨线程访问
    clipboard: Mutex<arboard::Clipboard>,
    /// 上次读取的复合内容哈希，用于变化检测
    last_hash: Mutex<u64>,
    /// 单调递增的变化计数
    count: Mutex<u64>,
}

impl ArboardBackend {
    /// 创建 ArboardBackend 实例。
    ///
    /// # Errors
    /// arboard::Clipboard::new() 在无 GUI 环境（headless CI）可能失败，
    /// 调用方应优雅降级（eprintln，不 panic）。
    #[must_use = "ArboardBackend::new 返回 Result，忽略它会导致初始化失败被静默丢弃"]
    pub fn new() -> Result<Self, String> {
        let clipboard = arboard::Clipboard::new().map_err(|e| format!("剪贴板初始化失败：{e}"))?;
        Ok(Self {
            clipboard: Mutex::new(clipboard),
            last_hash: Mutex::new(0),
            count: Mutex::new(0),
        })
    }

    /// 计算当前剪贴板内容的复合 FNV-1a 指纹。
    ///
    /// 将文本字节与图片 RGBA 字节按固定顺序送入 `fnv1a_64`，
    /// 保证文本或图片任一变化都会改变指纹值。
    /// 两者皆无时返回 `None`（不更新计数）。
    fn compute_composite_hash(cb: &mut arboard::Clipboard) -> Option<u64> {
        let text = cb.get_text().ok();
        let image = cb.get_image().ok();

        if text.is_none() && image.is_none() {
            return None;
        }

        // 将文本字节与图片字节拼接后统一哈希，顺序固定保证确定性
        let mut combined: Vec<u8> = Vec::new();
        if let Some(ref t) = text {
            combined.extend_from_slice(t.as_bytes());
        }
        // 分隔符：避免 "ab"+"c" 与 "a"+"bc" 哈希值相同
        combined.push(0xFF);
        if let Some(ref img) = image {
            // 直接对 RGBA 字节哈希，不转 PNG（避免编码开销）
            combined.extend_from_slice(&img.bytes);
        }

        Some(fnv1a_64(&combined))
    }
}

impl ClipboardBackend for ArboardBackend {
    fn change_count(&self) -> u64 {
        let Ok(mut cb) = self.clipboard.lock() else {
            return *self.count.lock().unwrap_or_else(|e| e.into_inner());
        };

        let Some(new_hash) = Self::compute_composite_hash(&mut cb) else {
            return *self.count.lock().unwrap_or_else(|e| e.into_inner());
        };

        let mut last_hash = self.last_hash.lock().unwrap_or_else(|e| e.into_inner());
        let mut count = self.count.lock().unwrap_or_else(|e| e.into_inner());

        if new_hash != *last_hash {
            *last_hash = new_hash;
            *count += 1;
        }

        *count
    }

    fn read(&self) -> ClipboardSnapshot {
        let Ok(mut cb) = self.clipboard.lock() else {
            return ClipboardSnapshot {
                text: None,
                html: None,
                image: None,
                has_self_marker: false,
                is_concealed: false,
                source_app: None,
            };
        };

        let text = cb.get_text().ok();

        // get_image 在 headless 或无图片时返回 Err（ContentNotAvailable 等），
        // 用 .ok() 降级为 None，不 panic。
        let image = cb.get_image().ok().map(|img_data| RawImageData {
            width: img_data.width,
            height: img_data.height,
            bytes: img_data.bytes.into_owned(),
        });

        ClipboardSnapshot {
            text,
            html: None,
            image,
            has_self_marker: false,
            is_concealed: false,
            source_app: None,
        }
    }
}

/// 打开（或新建）应用加密数据库。
///
/// 流程：通过 `key_provider` 取得或生成 256-bit 密钥 → 调用 `db::open_or_create`。
/// 错误统一转换为 `String`，方便 Tauri 命令层 `?` 传播。
///
/// # 为什么错误转 String
/// 调用方（lib.rs setup）需要将 KeyError 与 DbError 两类不同错误统一处理，
/// 以 String 为公共分母可避免在启动路径引入额外错误枚举。
///
/// # Errors
/// - 密钥获取失败（钥匙串不可用）：返回 KeyError 描述
/// - 数据库打开/创建失败（路径非法、解密失败）：返回 DbError 描述
pub fn open_app_db(key_provider: &dyn KeyProvider, db_path: &Path) -> Result<Connection, String> {
    let key = key_provider
        .get_or_create_key()
        .map_err(|e| format!("密钥获取失败：{e}"))?;

    db::open_or_create(db_path, &key).map_err(|e| format!("数据库打开失败：{e}"))
}

/// 单次剪贴板捕获并原子写库。
///
/// 调用 `poll_once_with_policy` 获取捕获结果列表（空 Vec 表示无新内容）：
/// - 有新内容 → 在 SAVEPOINT 内逐项写库；同一次捕获的多条原子写入，任一失败整体回滚
/// - 无新内容（计数未变、策略跳过等）→ 返回 `Ok(vec![])`，不开事务
///
/// # 为什么用 SAVEPOINT 而非 Transaction
/// `db::ingest_image_as_clip` 内部已有嵌套 SAVEPOINT（`ingest_image_clip`），
/// 若外层用 `conn.transaction()` 会因借用冲突无法将同一连接传入子函数。
/// SAVEPOINT 通过 `execute_batch` 字符串操作规避借用，支持嵌套，两级名称不同（`capture_ingest` vs `ingest_image_clip`）。
///
/// # 为什么返回 Vec<IngestOutcome>
/// 混合图文复制会在一次捕获中产生文本条目和图片条目两个结果，
/// 需要分别写库并收集各自的 IngestOutcome，Vec 是最自然的容器。
///
/// # Errors
/// `db::ingest` 或 `db::ingest_image_as_clip` 失败时，整体回滚后返回 DbError 描述字符串。
pub fn capture_and_ingest(
    backend: &dyn ClipboardBackend,
    last_seen: &mut u64,
    conn: &Connection,
    policy: &CapturePolicy<'_>,
    max_image_bytes: u64,
) -> Result<Vec<IngestOutcome>, String> {
    let clips = crate::clipboard::poll_once_with_policy(backend, last_seen, policy);
    if clips.is_empty() {
        return Ok(Vec::new());
    }

    conn.execute_batch("SAVEPOINT capture_ingest;")
        .map_err(|e| format!("开启事务失败：{e}"))?;

    match ingest_clips(conn, clips, max_image_bytes) {
        Ok(outcomes) => {
            conn.execute_batch("RELEASE SAVEPOINT capture_ingest;")
                .map_err(|e| format!("提交事务失败：{e}"))?;
            Ok(outcomes)
        }
        Err(e) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT capture_ingest; RELEASE SAVEPOINT capture_ingest;",
            );
            Err(e)
        }
    }
}

/// 逐条写库（拆出以保持主函数 ≤50 行、事务边界清晰）。
///
/// 在调用方的 SAVEPOINT 保护下运行，任一条失败则由调用方回滚整批。
/// `max_image_bytes`：图片原图大小上限，由调用方从 AppSettings 读取后传入。
fn ingest_clips(
    conn: &Connection,
    clips: Vec<CapturedClip>,
    max_image_bytes: u64,
) -> Result<Vec<IngestOutcome>, String> {
    let mut outcomes = Vec::with_capacity(clips.len());
    for clip in clips {
        let outcome = match clip {
            CapturedClip::Text(item) => {
                db::ingest(conn, &item).map_err(|e| format!("写库失败（文本）：{e}"))?
            }
            CapturedClip::Image {
                width,
                height,
                png_bytes,
            } => db::ingest_image_as_clip(conn, width, height, &png_bytes, max_image_bytes)
                .map_err(|e| format!("写库失败（图片）：{e}"))?,
        };
        outcomes.push(outcome);
    }
    Ok(outcomes)
}
