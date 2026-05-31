//! 启动数据管道（V4-F1-S04）
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

use crate::clipboard::{ClipboardBackend, ClipboardSnapshot};
use crate::db::{self, IngestOutcome};
use crate::keyprovider::KeyProvider;
use crate::privacy::CapturePolicy;

/// FNV-1a 64-bit 素数（与 db.rs text_hash 使用同一常量，保证跨模块算法一致）
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
/// FNV-1a 64-bit 偏移基础值
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// 计算字节切片的 FNV-1a 64-bit 哈希（显式稳定算法，非 Rust 默认 hash）。
///
/// 用于 ArboardBackend 的变化检测：将剪贴板文本哈希与上次比较，
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
/// `change_count` 语义：读取当前剪贴板文本，用 FNV-1a 哈希与上次比较；
/// 内容变化时内部单调计数+1，返回该计数。headless 环境无系统剪贴板时
/// arboard 会返回错误，此处降级为返回上次计数（不触发误捕）。
///
/// # 真实运行归 pending-manual
/// arboard 需要 GUI 环境，不为其编写联网/GUI 自动化测试。
pub struct ArboardBackend {
    /// arboard Clipboard 实例，Mutex 保护跨线程访问
    clipboard: Mutex<arboard::Clipboard>,
    /// 上次读取的文本哈希，用于变化检测
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
}

impl ClipboardBackend for ArboardBackend {
    fn change_count(&self) -> u64 {
        let Ok(mut cb) = self.clipboard.lock() else {
            return *self.count.lock().unwrap_or_else(|e| e.into_inner());
        };

        let text = match cb.get_text() {
            Ok(t) => t,
            Err(_) => return *self.count.lock().unwrap_or_else(|e| e.into_inner()),
        };

        let new_hash = fnv1a_64(text.as_bytes());
        let mut last_hash = self.last_hash.lock().unwrap_or_else(|e| e.into_inner());
        let mut count = self.count.lock().unwrap_or_else(|e| e.into_inner());

        if new_hash != *last_hash {
            *last_hash = new_hash;
            *count += 1;
        }

        *count
    }

    fn read(&self) -> ClipboardSnapshot {
        let text = self
            .clipboard
            .lock()
            .ok()
            .and_then(|mut cb| cb.get_text().ok());

        ClipboardSnapshot {
            text,
            html: None,
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

/// 单次剪贴板捕获并写库。
///
/// 调用 `poll_once_with_policy` 检查是否有新内容：
/// - 有新内容 → 调 `db::ingest` 写库，返回 `Ok(Some(outcome))`
/// - 无新内容（计数未变、策略跳过等）→ 返回 `Ok(None)`
///
/// # 为什么 change_count 用 &mut
/// `poll_once_with_policy` 需要持有并更新 `last_seen` 基线，
/// 调用方（轮询线程）在循环间保持 last_seen 状态，每次调用传入同一 &mut 引用。
///
/// # Errors
/// `db::ingest` 失败时返回 DbError 描述字符串。
pub fn capture_and_ingest(
    backend: &dyn ClipboardBackend,
    last_seen: &mut u64,
    conn: &Connection,
    policy: &CapturePolicy<'_>,
) -> Result<Option<IngestOutcome>, String> {
    let item = crate::clipboard::poll_once_with_policy(backend, last_seen, policy);

    match item {
        None => Ok(None),
        Some(captured) => {
            let outcome = db::ingest(conn, &captured).map_err(|e| format!("写库失败：{e}"))?;
            Ok(Some(outcome))
        }
    }
}
