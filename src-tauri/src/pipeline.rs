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
//! 生产接线（ArboardBackend + LocalKeyProvider + 轮询线程）在 lib.rs 完成。

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::clipboard::{
    CapturedClip, ClipboardBackend, ClipboardSnapshot, RawImageData, SourceApp,
};
use crate::db::{self, IngestOutcome};
use crate::image::{CleanupPolicy, CleanupReport};
use crate::keyprovider::KeyProvider;
use crate::pasteboard_meta::PasteboardMetaReader;
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
/// 文本、HTML、图片 RGBA 字节按固定顺序参与 FNV-1a 哈希；
/// 三者皆无时返回旧 count；任一变化则 count+1。
/// 纳入 HTML 使"同纯文本但新增/变更格式"也能触发捕获。
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
    /// 平台标记读取策略。
    ///
    /// 为什么不是直接调 `read_pasteboard_meta`：macOS 的 `NSPasteboard` 跨线程并发
    /// 调用会 SIGSEGV，必须由注入的读取器负责把读取搬到主线程（见 `pasteboard_meta` 文档）。
    meta_reader: Arc<dyn PasteboardMetaReader>,
    /// 最近一次 `read` 采集到的来源应用完整身份，供 `source_app_identity` 复用。
    ///
    /// 缓存一次而不是重复读平台层：读取要跨主线程派发（macOS）或开合剪贴板（Windows），
    /// 同一次捕获没必要读两遍。
    last_source_app: Mutex<Option<SourceApp>>,
}

impl ArboardBackend {
    /// 创建 ArboardBackend 实例。
    ///
    /// `meta_reader` 决定平台标记在哪个线程读：macOS 传主线程派发实现，
    /// Windows/其它平台传就地读实现（见 `lib.rs` 的 `build_meta_reader`）。
    ///
    /// # Errors
    /// arboard::Clipboard::new() 在无 GUI 环境（headless CI）可能失败，
    /// 调用方应优雅降级（eprintln，不 panic）。
    #[must_use = "ArboardBackend::new 返回 Result，忽略它会导致初始化失败被静默丢弃"]
    pub fn new(meta_reader: Arc<dyn PasteboardMetaReader>) -> Result<Self, String> {
        let clipboard = arboard::Clipboard::new().map_err(|e| format!("剪贴板初始化失败：{e}"))?;
        Ok(Self {
            clipboard: Mutex::new(clipboard),
            last_hash: Mutex::new(0),
            count: Mutex::new(0),
            meta_reader,
            last_source_app: Mutex::new(None),
        })
    }

    /// 读取当前剪贴板的文本/HTML/图片并计算复合 FNV-1a 指纹。
    ///
    /// 实际哈希组合委托给纯函数 `composite_hash_bytes`（便于单测），
    /// 此方法只负责从 arboard 取三段内容。三者皆无时返回 `None`。
    fn compute_composite_hash(cb: &mut arboard::Clipboard) -> Option<u64> {
        let text = cb.get_text().ok();
        // get() 是一次性 builder，故 html 与 text/image 分别取；.ok() 降级 None 不 panic
        let html = cb.get().html().ok();
        let image = cb.get_image().ok();
        let image_bytes = image.as_ref().map(|img| img.bytes.as_ref());

        composite_hash_bytes(text.as_deref(), html.as_deref(), image_bytes)
    }
}

/// 由文本/HTML/图片字节计算复合 FNV-1a 指纹（纯函数，便于单测）。
///
/// 三段按固定顺序 text + 分隔符 + html + 分隔符 + image 拼接后哈希，
/// 顺序与分隔符保证确定性：相邻段间的 `0xFF` 分隔符避免
/// "ab"+"" 与 "a"+"b" 这类跨段拼接碰撞。
///
/// # 为什么把 html 也纳入
/// "同纯文本但新增/变更了 html" 需触发 change_count 递增，
/// 否则带格式的复制不会被捕获，无法走 ingest 补写 html。
///
/// # 返回
/// 三者皆为 `None` 时返回 `None`（无内容，不更新计数）。
#[must_use]
pub fn composite_hash_bytes(
    text: Option<&str>,
    html: Option<&str>,
    image: Option<&[u8]>,
) -> Option<u64> {
    if text.is_none() && html.is_none() && image.is_none() {
        return None;
    }

    let mut combined: Vec<u8> = Vec::new();
    if let Some(t) = text {
        combined.extend_from_slice(t.as_bytes());
    }
    combined.push(0xFF);
    if let Some(h) = html {
        combined.extend_from_slice(h.as_bytes());
    }
    combined.push(0xFF);
    if let Some(img) = image {
        // 直接对 RGBA 字节哈希，不转 PNG（避免编码开销）
        combined.extend_from_slice(img);
    }

    Some(fnv1a_64(&combined))
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
            self.reset_source_app_cache();
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

        // get() 是一次性 builder（与 get_text/get_image 分别调用）；无 HTML 时
        // 返回 Err，用 .ok() 降级为 None 不 panic。
        let html = cb.get().html().ok();

        // get_image 在 headless 或无图片时返回 Err（ContentNotAvailable 等），
        // 用 .ok() 降级为 None，不 panic。
        let image = cb.get_image().ok().map(|img_data| RawImageData {
            width: img_data.width,
            height: img_data.height,
            bytes: img_data.bytes.into_owned(),
        });

        // arboard 抽象不暴露平台类型集合，故标记经注入的读取器取：
        // 敏感（macOS ConcealedType / TransientType；Windows 两个剪贴板格式）、
        // 自产标记（macOS 私有 UTI；Windows 写入门闩）、来源应用身份。
        // 三者来自同一次读取，描述的是同一份内容；macOS 上该读取在主线程执行。
        let meta = self.meta_reader.read();
        let source_app = meta.source_app.clone();
        *self
            .last_source_app
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = source_app.clone();

        ClipboardSnapshot {
            text,
            html,
            image,
            has_self_marker: meta.has_self_marker,
            is_concealed: meta.is_concealed,
            source_app: source_app.and_then(|app| app.primary_identifier()),
        }
    }

    /// 返回本次变化的来源应用完整身份（bundle id + 显示名）。
    ///
    /// 取自最近一次 `read` 采集并缓存的身份，供排除名单做显示名匹配。
    /// 轮询路径保证 `read` 先于本方法调用（`poll_once_with_policy` 的顺序）。
    fn source_app_identity(&self) -> Option<SourceApp> {
        self.last_source_app
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl ArboardBackend {
    /// 清空来源应用身份缓存（读取失败时调用，避免沿用上一轮的身份误判来源）。
    fn reset_source_app_cache(&self) {
        *self
            .last_source_app
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
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
/// 写库成功且**本轮新增了图片行**时，追加一次图片总量分级清理
/// （`run_image_cleanup_best_effort`，阈值取 [`CleanupPolicy::default`] 的 500 MiB）。
///
/// # 为什么用 SAVEPOINT 而非 Transaction
/// `db::ingest_image_as_clip` 内部已有嵌套 SAVEPOINT（`ingest_image_clip`），
/// 若外层用 `conn.transaction()` 会因借用冲突无法将同一连接传入子函数。
/// SAVEPOINT 通过 `execute_batch` 字符串操作规避借用，支持嵌套，两级名称不同（`capture_ingest` vs `ingest_image_clip`）。
///
/// # 为什么返回 `Vec<IngestOutcome>`
/// 混合图文复制会在一次捕获中产生文本条目和图片条目两个结果，
/// 需要分别写库并收集各自的 IngestOutcome，Vec 是最自然的容器。
///
/// # Errors
/// `db::ingest` 或 `db::ingest_image_as_clip` 失败时，整体回滚后返回 DbError 描述字符串。
/// 清理阶段的失败**不会**变成这里的错误（best-effort，只留痕）。
pub fn capture_and_ingest(
    backend: &dyn ClipboardBackend,
    last_seen: &mut u64,
    conn: &Connection,
    policy: &CapturePolicy<'_>,
    max_image_bytes: u64,
) -> Result<Vec<IngestOutcome>, String> {
    capture_and_ingest_with_cleanup(
        backend,
        last_seen,
        conn,
        policy,
        max_image_bytes,
        &CleanupPolicy::default(),
    )
}

/// [`capture_and_ingest`] 的实现体，多接一个清理策略。
///
/// 策略单独参数化的原因：生产用默认 500 MiB，单测要能设成极小阈值来验证
/// 「超限自动清理」真的被触发，否则就得在测试里造 500 MiB 数据。
fn capture_and_ingest_with_cleanup(
    backend: &dyn ClipboardBackend,
    last_seen: &mut u64,
    conn: &Connection,
    policy: &CapturePolicy<'_>,
    max_image_bytes: u64,
    cleanup_policy: &CleanupPolicy,
) -> Result<Vec<IngestOutcome>, String> {
    let clips = crate::clipboard::poll_once_with_policy(backend, last_seen, policy);
    if clips.is_empty() {
        return Ok(Vec::new());
    }

    conn.execute_batch("SAVEPOINT capture_ingest;")
        .map_err(|e| format!("开启事务失败：{e}"))?;

    let (outcomes, new_image_rows) = match ingest_clips(conn, clips, max_image_bytes) {
        Ok(result) => {
            conn.execute_batch("RELEASE SAVEPOINT capture_ingest;")
                .map_err(|e| format!("提交事务失败：{e}"))?;
            result
        }
        Err(e) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT capture_ingest; RELEASE SAVEPOINT capture_ingest;",
            );
            return Err(e);
        }
    };

    // 清理必须在事务提交之后：它是独立的后台维护动作，失败不能回滚本轮捕获
    if new_image_rows > 0 {
        run_image_cleanup_best_effort(conn, cleanup_policy);
    }

    Ok(outcomes)
}

/// 图片总量分级清理（best-effort）。
///
/// 只在「本轮新增了图片行」之后调用：
/// - 文本轮次、图片去重命中（`Bumped`）都与图片总量无关，调用它们只是白白付一次聚合；
/// - 500ms 轮询的空轮次更不会被调用。
///
/// 最坏额外开销：一次 `SELECT SUM(length(original) + length(thumbnail))`
/// （见 [`crate::image::total_image_bytes`]，只对未软删行做聚合），以及触发时
/// 按候选行数线性进行的 strip / 软删。相比本轮刚完成的「PNG 解码 + BLOB 入库」可忽略。
///
/// 失败只 eprintln 留痕，不向上传播：清理是维护性动作，不能让一次清理失败
/// 变成「这次复制没被记录」。
fn run_image_cleanup_best_effort(conn: &Connection, policy: &CleanupPolicy) {
    match crate::image::tiered_cleanup(conn, policy) {
        Ok(report) if report == CleanupReport::default() => {
            // 未超阈值：静默（这是绝大多数情况，不应产生日志噪声）
        }
        Ok(report) => eprintln!(
            "[QuickQuick] 图片总量超过 {} MiB，已自动清理：腾空原图 {} 条、整条软删 {} 条",
            policy.max_total_bytes / (1024 * 1024),
            report.stripped_originals,
            report.deleted_rows
        ),
        Err(e) => eprintln!("[QuickQuick] 图片分级清理失败（非致命，不影响本轮捕获）：{e}"),
    }
}

/// 一次入库结果是否让「图片总量」增长。
///
/// 这也是图片分级清理的唯一触发判据：
/// - 文本条目入库与图片总量无关；
/// - 图片命中同 hash 走 [`IngestOutcome::Bumped`]（仅置顶，BLOB 数量不变）；
/// - 只有图片 [`IngestOutcome::Inserted`] 才会新增一行、让总量只增不减。
fn counts_as_new_image_row(is_image_clip: bool, outcome: &IngestOutcome) -> bool {
    is_image_clip && matches!(outcome, IngestOutcome::Inserted(_))
}

/// 逐条写库（拆出以保持主函数 ≤50 行、事务边界清晰）。
///
/// 在调用方的 SAVEPOINT 保护下运行，任一条失败则由调用方回滚整批。
/// `max_image_bytes`：图片原图大小上限，由调用方从 AppSettings 读取后传入。
///
/// 返回值第二项是本轮**新增**的图片行数（供调用方决定是否跑图片总量清理）。
fn ingest_clips(
    conn: &Connection,
    clips: Vec<CapturedClip>,
    max_image_bytes: u64,
) -> Result<(Vec<IngestOutcome>, usize), String> {
    let mut outcomes = Vec::with_capacity(clips.len());
    let mut new_image_rows = 0usize;
    for clip in clips {
        let (outcome, added_image_row) = match clip {
            CapturedClip::Text(item) => {
                let outcome =
                    db::ingest(conn, &item).map_err(|e| format!("写库失败（文本）：{e}"))?;
                let added = counts_as_new_image_row(false, &outcome);
                (outcome, added)
            }
            CapturedClip::Image {
                width,
                height,
                png_bytes,
            } => {
                let outcome =
                    db::ingest_image_as_clip(conn, width, height, &png_bytes, max_image_bytes)
                        .map_err(|e| format!("写库失败（图片）：{e}"))?;
                let added = counts_as_new_image_row(true, &outcome);
                (outcome, added)
            }
        };
        if added_image_row {
            new_image_rows += 1;
        }
        outcomes.push(outcome);
    }
    Ok((outcomes, new_image_rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy::ExcludeList;

    /// 单轮剪贴板后端：计数恒为 `count`，快照固定。
    struct OneShotBackend {
        count: u64,
        snapshot: ClipboardSnapshot,
    }

    impl ClipboardBackend for OneShotBackend {
        fn change_count(&self) -> u64 {
            self.count
        }

        fn read(&self) -> ClipboardSnapshot {
            self.snapshot.clone()
        }
    }

    /// 建一个带完整 schema 的临时加密库（复用生产建表语句）。
    fn open_test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn =
            crate::db::open_or_create(&dir.path().join("qq.db"), &[9u8; 32]).expect("建库应成功");
        (dir, conn)
    }

    /// 直接插一条"最旧"的图片行（created_utc 早于运行时新插入的行）。
    fn seed_old_image_row(conn: &Connection, original_size: usize, created_utc: i64) {
        conn.execute(
            "INSERT INTO clip_images
                (id, clip_item_id, thumbnail, original, original_present, image_hash,
                 created_utc, last_modified_utc, is_deleted, is_favorite)
             VALUES ('seeded-old', NULL, ?1, ?2, 1, 'seeded-hash', ?3, ?3, 0, 0)",
            rusqlite::params![vec![0xABu8; 10], vec![0xCDu8; original_size], created_utc],
        )
        .expect("插入种子图片行应成功");
    }

    fn image_snapshot(width: usize, height: usize, rgba: Vec<u8>) -> ClipboardSnapshot {
        ClipboardSnapshot {
            text: None,
            html: None,
            image: Some(RawImageData {
                width,
                height,
                bytes: rgba,
            }),
            has_self_marker: false,
            is_concealed: false,
            source_app: None,
        }
    }

    fn text_snapshot(text: &str) -> ClipboardSnapshot {
        ClipboardSnapshot {
            text: Some(text.to_owned()),
            html: None,
            image: None,
            has_self_marker: false,
            is_concealed: false,
            source_app: None,
        }
    }

    #[test]
    fn counts_as_new_image_row_only_for_inserted_image() {
        // Arrange：四种组合（文本/图片 × 新增行/去重命中）
        let inserted = IngestOutcome::Inserted("id-1".to_owned());
        let bumped = IngestOutcome::Bumped("id-2".to_owned());

        // Act & Assert：只有"图片 + 新增行"才让图片总量增长
        assert!(
            counts_as_new_image_row(true, &inserted),
            "图片新增行应触发图片总量清理"
        );
        assert!(
            !counts_as_new_image_row(true, &bumped),
            "图片去重命中（仅置顶）不改变总量，不应清理"
        );
        assert!(
            !counts_as_new_image_row(false, &inserted),
            "文本条目与图片总量无关"
        );
        assert!(!counts_as_new_image_row(false, &bumped), "文本置顶同理");
    }

    #[test]
    fn new_image_capture_triggers_tiered_cleanup_over_limit() {
        // Arrange：库里已有一条很旧的图片行（原图 200 字节）
        let (_dir, conn) = open_test_db();
        seed_old_image_row(&conn, 200, 1_000);
        let backend = OneShotBackend {
            count: 1,
            snapshot: image_snapshot(2, 2, vec![0u8; 2 * 2 * 4]),
        };
        let exclude = ExcludeList::default();
        let policy = CapturePolicy {
            paused: false,
            skip_sensitive: true,
            exclude: &exclude,
        };
        // 阈值设为种子行大小：新图入库后必然超限 → 应腾空最旧（种子）行原图
        let cleanup = CleanupPolicy {
            max_total_bytes: 210,
        };
        let mut last_seen = 0u64;

        // Act
        let outcomes = capture_and_ingest_with_cleanup(
            &backend,
            &mut last_seen,
            &conn,
            &policy,
            usize::MAX as u64,
            &cleanup,
        )
        .expect("捕获应成功");

        // Assert 1：本轮确实新增了图片条目（清理的前提成立）
        assert_eq!(outcomes.len(), 1, "应产生一条图片入库结果");
        assert!(
            matches!(outcomes[0], IngestOutcome::Inserted(_)),
            "首次捕获应为新增行"
        );
        // Assert 2：最旧行的原图被腾空（第一级清理真的跑了）
        assert_eq!(
            crate::image::get_original_present(&conn, "seeded-old").expect("ok"),
            Some(0),
            "超限时应腾空最旧非收藏行的原图"
        );
        // Assert 3：清理后总量回到阈值内
        let total = crate::image::total_image_bytes(&conn).expect("total");
        assert!(
            total <= cleanup.max_total_bytes,
            "清理后总量应不超过阈值，实际 {total}"
        );
        // Assert 4：本轮新插入的图片行未被清理掉
        assert_eq!(
            crate::image::image_count(&conn).expect("image_count"),
            2,
            "新插入的图片行应仍在（清理只腾空最旧行的原图）"
        );
    }

    #[test]
    fn text_only_capture_does_not_trigger_cleanup() {
        // Arrange：库里已经超限（种子原图 200 字节，阈值只给 100）
        let (_dir, conn) = open_test_db();
        seed_old_image_row(&conn, 200, 1_000);
        let backend = OneShotBackend {
            count: 1,
            snapshot: text_snapshot("纯文本复制"),
        };
        let exclude = ExcludeList::default();
        let policy = CapturePolicy {
            paused: false,
            skip_sensitive: true,
            exclude: &exclude,
        };
        let cleanup = CleanupPolicy {
            max_total_bytes: 100,
        };
        let mut last_seen = 0u64;

        // Act
        let outcomes = capture_and_ingest_with_cleanup(
            &backend,
            &mut last_seen,
            &conn,
            &policy,
            usize::MAX as u64,
            &cleanup,
        )
        .expect("捕获应成功");

        // Assert：文本轮次不触发清理——种子行原图保持不动（即使它已超限）
        assert_eq!(outcomes.len(), 1, "应产生一条文本入库结果");
        assert_eq!(
            crate::image::get_original_present(&conn, "seeded-old").expect("ok"),
            Some(1),
            "文本轮次不应触发图片清理（避免每轮付一次聚合开销）"
        );
    }

    /// 只在"腾空原图"这一种更新上装触发器，让清理阶段的 strip 必然失败。
    ///
    /// 条件 `OLD.original_present = 1 AND NEW.original_present = 0` 精确对应
    /// `strip_original` 的写入特征，不会误伤入库路径（入库会把 `clip_item_id` 从 NULL
    /// 领养过来，那是另一类 UPDATE）。
    fn block_clip_images_updates(conn: &Connection) {
        conn.execute_batch(
            "CREATE TRIGGER block_strip BEFORE UPDATE ON clip_images
             WHEN OLD.original_present = 1 AND NEW.original_present = 0
             BEGIN SELECT RAISE(ABORT, 'cleanup blocked by test trigger'); END;",
        )
        .expect("创建触发器应成功");
    }

    #[test]
    fn cleanup_failure_does_not_block_capture() {
        // Arrange：清理阶段必然失败（UPDATE 被触发器阻断），且本轮确实新增了图片行
        let (_dir, conn) = open_test_db();
        seed_old_image_row(&conn, 200, 1_000);
        block_clip_images_updates(&conn);
        let backend = OneShotBackend {
            count: 1,
            snapshot: image_snapshot(2, 2, vec![0u8; 2 * 2 * 4]),
        };
        let exclude = ExcludeList::default();
        let policy = CapturePolicy {
            paused: false,
            skip_sensitive: true,
            exclude: &exclude,
        };
        let cleanup = CleanupPolicy {
            max_total_bytes: 210,
        };
        let mut last_seen = 0u64;

        // Act
        let result = capture_and_ingest_with_cleanup(
            &backend,
            &mut last_seen,
            &conn,
            &policy,
            usize::MAX as u64,
            &cleanup,
        );

        // Assert 1：清理失败没有把捕获一起回滚——本轮图片条目仍然入库成功
        let outcomes = result.expect("清理失败不应让捕获失败");
        assert_eq!(outcomes.len(), 1, "图片条目仍应成功入库");
        assert!(
            matches!(outcomes[0], IngestOutcome::Inserted(_)),
            "新图片应为新增行"
        );
        // Assert 2：清理确实失败了（最旧行原图还在），证明失败路径被走到过
        assert_eq!(
            crate::image::get_original_present(&conn, "seeded-old").expect("ok"),
            Some(1),
            "清理被阻断时原图不应被改动"
        );
    }

    #[test]
    fn best_effort_cleanup_swallows_db_error_and_leaves_data_intact() {
        // Arrange：超限 + UPDATE 被阻断 → tiered_cleanup 必然返回 Err
        let (_dir, conn) = open_test_db();
        seed_old_image_row(&conn, 200, 1_000);
        block_clip_images_updates(&conn);
        let cleanup = CleanupPolicy {
            max_total_bytes: 210,
        };

        // Act：函数签名为 ()，不应 panic、不应向上抛错
        run_image_cleanup_best_effort(&conn, &cleanup);

        // Assert：失败被吞，且没有留下半清理状态（原图仍在、行仍存活）
        assert_eq!(
            crate::image::get_original_present(&conn, "seeded-old").expect("ok"),
            Some(1),
            "清理失败后不应改动原图"
        );
        assert_eq!(
            crate::image::image_count(&conn).expect("image_count"),
            1,
            "清理失败后不应软删任何行"
        );
    }
}
