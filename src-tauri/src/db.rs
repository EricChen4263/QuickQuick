//! 加密数据库模块：SQLCipher 开库、schema 预埋、软删、GC、失败恢复、去重入库
//!
//! 设计对齐：设计文档§六（失败/恢复语义）、§十（schema 预埋铁律）、§三（去重+置顶刷新）
//!           §九.2（★ 置顶收藏）、§五（收藏永远豁免清理）
//!
//! schema 版本与迁移铁律（改表结构前必读）：
//! - schema 版本由 `PRAGMA user_version` 承担，目标值见 [`CURRENT_SCHEMA_VERSION`]，
//!   有序步骤见 `MIGRATIONS`。
//! - 开库路径（[`ensure_schema`]）只跑「落后于当前版本」的步骤；库版本超前时返回
//!   [`DbError::SchemaTooNew`]，绝不按老结构继续读写。
//! - 全部步骤在**单个事务**内执行，任一步失败整笔回滚，库停在迁移前版本。
//! - **新增列 / 索引 / 表必须新加一个迁移步骤并 bump `CURRENT_SCHEMA_VERSION`**，
//!   步骤内写无条件 DDL；`create_tables` / `ensure_indexes` 已冻结为版本 1 基线，
//!   不得再往里加结构，否则会出现「同一 user_version 对应两种表形态」的库。
//! - **禁止再用「探测列是否存在再 ALTER」的 ad-hoc 迁移**：版本号已经确定了库的形态。
//!   探测只允许活在版本 1 的历史对齐里（[`reconcile_html_column`]），不要复制该模式。
//! - **数据回填不写进迁移步骤**：迁移跑在开库同步路径上，全表 `UPDATE` 会把写锁拖到秒级。
//!   版本 2 的 `title` 列就是范例——迁移只无条件 `ADD COLUMN`（既有行为 NULL），
//!   回填走开库后的分批幂等 [`backfill_titles_batch`]。
//!
//! 核心函数：
//! - `open_or_create`       — 打开或新建加密库，并确保 schema 已初始化
//! - `open_or_recover`      — 带损坏检测的打开：损坏时改名备份，按标志决定是否重建
//! - `soft_delete`          — 软删（置墓碑，非物理删）
//! - `gc_purge_deleted`     — 本地物理清理 GC（删除 is_deleted=1 的行）
//! - `ingest`               — 去重入库：同 text_hash 存在则 bump，否则插入新行
//! - `bump_to_top`          — 显式置顶：仅更新 last_modified_utc（不新建记录）
//! - `count_live`           — 查询未软删行数（测试/业务用）
//! - `top_id`               — 返回 last_modified_utc 最新的未软删行 id（测试/业务用）
//! - `set_favorite`         — 设置/取消收藏（is_favorite 字段）
//! - `list_ordered`         — 返回排序后的条目列表（收藏优先，组内按最近）
//! - `cleanup_keep_recent`  — 容量裁剪：只删非收藏的旧项，收藏永远豁免
//! - `evict_over_limit`     — 自动淘汰入口：非收藏条目超过可配上限时裁剪（带计数短路）
//! - `compute_title`        — 预计算 title：正文空白规范化后截断到 1000 字符（图片条目留空）
//! - `backfill_titles_batch` — title 分批幂等回填：只补 `title IS NULL` 的行，可中断可续跑
//!
//! 安全约定：
//! - 密钥以 raw key 格式传入（`PRAGMA key = "x'<hex>'"`)，不写入日志
//! - 打开后立即执行 `PRAGMA user_version` 轻量查询触发解密校验（它同时承担 schema 版本，
//!   见上方迁移铁律：开库时先读它，再决定要不要跑迁移）
//! - 永不静默删除数据库文件（设计§六硬约束）

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::clipboard::CapturedItem;
use crate::image as img_mod;

/// 数据库操作错误
#[derive(Debug, Error)]
pub enum DbError {
    /// SQLite / SQLCipher 操作失败（含解密失败）
    #[error("数据库操作失败：{0}")]
    Sqlite(#[from] rusqlite::Error),

    /// 文件 I/O 失败（改名备份、读写等）
    #[error("文件 I/O 失败：{0}")]
    Io(#[from] std::io::Error),

    /// 数据库文件损坏（解密失败或格式非法）
    #[error("数据库文件损坏，旧库已备份为 {backup_path}")]
    Corrupt {
        /// 备份文件路径（改名后的路径）
        backup_path: String,
    },

    /// 其他业务层错误（如缩略图编码失败等不属于 SQL/IO 的失败）
    #[error("操作失败：{0}")]
    Other(String),

    /// 瞬时钥匙串错误（钥匙串被拒/系统锁定；不代表数据损坏，仅需重试）
    ///
    /// 与 KeyError::Backend 对应的数据库层表示：上层 KeyProvider 返回 Backend 错误后，
    /// 调用方可通过此变体向 db 层传递"瞬时失败"语义，触发 Transient 分级。
    #[error("瞬时钥匙串错误：{0}")]
    TransientKeychain(String),

    /// 数据库 schema 版本高于本程序支持的版本（旧程序打开了新库）
    ///
    /// 独立成变体而不复用 [`DbError::Sqlite`]，是为了让 [`open_or_recover`] 不去备份它：
    /// 那里把 `Sqlite` 当「库已损坏」，改名前会带走一个完全可用（只是比本程序新）的库。
    /// 版本超前不是损坏，库文件一个字节都不该动。
    ///
    /// `Display` 文案就是启动期用户与日志能看到的唯一解释，必须点明真实原因（库比程序新、
    /// 要更新应用），**不得**退化成解密失败那类话术（「请检查钥匙串授权」），也不得出现
    /// `not a database` 之类会被一次性重置误判的标记。
    #[error("数据库由更新版本的 QuickQuick 创建（v{found} > v{supported}），请更新应用后再试")]
    SchemaTooNew {
        /// 库里 `PRAGMA user_version` 的实际值
        found: i32,
        /// 本程序支持的最高版本（[`CURRENT_SCHEMA_VERSION`]）
        supported: i32,
    },

    /// schema 迁移失败（事务已回滚，库停在迁移前的版本）
    ///
    /// 同样不复用 [`DbError::Sqlite`]：迁移失败是「本程序的迁移代码/环境出了问题」，
    /// 不是库损坏，不能触发备份重建。
    ///
    /// `Display` 文案即用户可见原因，三段信息都不能少：哪个版本升到哪个版本失败、
    /// 底层原因（透传）、以及「原库未被修改」——最后这句与事务回滚的事实对齐，
    /// 用户不必担心一次升级失败把历史改坏。
    ///
    /// 文本约束：**本变体自身的文案**不得包含 `not a database` 一类可重置标记，否则启动期
    /// 的一次性重置会把库备份走并重建空库（丢用户历史）。`reason` 是底层错误的透传文本，
    /// 若它真的报 `file is not a database`，说明库确实不是可用数据库，走重置才是对的。
    #[error("数据库升级失败（{from_version}→{to_version}）：{reason}。原库未被修改，请联系支持")]
    MigrationFailed {
        /// 失败的迁移步骤目标版本
        to_version: i32,
        /// 迁移前（回滚后）的版本
        from_version: i32,
        /// 底层失败原因（保留原始错误文本，便于定位是哪条 DDL 挂了）
        reason: String,
    },
}

impl DbError {
    /// 构造瞬时钥匙串错误（测试与业务层用）。
    ///
    /// 对应设计§六#1 中"钥匙串被拒/锁"路径——仅提示重试，绝不碰库文件。
    pub fn transient_keychain_error(msg: impl Into<String>) -> Self {
        Self::TransientKeychain(msg.into())
    }
}

/// 错误失败分级：区分可重试的瞬时失败与需要恢复的永久失败。
///
/// 设计§六#1 失败/恢复语义：
/// - `Transient`：钥匙串被拒/系统锁定等，库文件完好，仅需重试
/// - `Permanent`：密钥丢失/库损坏/解密失败，需备份旧库并（显式确认后）重建
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureTier {
    /// 瞬时失败：库文件完好，仅需提示用户重试，绝不改动库文件
    Transient,
    /// 永久失败：密钥不可用或库已损坏，须备份旧库，显式确认后才可重建
    Permanent,
}

/// 失败后的恢复动作。
///
/// 由 `recovery_action(tier)` 根据 `FailureTier` 返回，供调用方决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// 仅提示用户重试，绝不改名/删除/重建库文件（对应 Transient）
    RetryNoTouch,
    /// 将旧库改名备份，等待显式用户确认后才重建空库（对应 Permanent）
    BackupAndConfirmRebuild,
}

/// 将 `DbError` 分类为瞬时或永久失败。
///
/// 分类规则（设计§六#1）：
/// - `TransientKeychain`       → `Transient`（钥匙串被拒/锁，库文件完好）
/// - `Corrupt` / `Sqlite`      → `Permanent`（库损坏或解密失败，需备份恢复）
/// - `Io` / `Other`            → `Permanent`（保守归类，IO 失败视为需人工介入）
/// - `SchemaTooNew` / `MigrationFailed` → `Permanent`（需换程序版本或修迁移，绝不重建库）
pub fn classify_failure(err: &DbError) -> FailureTier {
    match err {
        DbError::TransientKeychain(_) => FailureTier::Transient,
        DbError::Corrupt { .. }
        | DbError::Sqlite(_)
        | DbError::Io(_)
        | DbError::Other(_)
        | DbError::SchemaTooNew { .. }
        | DbError::MigrationFailed { .. } => FailureTier::Permanent,
    }
}

/// 根据失败分级返回对应的恢复动作。
///
/// - `Transient` → `RetryNoTouch`：仅提示重试，不触碰库文件
/// - `Permanent` → `BackupAndConfirmRebuild`：改名备份，显式确认才重建
pub fn recovery_action(tier: FailureTier) -> RecoveryAction {
    match tier {
        FailureTier::Transient => RecoveryAction::RetryNoTouch,
        FailureTier::Permanent => RecoveryAction::BackupAndConfirmRebuild,
    }
}

/// 执行恢复动作：将 `RecoveryAction` 的语义落地到文件系统操作。
///
/// - `RetryNoTouch`        → 不碰文件，直接返回 `Ok(())`（库文件原样保留）
/// - `BackupAndConfirmRebuild` → 调用 `backup_corrupt_file` 将旧库改名备份
///
/// # Errors
/// - `DbError::Io`：`BackupAndConfirmRebuild` 路径下 rename 失败
/// - `DbError::Io`：`path` 无文件名分量（`backup_corrupt_file` 报错）
pub fn apply_recovery_action(path: &Path, action: RecoveryAction) -> Result<(), DbError> {
    match action {
        RecoveryAction::RetryNoTouch => Ok(()),
        RecoveryAction::BackupAndConfirmRebuild => {
            backup_corrupt_file(path)?;
            Ok(())
        }
    }
}

/// 打开或新建加密数据库，确保 schema 已初始化。
///
/// # 密钥格式
/// 使用 SQLCipher raw key：`PRAGMA key = "x'<64-hex-chars>'"` 形式。
/// raw key 直接作为 AES-256 密钥材料，无需 PBKDF2 派生，与 KeyProvider 的
/// 32 字节密钥直接对应。
///
/// # 验证时机
/// 打开后立即执行 `PRAGMA user_version` 触发解密校验；错误密钥在此处暴露。
///
/// # Errors
/// - `DbError::Sqlite`：解密失败或 SQL 执行失败
/// - `DbError::Io`：路径目录无法创建
pub fn open_or_create(path: &Path, key: &[u8; 32]) -> Result<Connection, DbError> {
    let conn = open_with_key(path, key)?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// 带损坏检测的打开：损坏时改名备份，按 `allow_rebuild` 决定是否重建空库。
///
/// # 行为
/// 1. 尝试 `open_or_create`
/// 2. 成功 → 直接返回
/// 3. 判定为永久损坏（解密/格式错误）→ 改名备份（绝不删除原内容）
///    - `allow_rebuild=false` → 返回 `DbError::Corrupt`（不建空库）
///    - `allow_rebuild=true`  → 备份后重建新空库并返回 `Ok`
///
/// # 设计§六硬约束
/// 永不静默删除或覆盖原文件；备份命名格式：`<原名>.corrupt-<utc_secs>`。
///
/// # 非损坏类错误原样上抛
/// 只把 [`DbError::Sqlite`]（解密/格式失败）当损坏。schema 版本超前
/// （[`DbError::SchemaTooNew`]）与迁移失败（[`DbError::MigrationFailed`]）都不是损坏：
/// 库本身完好，备份改名只会把可用数据搬走，故一律原样返回、不动文件。
///
/// # Errors
/// - `DbError::Corrupt`：损坏且 `allow_rebuild=false`
/// - `DbError::Io`：改名或重建时 I/O 失败
/// - `DbError::Sqlite`：重建后 schema 初始化失败
pub fn open_or_recover(
    path: &Path,
    key: &[u8; 32],
    allow_rebuild: bool,
) -> Result<Connection, DbError> {
    match open_or_create(path, key) {
        Ok(conn) => return Ok(conn),
        Err(DbError::Sqlite(_)) => {
            // 解密/格式失败 → 视为永久损坏，走备份流程
        }
        Err(other) => return Err(other),
    }

    let backup_path = backup_corrupt_file(path)?;

    if !allow_rebuild {
        return Err(DbError::Corrupt {
            backup_path: backup_path.to_string_lossy().into_owned(),
        });
    }

    // allow_rebuild=true：备份已完成，重建新空库
    let conn = open_or_create(path, key)?;
    Ok(conn)
}

/// 软删一条记录（置墓碑，不物理删除）。
///
/// 更新字段：`is_deleted=1`、`deleted_at_utc=now`、`last_modified_utc=now`。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败（含 id 不存在时影响行数为 0，不报错）
pub fn soft_delete(conn: &Connection, id: &str) -> Result<(), DbError> {
    let now_ms = current_utc_ms();
    conn.execute(
        "UPDATE clip_items
         SET is_deleted = 1,
             deleted_at_utc = ?1,
             last_modified_utc = ?2
         WHERE id = ?3",
        rusqlite::params![now_ms, now_ms, id],
    )?;
    Ok(())
}

/// 完整剪贴板条目行（list_items_full 返回元素）。
///
/// 包含前端展示所需的全量字段：内容、类型、收藏状态、时间。
#[derive(Debug, PartialEq)]
pub struct ClipItemRow {
    /// 条目 UUID
    pub id: String,
    /// 条目内容（文本）
    pub content: String,
    /// 内容类型（如 "text"）
    pub kind: String,
    /// 收藏标记（true = 收藏）
    pub is_favorite: bool,
    /// 最后修改时间（UTC epoch ms）
    pub last_modified_utc: i64,
    /// 富文本 HTML（None=纯文本条目）；供预览富文本渲染与粘贴/复制还原格式
    pub html_content: Option<String>,
}

/// 完整剪贴板条目行，含关联图片字段（list_items_with_images 返回元素）。
///
/// 文本条目因 LEFT JOIN 无匹配，`image_id` 与 `thumbnail` 均为 `None`。
#[derive(Debug)]
pub struct ClipItemRowWithImage {
    /// 条目 UUID
    pub id: String,
    /// 条目内容（文本）
    pub content: String,
    /// 内容类型（"text" / "image"）
    pub kind: String,
    /// 收藏标记
    pub is_favorite: bool,
    /// 最后修改时间（UTC epoch ms）
    pub last_modified_utc: i64,
    /// 首次入库时间（UTC epoch ms）；「首次复制」排序用它，再次复制不会改动
    pub created_utc: i64,
    /// 关联图片行 id（None 表示无图）
    pub image_id: Option<String>,
    /// 缩略图 BLOB（WebP 字节；None 表示无图）
    pub thumbnail: Option<Vec<u8>>,
    /// 富文本 HTML（None=纯文本/图片条目）；供预览富文本渲染与粘贴/复制还原格式
    pub html_content: Option<String>,
}

/// 列表主查询骨架：FROM/JOIN/WHERE/ORDER BY 统一由本宏给出，图片列由调用方指定。
///
/// 两个变体（缩略图 BLOB / 缩略图标志）必须共享同一份「决定索引计划」的骨架：
/// 计划测试断言的是常量里的生产语句本身，若两处各抄一份 SQL，
/// 改了一处忘了另一处时查询计划会在无告警的情况下退化。
macro_rules! list_items_sql {
    ($image_projection:literal) => {
        concat!(
            "SELECT ci.id, ci.content, ci.kind, ci.is_favorite, ci.last_modified_utc,
                ci.created_utc,
                cimg.id AS image_id, ",
            $image_projection,
            ", ci.html_content
         FROM clip_items ci
         LEFT JOIN clip_images cimg ON cimg.clip_item_id = ci.id AND cimg.is_deleted = 0
         WHERE ci.is_deleted = 0
         ORDER BY ci.is_favorite DESC, ci.last_modified_utc DESC, ci.rowid DESC"
        )
    };
}

/// 列表主查询（含关联图片缩略图 BLOB）的 SQL。
///
/// 独立成常量是为了让「查询计划不得退化」的测试断言的就是生产语句本身，
/// 而不是测试里另抄一份、日后与实现悄悄漂移的副本。
const LIST_ITEMS_WITH_IMAGES_SQL: &str = list_items_sql!("cimg.thumbnail");

/// 列表主查询（图片列只取「是否有缩略图」标志）的 SQL。
///
/// IPC 列表用这个变体：缩略图 BLOB 不出库，Rust 侧也就不必为每行分配一块图片内存。
const LIST_ITEMS_WITH_THUMBNAIL_FLAG_SQL: &str = list_items_sql!(
    "CASE WHEN cimg.thumbnail IS NOT NULL AND length(cimg.thumbnail) > 0 THEN 1 ELSE 0 END"
);

/// 返回未软删条目列表，LEFT JOIN 图片缩略图：收藏优先，组内按最近修改时间降序。
///
/// 与 `list_items_full` 排序规则相同，但额外带出关联 `clip_images` 的
/// `id`（`image_id`）和 `thumbnail` BLOB，文本条目这两个字段为 `None`。
///
/// 排序兜底：`rowid DESC` 确保同毫秒并列时稳定有序。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn list_items_with_images(conn: &Connection) -> Result<Vec<ClipItemRowWithImage>, DbError> {
    let mut stmt = conn.prepare(LIST_ITEMS_WITH_IMAGES_SQL)?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let content: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
        let kind: String = row.get(2)?;
        let is_fav_int: i64 = row.get(3)?;
        let last_modified: i64 = row.get(4)?;
        let created: i64 = row.get(5)?;
        let image_id: Option<String> = row.get(6)?;
        let thumbnail: Option<Vec<u8>> = row.get(7)?;
        let html_content: Option<String> = row.get(8)?;
        Ok(ClipItemRowWithImage {
            id,
            content,
            kind,
            is_favorite: is_fav_int != 0,
            last_modified_utc: last_modified,
            created_utc: created,
            image_id,
            thumbnail,
            html_content,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 列表行，图片列只带「是否有缩略图」标志（`list_items_with_image_flag` 返回元素）。
///
/// 与 `ClipItemRowWithImage` 的区别只有一处：不把缩略图 BLOB 取进内存。
/// IPC 列表只需要知道「该不该为此行拉缩略图」，字节留给按需接口去取。
#[derive(Debug)]
pub struct ClipItemRowWithImageFlag {
    /// 条目 UUID
    pub id: String,
    /// 条目内容（文本）
    pub content: String,
    /// 内容类型（"text" / "image"）
    pub kind: String,
    /// 收藏标记
    pub is_favorite: bool,
    /// 最后修改时间（UTC epoch ms）
    pub last_modified_utc: i64,
    /// 首次入库时间（UTC epoch ms）；「首次复制」排序用它，再次复制不会改动
    pub created_utc: i64,
    /// 关联图片行 id（None 表示无图）
    pub image_id: Option<String>,
    /// 关联图片行是否有非空缩略图 BLOB（无图条目为 false）
    pub has_thumbnail: bool,
    /// 富文本 HTML（None=纯文本/图片条目）；供预览富文本渲染与粘贴/复制还原格式
    pub html_content: Option<String>,
}

/// 返回未软删条目列表，图片列只取「是否有缩略图」标志：收藏优先，组内按最近修改时间降序。
///
/// 与 `list_items_with_images` 排序与关联口径完全一致，唯一差别是缩略图 BLOB 不出库，
/// 供 IPC 列表使用——列表每次刷新都会走这条路，把 BLOB 搬进内存没有收益。
/// 「空 BLOB 也算没有缩略图」：降级写入路径会留下零字节缩略图，按标志取图时必然返回空。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn list_items_with_image_flag(
    conn: &Connection,
) -> Result<Vec<ClipItemRowWithImageFlag>, DbError> {
    let mut stmt = conn.prepare(LIST_ITEMS_WITH_THUMBNAIL_FLAG_SQL)?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let content: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
        let kind: String = row.get(2)?;
        let is_fav_int: i64 = row.get(3)?;
        let last_modified: i64 = row.get(4)?;
        let created: i64 = row.get(5)?;
        let image_id: Option<String> = row.get(6)?;
        let has_thumbnail_int: i64 = row.get(7)?;
        let html_content: Option<String> = row.get(8)?;
        Ok(ClipItemRowWithImageFlag {
            id,
            content,
            kind,
            is_favorite: is_fav_int != 0,
            last_modified_utc: last_modified,
            created_utc: created,
            image_id,
            has_thumbnail: has_thumbnail_int != 0,
            html_content,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 返回未软删条目的完整列表：收藏优先，组内按最近修改时间降序。
///
/// 与 `list_ordered` 排序规则相同，但返回前端展示所需的全量字段。
///
/// 排序兜底：`rowid DESC` 确保同毫秒并列时最后插入的条目稳定排前，
/// 消除并发测试下因时间戳精度导致的不确定顺序（flaky 根因：同毫秒两次 ingest）。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn list_items_full(conn: &Connection) -> Result<Vec<ClipItemRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, content, kind, is_favorite, last_modified_utc, html_content
         FROM clip_items
         WHERE is_deleted = 0
         ORDER BY is_favorite DESC, last_modified_utc DESC, rowid DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let content: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
        let kind: String = row.get(2)?;
        let is_fav_int: i64 = row.get(3)?;
        let last_modified: i64 = row.get(4)?;
        let html_content: Option<String> = row.get(5)?;
        Ok(ClipItemRow {
            id,
            content,
            kind,
            is_favorite: is_fav_int != 0,
            last_modified_utc: last_modified,
            html_content,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 列表排序后的条目行（list_ordered 返回元素）。
///
/// 含业务排序所需的最小字段集。
#[derive(Debug, PartialEq)]
pub struct ClipRow {
    /// 条目 UUID
    pub id: String,
    /// 收藏标记（true = 收藏）
    pub is_favorite: bool,
    /// 最后修改时间（UTC epoch ms）
    pub last_modified_utc: i64,
}

/// 设置或取消收藏。
///
/// 同时刷新 `last_modified_utc`，使收藏操作反映到时间线。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败（id 不存在时影响行数为 0，不报错）
pub fn set_favorite(conn: &Connection, id: &str, fav: bool) -> Result<(), DbError> {
    let now_ms = current_utc_ms();
    let fav_int: i64 = if fav { 1 } else { 0 };
    conn.execute(
        "UPDATE clip_items
         SET is_favorite = ?1,
             last_modified_utc = ?2
         WHERE id = ?3 AND is_deleted = 0",
        rusqlite::params![fav_int, now_ms, id],
    )?;
    Ok(())
}

/// 返回未软删条目的排序列表：收藏优先，组内按最近修改时间降序。
///
/// 排序规则：`ORDER BY is_favorite DESC, last_modified_utc DESC, rowid DESC`
/// - 收藏项（is_favorite=1）整体排在非收藏项（is_favorite=0）之前
/// - 同组内按 last_modified_utc 从新到旧排列
/// - `rowid DESC` 兜底：同毫秒并列时最后插入的条目稳定排前，保证确定性
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn list_ordered(conn: &Connection) -> Result<Vec<ClipRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, is_favorite, last_modified_utc
         FROM clip_items
         WHERE is_deleted = 0
         ORDER BY is_favorite DESC, last_modified_utc DESC, rowid DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let is_fav_int: i64 = row.get(1)?;
        let last_modified: i64 = row.get(2)?;
        Ok(ClipRow {
            id,
            is_favorite: is_fav_int != 0,
            last_modified_utc: last_modified,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 容量裁剪候选查询的 SQL（独立成常量，供查询计划回归测试断言生产语句本身）。
const CLEANUP_KEEP_RECENT_CANDIDATES_SQL: &str = "SELECT id FROM clip_items
         WHERE is_deleted = 0 AND is_favorite = 0
         ORDER BY last_modified_utc DESC";

/// 容量裁剪：只删非收藏的旧项，收藏项永远豁免。
///
/// 语义：非收藏的未软删条目中，按 last_modified_utc 降序保留最新 `keep_count` 条，
/// 超出部分软删（置墓碑）。收藏项（is_favorite=1）**完全不计入也不删除**。
///
/// 选择软删而非物理删：与 `soft_delete` 语义一致（设计§六），GC 负责后续物理清理。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn cleanup_keep_recent(conn: &Connection, keep_count: usize) -> Result<usize, DbError> {
    let now_ms = current_utc_ms();
    // 查询需要被软删的非收藏旧项 id：跳过最新的 keep_count 条，删除余下的
    let mut stmt = conn.prepare(CLEANUP_KEEP_RECENT_CANDIDATES_SQL)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut all_ids: Vec<String> = Vec::new();
    for r in rows {
        all_ids.push(r?);
    }

    // 超出 keep_count 的部分才需要清理
    let to_delete = if all_ids.len() > keep_count {
        &all_ids[keep_count..]
    } else {
        return Ok(0);
    };

    let mut deleted = 0usize;
    for id in to_delete {
        conn.execute(
            "UPDATE clip_items
             SET is_deleted = 1,
                 deleted_at_utc = ?1,
                 last_modified_utc = ?1
             WHERE id = ?2",
            rusqlite::params![now_ms, id],
        )?;
        deleted += 1;
    }
    Ok(deleted)
}

/// 自动淘汰的超限判据 SQL（独立成常量，供查询计划回归测试断言生产语句本身）。
const EVICT_LIVE_NON_FAVORITE_COUNT_SQL: &str =
    "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 0 AND is_favorite = 0";

/// 把非收藏条目收敛到 `max_items` 条（入库路径的自动淘汰入口）。
///
/// 语义与 [`cleanup_keep_recent`] 完全一致（保留最新的 `max_items` 条非收藏条目、
/// 收藏项既不计数也不删除），多出来的只有一条**短路判据**：
///
/// `cleanup_keep_recent` 会先把全部非收藏 id 取回内存再截断——入库路径每次新增行都会调用它，
/// 而绝大多数轮次都还在上限内，那时这一步只是白取一遍 id、一行都删不了。
/// 先 `COUNT(*)` 一次就能把这个常态轮次收敛成单个聚合：计数走
/// `idx_clip_items_live_favorite_modified` 的等值前缀（`is_deleted` + `is_favorite`），
/// 既不扫全表也不需要临时排序。
///
/// 计数与裁剪之间理论上存在「另一个写入者在中间插入新行」的窗口，但本库的写入全部串行在
/// `AppDb` 的单个 Mutex 连接上（轮询线程与命令层共用），不存在并发写；即使发生，
/// 后果也只是这一轮少删一条、下一轮再收敛。
///
/// # Errors
/// `DbError::Sqlite`：计数或裁剪 SQL 失败
pub fn evict_over_limit(conn: &Connection, max_items: usize) -> Result<usize, DbError> {
    let live_non_favorite: i64 =
        conn.query_row(EVICT_LIVE_NON_FAVORITE_COUNT_SQL, [], |row| row.get(0))?;
    if live_non_favorite <= max_items as i64 {
        return Ok(0);
    }
    cleanup_keep_recent(conn, max_items)
}

/// `ingest` 的返回结果：新建行 vs 原行置顶刷新。
#[derive(Debug, PartialEq)]
pub enum IngestOutcome {
    /// 内容不存在，插入了新行；携带新行的 id
    Inserted(String),
    /// 内容已存在，原行已置顶刷新（last_modified_utc 已更新）；携带原行 id
    Bumped(String),
}

/// `ingest` 文本去重命中判定查询的 SQL（独立成常量，供查询计划回归测试断言生产语句本身）。
const INGEST_TEXT_DEDUP_SQL: &str = "SELECT id, html_content FROM clip_items
             WHERE text_hash = ?1 AND is_deleted = 0 LIMIT 1";

/// 去重入库：将捕获到的 `CapturedItem` 写入数据库，自动去重。
///
/// 流程：
/// 1. 计算 `item.text` 的 `text_hash`
/// 2. 查询 `clip_items` 中是否有未软删的同 hash 行
/// 3. 命中 → 调用 `bump_to_top`（刷新 `last_modified_utc`），返回 `Bumped(id)`
/// 4. 未命中 → 插入新行，返回 `Inserted(id)`
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn ingest(conn: &Connection, item: &CapturedItem) -> Result<IngestOutcome, DbError> {
    let hash = text_hash(&item.text);
    let now_ms = current_utc_ms();

    // 查询是否已有未软删的同 hash 行（连带其 html_content 以便补写判定）
    let existing: Option<(String, Option<String>)> = conn
        .query_row(INGEST_TEXT_DEDUP_SQL, rusqlite::params![hash], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .optional()?;

    if let Some((id, existing_html)) = existing {
        // 命中已有行：决策4——旧行无 html 而新来的带 html，则补写并升 kind，再置顶。
        // 其余情况（已有 html / 新来纯文本）仅置顶刷新，不动 html，纯文本去重语义不变。
        if existing_html.is_none() && item.html.is_some() {
            conn.execute(
                "UPDATE clip_items SET html_content = ?1, kind = 'richtext' WHERE id = ?2",
                rusqlite::params![item.html, id],
            )?;
        }
        bump_to_top(conn, &id)?;
        return Ok(IngestOutcome::Bumped(id));
    }

    // 未命中：插入新行。带 html 时 kind='richtext'，否则 'text'。
    let new_id = Uuid::new_v4().to_string();
    let kind = if item.html.is_some() {
        "richtext"
    } else {
        "text"
    };
    // 预计算 title 与去重键在同一个 INSERT 内写定：写入路径是本列的唯一新增来源，
    // 晚补会让「刚复制的条目搜不到」与「回填又要重扫新行」同时发生。
    let title = compute_title(kind, &item.text);
    conn.execute(
        "INSERT INTO clip_items
             (id, content, kind, created_utc, last_modified_utc, is_deleted, text_hash,
              html_content, title)
         VALUES (?1, ?2, ?3, ?4, ?4, 0, ?5, ?6, ?7)",
        rusqlite::params![new_id, item.text, kind, now_ms, hash, item.html, title],
    )?;

    Ok(IngestOutcome::Inserted(new_id))
}

/// 预计算 title 的最大字符数（Unicode 标量个数）。
///
/// 1000 是产品约定的搜索文本上界：它要短到「整库 title 能常驻内存做过滤」，
/// 又要长到「正常的一段话不丢搜索命中」。**不要在别处另设一个截断值**——
/// 前端与 DTO 都按这一条的产物走，长度散落成两处就会出现「列里 1000、显示 100」的漂移。
pub const TITLE_MAX_CHARS: usize = 1000;

/// 图片条目的 `kind` 取值（与前端 `ClipKind` 对齐）。
const KIND_IMAGE: &str = "image";

/// 把任意空白串折叠成单空格分隔的词串（去首尾空白）。
///
/// 为什么 title 必须做空白规范化：剪贴板里粘进来的多行文本/代码缩进会让同一句话
/// 在搜索文本里被换行切开，逐词子串匹配本身不敏感，但命中高亮的区间会把换行算进去，
/// 用户看到的高亮块会带上整段缩进。规范化后 title 是「单行等价物」。
fn collapse_whitespace(text: &str) -> String {
    let mut collapsed = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !collapsed.is_empty() {
            collapsed.push(' ');
        }
        collapsed.push_str(word);
    }
    collapsed
}

/// 计算某条目的预计算 title。
///
/// 规则（与前端 `clipSearchText` 的口径一致）：
/// - **图片条目留空**（空串，不是 NULL）：图片文本要靠后续 OCR 能力，本列不臆造内容。
/// - 其余条目取正文的空白规范化结果，再截断到 [`TITLE_MAX_CHARS`] 个字符。
///
/// 空串与 NULL 的语义必须分开：NULL = 「尚未回填」，空串 = 「回填过且按约定为空」。
/// 混用会让 [`backfill_titles_batch`] 的 `WHERE title IS NULL` 永远挑到图片行，无法收敛。
///
/// # Examples
/// ```
/// # use quickquick_lib::db::compute_title;
/// assert_eq!(compute_title("text", "  多行\n\t文本  "), "多行 文本");
/// assert_eq!(compute_title("image", "[图片] 12×34"), "");
/// ```
pub fn compute_title(kind: &str, content: &str) -> String {
    if kind == KIND_IMAGE {
        return String::new();
    }
    collapse_whitespace(content)
        .chars()
        .take(TITLE_MAX_CHARS)
        .collect()
}

/// title 回填的单批行数上限。
///
/// 200 行的理由：单批 = 一次「选 200 行 + 一个短写事务」，在 1 万行库上实测单批
/// 0.2–0.8 ms（见 dev-log 活动卡），既能保证单次持锁停在毫秒级（启动期不阻塞别的命令），
/// 又不至于把「每批固定开销（事务提交 + 一次 rowid 顺序扫描）」摊到太少行上——
/// 批越小，同一批行被反复扫到的次数越多。
pub const TITLE_BACKFILL_BATCH_SIZE: usize = 200;

/// 待回填候选：`(id, kind, content)`。
///
/// **`WHERE title IS NULL` 是幂等与可续跑的唯一落点**：一行被补上后立刻离开候选集，
/// 所以「跑一半被杀掉」后重跑只会挑剩下的行，已补行不会被重算。
const PENDING_TITLE_SQL: &str =
    "SELECT id, kind, content FROM clip_items WHERE title IS NULL ORDER BY rowid LIMIT ?1";

/// 单批写入 SQL。
///
/// 写入侧不再重复判 `title IS NULL`：候选集已经由 [`PENDING_TITLE_SQL`] 筛过一遍，
/// 而写入值由 [`compute_title`] 从 `content` 确定性算出——即便真有第二个写者补同一行，
/// 两者算出的值也相同，重写是幂等的。留一个「谁都覆盖不到」的条件只会让分支无人验证。
const BACKFILL_TITLE_ONE_SQL: &str = "UPDATE clip_items SET title = ?1 WHERE id = ?2";

/// 单批回填：把最多 `batch_size` 行 `title IS NULL` 的条目补上 title，返回本批写入行数。
///
/// 单批一个短事务：进程在批与批之间被杀掉时，已提交的批全部落盘、当前批整体回滚，
/// 因此「可中断」在批粒度上成立；重跑只挑 `title IS NULL`，天然「可续跑」。
///
/// **调用方负责循环与批间让锁**（见 `lib.rs` 的 `spawn_title_backfill`）：
/// 一次只补一批、批间释放 `AppDb` 锁，启动路径与用户命令不会被长写锁挡住。
/// 返回 0 即表示候选集已空，可以停。
///
/// # Errors
/// `DbError::Sqlite`：查询或 UPDATE 失败（事务已回滚，本批一行未写）
pub fn backfill_titles_batch(conn: &Connection, batch_size: usize) -> Result<usize, DbError> {
    if batch_size == 0 {
        return Ok(0);
    }

    let pending: Vec<(String, String, Option<String>)> = {
        let mut stmt = conn.prepare(PENDING_TITLE_SQL)?;
        let rows = stmt.query_map(rusqlite::params![batch_size as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut collected = Vec::with_capacity(batch_size);
        for row in rows {
            collected.push(row?);
        }
        collected
    };
    if pending.is_empty() {
        return Ok(0);
    }

    let tx = conn.unchecked_transaction()?;
    let mut written = 0usize;
    {
        let mut stmt = tx.prepare(BACKFILL_TITLE_ONE_SQL)?;
        for (id, kind, content) in &pending {
            let title = compute_title(kind, content.as_deref().unwrap_or_default());
            written += stmt.execute(rusqlite::params![title, id])?;
        }
    }
    tx.commit()?;
    Ok(written)
}

/// 显式置顶：仅更新 `last_modified_utc = now`，不新建任何记录。
///
/// 列表按 `last_modified_utc DESC` 排序时，该行将排到最前。
/// 业务语义：「置顶」由本函数**显式改库**实现，绝不通过重新捕获产生新记录。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败（id 不存在时影响行数为 0，不报错）
pub fn bump_to_top(conn: &Connection, id: &str) -> Result<(), DbError> {
    let now_ms = current_utc_ms();
    conn.execute(
        "UPDATE clip_items SET last_modified_utc = ?1 WHERE id = ?2",
        rusqlite::params![now_ms, id],
    )?;
    Ok(())
}

/// 返回未软删的行数。
///
/// 用于测试断言和业务层「历史条目总数」查询。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn count_live(conn: &Connection) -> Result<i64, DbError> {
    let n = conn.query_row(
        "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 0",
        [],
        |row| row.get(0),
    )?;
    Ok(n)
}

/// 返回 `last_modified_utc` 最新的未软删行的 `id`（即当前「最前」条目）。
///
/// 用于测试断言和业务层「取最新条目」查询。
/// 库为空或全部软删时返回 `Ok(None)`。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn top_id(conn: &Connection) -> Result<Option<String>, DbError> {
    let result = conn
        .query_row(
            "SELECT id FROM clip_items WHERE is_deleted = 0
             ORDER BY last_modified_utc DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(result)
}

/// 物理清理 GC：删除所有 `is_deleted=1` 的行，返回清理条数。
///
/// 仅删除已标记软删的行；正常行不受影响。
///
/// 注意：本函数只删 `clip_items`，`clip_images` 依赖外键 `ON DELETE CASCADE`
/// 随父条目一起物理消失。因此**没有父条目的孤立图片行**不会被这里清理，
/// 那条缺口由 [`purge_orphan_image_tombstones`] 补上（分级清理在**本次有实际清理动作时**
/// 才会调用它，日常未超限的轮次不为此付一次全表扫描）。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn gc_purge_deleted(conn: &Connection) -> Result<u64, DbError> {
    let count = conn.execute("DELETE FROM clip_items WHERE is_deleted = 1", [])?;
    Ok(count as u64)
}

/// 物理清除「无父条目可级联」的 `clip_images` 墓碑（孤立行回收），返回删除条数。
///
/// 为什么需要单独一条路径：`gc_purge_deleted` 只删 `clip_items` 墓碑，图片行靠
/// `ON DELETE CASCADE` 随父条目消失。而 `clip_item_id IS NULL`（或父条目已不存在）
/// 的孤立图片行没有任何父条目可级联，**所有既有 GC 路径都碰不到它**，
/// 其缩略图/原图 BLOB 会永久占库、`file_size_bytes` 永不下降。
///
/// 回收范围刻意收窄到「已软删 + 无父条目」：
/// - 未软删的孤立行仍可能被 [`crate::db::ingest_image_as_clip`] 的去重/领养路径使用，
///   不能删；
/// - 父条目尚存的图片行由级联负责，不在这里重复处理；
/// - 孤立行对 UI 不可见（列表以 `clip_items` 为主表），故物理删除不改变用户可见状态，
///   只是把本已无主的 BLOB 真正释放掉。
///
/// # 调用时机
/// 由 [`crate::image::tiered_cleanup`] 的第三级调用，且**仅当本次清理真的动过行**
/// （腾空原图或整条软删）时才执行——孤立墓碑只可能来自第二级删除或历史遗留，
/// 历史遗留会在第一次真正需要清理（有空间压力）时被顺带回收；
/// 未超限的日常轮次不做这次全表 `DELETE` 扫描。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn purge_orphan_image_tombstones(conn: &Connection) -> Result<u64, DbError> {
    let count = conn.execute(
        "DELETE FROM clip_images
         WHERE is_deleted = 1
           AND (clip_item_id IS NULL
                OR NOT EXISTS (
                    SELECT 1 FROM clip_items ci WHERE ci.id = clip_images.clip_item_id
                ))",
        [],
    )?;
    Ok(count as u64)
}

/// `ingest_image_as_clip` 图片去重命中判定查询的 SQL（独立成常量，供查询计划回归测试断言）。
const INGEST_IMAGE_DEDUP_SQL: &str = "SELECT id, clip_item_id FROM clip_images
             WHERE image_hash = ?1 AND is_deleted = 0
             LIMIT 1";

/// 图片剪贴板去重入库：将一张图片作为独立 clip_items 行写入，并关联 clip_images。
///
/// 行为：
/// 1. 用 `image::image_hash` 计算字节哈希，查 `clip_images` 是否已有未软删同 hash 行。
/// 2. 命中且 clip_item_id 非 NULL → bump 关联 clip_item 的 `last_modified_utc`，返回 `Bumped`。
/// 3. 未命中（或 clip_item_id 为 NULL 的孤立行）→ 在 SAVEPOINT 事务内：
///    a. INSERT `clip_items`（kind='image'，content 含尺寸）
///    b. 调用 `image::ingest_image_with_policy` 写 `clip_images`，得到 image_id
///    c. UPDATE `clip_images SET clip_item_id = item_id WHERE id = image_id`（补写外键，SAVEPOINT 内）
///    d. RELEASE SAVEPOINT 提交；任一步失败则 ROLLBACK SAVEPOINT，不留孤立行
///    返回 `Inserted`。
///
/// `max_image_bytes`：原图大小上限（字节），超出则只存缩略图（original_present=0）。
/// 调用方从 AppSettings.max_image_bytes 读取并传入，使阈值可由用户配置。
///
/// # Errors
/// - `DbError::Sqlite`：SQL 执行失败
/// - `DbError::Other`：图片解码/缩略图生成失败
pub fn ingest_image_as_clip(
    conn: &Connection,
    width: usize,
    height: usize,
    png_bytes: &[u8],
    max_image_bytes: u64,
) -> Result<IngestOutcome, DbError> {
    let hash = img_mod::image_hash(png_bytes);

    // 查是否已有同 hash 的未软删图片行
    let existing: Option<(String, Option<String>)> = conn
        .query_row(INGEST_IMAGE_DEDUP_SQL, rusqlite::params![hash], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .optional()?;

    // 合并嵌套 if let：仅当 clip_item_id 非 NULL 时才视为有效命中并 bump
    if let Some((_img_id, Some(item_id))) = existing {
        bump_to_top(conn, &item_id)?;
        return Ok(IngestOutcome::Bumped(item_id));
    }
    // clip_item_id 为 NULL（孤立行）或无命中：走新建路径，UPDATE 将领养孤立行

    insert_image_clip(conn, width, height, png_bytes, max_image_bytes)
}

/// 在 SAVEPOINT 事务内插入 clip_items + clip_images 并补写外键关联。
///
/// SAVEPOINT 生命周期：成功时 RELEASE，失败时 ROLLBACK TO + RELEASE。
fn insert_image_clip(
    conn: &Connection,
    width: usize,
    height: usize,
    png_bytes: &[u8],
    max_image_bytes: u64,
) -> Result<IngestOutcome, DbError> {
    let now_ms = current_utc_ms();
    let item_id = Uuid::new_v4().to_string();
    let content = format!("[图片] {width}×{height}");

    conn.execute_batch("SAVEPOINT ingest_image_clip;")?;

    let result =
        try_insert_image_clip(conn, &item_id, &content, now_ms, png_bytes, max_image_bytes);

    match result {
        Ok(()) => {
            conn.execute_batch("RELEASE SAVEPOINT ingest_image_clip;")?;
            Ok(IngestOutcome::Inserted(item_id))
        }
        Err(e) => {
            // 整体回滚，不留孤立 clip_items 行
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT ingest_image_clip;");
            let _ = conn.execute_batch("RELEASE SAVEPOINT ingest_image_clip;");
            Err(e)
        }
    }
}

/// SAVEPOINT 内的实际写操作：INSERT clip_items → ingest_image_with_policy → UPDATE clip_item_id。
///
/// 三步全在 SAVEPOINT 保护内，任一步失败由调用方 ROLLBACK。
fn try_insert_image_clip(
    conn: &Connection,
    item_id: &str,
    content: &str,
    now_ms: i64,
    png_bytes: &[u8],
    max_image_bytes: u64,
) -> Result<(), DbError> {
    // 图片条目的 title 写空串而非留 NULL：空串 = 「已定值且按约定为空」，
    // NULL 保留给「尚未回填」，否则回填的 `WHERE title IS NULL` 会永远挑到图片行。
    let title = compute_title(KIND_IMAGE, content);
    conn.execute(
        "INSERT INTO clip_items
             (id, content, kind, created_utc, last_modified_utc, is_deleted, text_hash, title)
         VALUES (?1, ?2, 'image', ?3, ?3, 0, NULL, ?4)",
        rusqlite::params![item_id, content, now_ms, title],
    )?;

    let policy = img_mod::OversizePolicy {
        max_original_bytes: max_image_bytes as usize,
    };
    let outcome = img_mod::ingest_image_with_policy(conn, png_bytes, &policy)?;

    // Bumped 命中孤立行时返回旧 image_id，UPDATE 将其关联到本次新建的 item_id
    let image_id = match outcome {
        img_mod::IngestImageOutcome::Inserted(id) => id,
        img_mod::IngestImageOutcome::Bumped(id) => id,
    };

    // 补写外键（ingest_image_with_policy 不写 clip_item_id 字段）
    let affected = conn.execute(
        "UPDATE clip_images SET clip_item_id = ?1 WHERE id = ?2",
        rusqlite::params![item_id, image_id],
    )?;
    if affected != 1 {
        return Err(DbError::Other(format!(
            "clip_item_id 补写影响行数异常：期望 1，实际 {affected}（image_id={image_id}）"
        )));
    }

    Ok(())
}

// ===================== 备份导入/导出辅助（只读 + 只插入，不改 schema） =====================
//
// 便携备份包的导出与导入需要两件事是 `ingest` 系列做不到的：
// 1. 导出要读到「原图 BLOB + 缩略图 + 创建时间 + 收藏位」的完整行（`ingest` 只写不读）；
// 2. 导入要按备份里的原始时间/收藏位原样插入，且命中去重键时**跳过而非 bump**
//    （`ingest` 的语义是「再次复制就置顶」，会把导入变成打乱现有顺序）。
//
// 因此这里只补最小集合的读取与插入原语，判重决策与事务边界留在 IPC 层。

/// 文本判重哈希的公开入口：与 `ingest` 使用的算法完全一致（FNV-1a 64-bit）。
///
/// 备份导入必须用同一算法重算哈希，否则与既有条目的判重键对不上，
/// 会把库里已有的条目再插一遍。
pub fn text_hash_of(text: &str) -> String {
    text_hash(text)
}

/// 备份导出用的图片字段。
#[derive(Debug, Clone, PartialEq)]
pub struct BackupImageRow {
    /// 库内原图字节哈希（列可为 NULL：返回 `None` 由调用方兜底）
    pub image_hash: Option<String>,
    /// 缩略图 BLOB
    pub thumbnail: Vec<u8>,
    /// 原图 BLOB（`original_present=0` 时为空 Vec）
    pub original: Vec<u8>,
    /// 原图是否已存（0 = 超过单张阈值被降级丢弃）
    pub original_present: bool,
}

/// 备份导出用的条目行：`clip_items` 业务字段 + 可选关联图片。
///
/// 不含条目 id：备份包按内容/图片哈希判重，不搬迁 id（新库里 id 重新生成）。
#[derive(Debug, Clone, PartialEq)]
pub struct BackupItemRow {
    /// 条目内容
    pub content: String,
    /// 条目类型（"text" / "richtext" / "image"）
    pub kind: String,
    /// 创建时间（UTC epoch ms）
    pub created_utc: i64,
    /// 最后修改时间（UTC epoch ms）
    pub last_modified_utc: i64,
    /// 收藏标记
    pub is_favorite: bool,
    /// 富文本 HTML
    pub html_content: Option<String>,
    /// 关联图片（文本条目为 `None`）
    pub image: Option<BackupImageRow>,
}

/// 读取全部未软删条目（含图片原图/缩略图），顺序与列表页一致。
///
/// 每个条目最多关联一张未软删图片：关联条件用「按 rowid 取第一张」的相关子查询，
/// 避免同一 `clip_item_id` 有多行 `clip_images` 时把一个条目展开成多条。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn list_backup_items(conn: &Connection) -> Result<Vec<BackupItemRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT ci.content, ci.kind, ci.created_utc, ci.last_modified_utc,
                ci.is_favorite, ci.html_content,
                cimg.id, cimg.image_hash, cimg.thumbnail, cimg.original, cimg.original_present
         FROM clip_items ci
         LEFT JOIN clip_images cimg ON cimg.id = (
             SELECT id FROM clip_images
             WHERE clip_item_id = ci.id AND is_deleted = 0
             ORDER BY rowid LIMIT 1
         )
         WHERE ci.is_deleted = 0
         ORDER BY ci.is_favorite DESC, ci.last_modified_utc DESC, ci.rowid DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        // 关联到图片行的判据是图片 id 非 NULL（文本条目的 JOIN 结果全为 NULL）
        let image = match row.get::<_, Option<String>>(6)? {
            Some(_) => Some(BackupImageRow {
                image_hash: row.get(7)?,
                thumbnail: row.get::<_, Option<Vec<u8>>>(8)?.unwrap_or_default(),
                original: row.get::<_, Option<Vec<u8>>>(9)?.unwrap_or_default(),
                original_present: row.get::<_, Option<i64>>(10)?.unwrap_or(0) != 0,
            }),
            None => None,
        };

        Ok(BackupItemRow {
            content: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            kind: row.get(1)?,
            created_utc: row.get(2)?,
            last_modified_utc: row.get(3)?,
            is_favorite: row.get::<_, i64>(4)? != 0,
            html_content: row.get(5)?,
            image,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 库里是否已有未软删的同内容文本条目（判重键 = `text_hash`）。
///
/// 只看未软删行：软删条目的内容视为「用户已丢弃」，导入应重新建行（与 `ingest` 一致）。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn has_live_text_item(conn: &Connection, text_hash: &str) -> Result<bool, DbError> {
    let found = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM clip_items WHERE text_hash = ?1 AND is_deleted = 0)",
        rusqlite::params![text_hash],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(found != 0)
}

/// 库里是否已有未软删的同图行（判重键 = `clip_images.image_hash`）。
///
/// 含 `clip_item_id` 为 NULL 的孤立图片行：那种行同样占着这个去重键，
/// 再插一条会造成同图两行。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn has_live_image_item(conn: &Connection, image_hash: &str) -> Result<bool, DbError> {
    let found = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM clip_images WHERE image_hash = ?1 AND is_deleted = 0)",
        rusqlite::params![image_hash],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(found != 0)
}

/// 备份导入：按备份里的原始时间与收藏位插入文本/富文本条目，返回新行 id。
///
/// 判重由调用方先做；本函数不查重、不 bump 既有行。
/// 条目的 `kind` 由 `html` 是否存在推导，与 `ingest` 的推导规则一致。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn insert_backup_text_item(
    conn: &Connection,
    content: &str,
    html: Option<&str>,
    is_favorite: bool,
    created_utc: i64,
    last_modified_utc: i64,
) -> Result<String, DbError> {
    let id = Uuid::new_v4().to_string();
    let kind = if html.is_some() { "richtext" } else { "text" };
    let hash = text_hash(content);

    conn.execute(
        "INSERT INTO clip_items
             (id, content, kind, created_utc, last_modified_utc, is_deleted,
              text_hash, is_favorite, html_content, title)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            id,
            content,
            kind,
            created_utc,
            last_modified_utc,
            hash,
            is_favorite as i64,
            html,
            compute_title(kind, content)
        ],
    )?;

    Ok(id)
}

/// 备份导入要写入的图片条目字段集。
///
/// 用结构体而非长参数列表：字段超过 5 个且同属一条记录，平铺参数极易把
/// 两个相邻的时间戳或两个相邻的 BLOB 传反。
#[derive(Debug, Clone, Copy)]
pub struct BackupImageInsert<'a> {
    /// 条目内容（图片条目的尺寸描述）
    pub content: &'a str,
    /// 收藏标记
    pub is_favorite: bool,
    /// 创建时间（UTC epoch ms）
    pub created_utc: i64,
    /// 最后修改时间（UTC epoch ms）
    pub last_modified_utc: i64,
    /// 原图判重键
    pub image_hash: &'a str,
    /// 缩略图 PNG
    pub thumbnail: &'a [u8],
    /// 原图 PNG（`original_present=false` 时传空切片）
    pub original: &'a [u8],
    /// 原图是否已存
    pub original_present: bool,
}

/// 备份导入：按备份里的原始字段插入图片条目及其 `clip_images` 行，返回新条目 id。
///
/// 两行必须同生共死：任一 INSERT 失败时由调用方的 SAVEPOINT 整体回滚，
/// 因此本函数不自带事务（嵌套 SAVEPOINT 会让上层回滚边界变得不可预测）。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn insert_backup_image_item(
    conn: &Connection,
    row: &BackupImageInsert<'_>,
) -> Result<String, DbError> {
    let item_id = Uuid::new_v4().to_string();
    let image_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO clip_items
             (id, content, kind, created_utc, last_modified_utc, is_deleted,
              text_hash, is_favorite, title)
         VALUES (?1, ?2, 'image', ?3, ?4, 0, NULL, ?5, ?6)",
        rusqlite::params![
            item_id,
            row.content,
            row.created_utc,
            row.last_modified_utc,
            row.is_favorite as i64,
            compute_title(KIND_IMAGE, row.content)
        ],
    )?;

    conn.execute(
        "INSERT INTO clip_images
             (id, clip_item_id, thumbnail, original, original_present, image_hash,
              created_utc, last_modified_utc, is_deleted, is_favorite)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
        rusqlite::params![
            image_id,
            item_id,
            row.thumbnail,
            row.original,
            row.original_present as i64,
            row.image_hash,
            row.created_utc,
            row.last_modified_utc,
            row.is_favorite as i64
        ],
    )?;

    Ok(item_id)
}

/// 用指定 key 打开 SQLCipher 数据库并触发解密校验。
///
/// 开库成功后立即开启外键约束（`PRAGMA foreign_keys = ON`）。
/// rusqlite/SQLite 每条连接默认外键为 OFF，必须在每次开库后显式启用。
fn open_with_key(path: &Path, key: &[u8; 32]) -> Result<Connection, DbError> {
    // 确保父目录存在（首次启动时目录可能未创建）
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;

    // I-01：SQLCipher 的 PRAGMA key 不支持 `?` 参数化（上游约束，SQLCipher 协议限制）。
    // 此处字符串构造为代码库中唯一的参数化例外：
    //   - hex_key 来自受控 [u8;32]，内容仅 64 位十六进制字符（0-9/a-f），不可注入；
    //   - 未启用 SQLite trace 钩子，故 PRAGMA 语句不会写入日志或泄漏密钥材料。
    let hex_key = hex_encode(key);
    conn.execute_batch(&format!("PRAGMA key = \"x'{hex_key}'\";"))?;

    // 轻量查询触发解密校验：错误密钥在此暴露，避免延迟到写操作时才报错
    conn.execute_batch("PRAGMA user_version;")?;

    // I-03：每条连接必须显式开启外键约束（SQLite 默认 OFF）
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    Ok(conn)
}

/// 当前 schema 版本，即 `PRAGMA user_version` 的目标值。
///
/// 改动表结构（新增列 / 索引 / 表）时必须成对做两件事：向 `MIGRATIONS` 末尾追加一个
/// `to_version` 为本值 +1 的步骤，并把本常量 +1。两者由
/// `migration_list_is_contiguous_up_to_current_version` 与
/// `apply_migrations` 的运行期校验双重钉住——只改一边会立刻在开库时报错。
pub const CURRENT_SCHEMA_VERSION: i32 = 2;

/// 单个 schema 迁移步骤：把库从 `to_version - 1` 升到 `to_version`。
struct Migration {
    /// 目标版本号，必须比上一步大 1（不允许跳号，跳号由 `apply_migrations` 拒绝）。
    to_version: i32,
    /// 迁移体。在调用方开启的事务内执行，返回 `Err` 时整笔事务回滚。
    ///
    /// 步骤一旦随版本发布就**不可再改写**——库可能早已越过这一步，
    /// 改了只会让「已升级」和「待升级」的库走向不同形态。
    apply: fn(&Transaction<'_>) -> Result<(), DbError>,
}

/// 有序迁移步骤表（生产列表）。
///
/// 版本 1 是从「无版本号时代」到「版本化」的过渡步骤，同时承担建表、建索引与形态对齐，
/// 因此内含一次列存在性探测（见 [`reconcile_html_column`]）。版本 2 起每步只做无条件的
/// 增量 DDL，不再需要探测。
const MIGRATIONS: &[Migration] = &[
    Migration {
        to_version: 1,
        apply: baseline_schema_v1,
    },
    Migration {
        to_version: 2,
        apply: add_title_column_v2,
    },
];

/// 版本 2：`clip_items` 增加预计算 `title` 列。
///
/// 只做**无条件**的 `ALTER TABLE ADD COLUMN`——不在这里判存在性（版本号已经确定形态），
/// 也**不在这里回填**：本步骤跑在开库同步路径上，全表 `UPDATE` 会把写锁拖到秒级，
/// 同时 5 条索引也会全表扫。旧库升级后既有行的 `title` 为 NULL，表示「尚未回填」，
/// 由开库后的 [`backfill_titles_batch`] 分批补齐。
fn add_title_column_v2(tx: &Transaction<'_>) -> Result<(), DbError> {
    add_title_column(tx)
}

/// 版本 2 的 DDL 本体（接受 `&Connection`，以便测试库直接复用同一句，而不是手抄列定义）。
///
/// `Transaction` 自动解引用成 `Connection`，迁移步骤只需一层转调，不必有第二份 DDL 文本。
fn add_title_column(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch("ALTER TABLE clip_items ADD COLUMN title TEXT;")?;
    Ok(())
}

/// 初始化 / 升级 schema（幂等）。
///
/// 开库后的唯一 schema 入口：读 `PRAGMA user_version`，落后则按 [`MIGRATIONS`] 顺序升到
/// [`CURRENT_SCHEMA_VERSION`]；已是最新则一步 DDL 都不执行；版本超前（旧程序打开新库）
/// 直接报 [`DbError::SchemaTooNew`]，绝不按老结构继续读写。
///
/// 预埋字段遵循设计§十铁律：UUID 主键、UTC 时间戳、last_modified、墓碑、图片表拆分。
///
/// I-03（§十预埋）：外键约束必须在**事务外**开启——`PRAGMA foreign_keys` 在事务内是
/// 静默 no-op，放进迁移事务里等于没开（open_with_key 已开过一次，这里是防绕过它的兜底）。
///
/// # Errors
/// - `DbError::SchemaTooNew`：库版本高于本程序支持的版本
/// - `DbError::MigrationFailed`：某步迁移失败（事务已回滚，库停在迁移前版本）
/// - `DbError::Other`：迁移列表本身不自洽（跳号 / 未与常量同步）
/// - `DbError::Sqlite`：读版本号、开启事务等底层失败
fn ensure_schema(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    apply_migrations(conn, CURRENT_SCHEMA_VERSION, MIGRATIONS)
}

/// 把库迁移到 `target_version`，只执行尚未应用的步骤。
///
/// `migrations` 做成参数而不是直接读 [`MIGRATIONS`]，是为了让测试能注入**自己的**步骤
/// 列表（含故意失败的步骤）来验证回滚；生产路径只传 [`MIGRATIONS`]。
///
/// # 事务性
/// 全部步骤 + 每一步的版本号写入在同一个事务内完成：任一步失败 → 整笔回滚，
/// `user_version` 与表结构一起退回迁移前状态，不会留下「表改了但版本没写」的半成品库。
///
/// # Errors
/// - `DbError::SchemaTooNew`：当前版本已高于目标版本（先校验，未动库）
/// - `DbError::MigrationFailed`：某步或其版本号写入失败（已回滚）
/// - `DbError::Other`：迁移列表跳号，或最高版本与目标版本不一致
/// - `DbError::Sqlite`：读版本号 / 开启事务失败
fn apply_migrations(
    conn: &Connection,
    target_version: i32,
    migrations: &[Migration],
) -> Result<(), DbError> {
    let current = read_schema_version(conn)?;
    if current > target_version {
        return Err(DbError::SchemaTooNew {
            found: current,
            supported: target_version,
        });
    }
    if current == target_version {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;
    let mut version = current;
    for step in migrations {
        if step.to_version <= version {
            continue;
        }
        if step.to_version != version + 1 {
            return Err(DbError::Other(format!(
                "schema 迁移列表跳号：版本 {version} 之后应为 {}，实际出现 {}",
                version + 1,
                step.to_version
            )));
        }
        let step_result = (step.apply)(&tx).and_then(|()| {
            tx.pragma_update(None, "user_version", step.to_version)
                .map_err(DbError::from)
        });
        if let Err(reason) = step_result {
            // `tx` 在此处 drop → ROLLBACK，库停在 `current`。
            return Err(DbError::MigrationFailed {
                to_version: step.to_version,
                from_version: current,
                reason: reason.to_string(),
            });
        }
        version = step.to_version;
    }
    if version != target_version {
        return Err(DbError::Other(format!(
            "schema 迁移列表最高版本是 {version}，低于目标版本 {target_version}；\
             把 CURRENT_SCHEMA_VERSION 提到 {target_version} 之后必须补上对应版本的迁移步骤"
        )));
    }
    tx.commit().map_err(|e| DbError::MigrationFailed {
        to_version: target_version,
        from_version: current,
        reason: e.to_string(),
    })?;
    Ok(())
}

/// 读取当前 schema 版本（`PRAGMA user_version`）。
///
/// # Errors
/// `DbError::Sqlite`：PRAGMA 执行失败（未解密 / 连接不可用）
fn read_schema_version(conn: &Connection) -> Result<i32, DbError> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

/// 版本 1 基线：建表 + 对齐历史形态 + 建索引。
///
/// 为什么建表和建索引都收进这一步、而不是留在每次开库的路径上重跑：
/// 版本号是「库是什么形态」的唯一真相。若把 `CREATE ... IF NOT EXISTS` 留在开库路径上，
/// 以后往那两处加一条列 / 索引就会在用户开库时**绕过版本号**生效，同一个
/// `user_version` 下长出两种表形态，后续迁移步骤的前提就不成立了。收进版本 1 之后，
/// [`create_tables`] / [`ensure_indexes`] 冻结为「版本 1 快照」，新增结构只能走新步骤。
///
/// 代价是表 / 索引被外力删掉后不会再被自动补回。这个代价被刻意接受：自动补回意味着
/// 「DDL 永远跑在版本号之外」，而这正是要消灭的不确定性；真出现缺表缺索引，
/// 报错位置（第一条失败的查询）比静默重建更容易定位。
fn baseline_schema_v1(tx: &Transaction<'_>) -> Result<(), DbError> {
    create_tables(tx)?;
    reconcile_html_column(tx)?;
    ensure_indexes(tx)?;
    Ok(())
}

/// 建立版本 1 基线的全部业务表（`IF NOT EXISTS`）。
///
/// **已冻结**：这里描述的是版本 1 的表形态，不得再新增列 / 表——新库与老库都由
/// [`baseline_schema_v1`] 走到同一形态，往这里加东西会让「user_version 相同但表不同」
/// 的两个库并存。新增结构请追加迁移步骤。
///
/// 外键开关不在这里打开：本函数在迁移事务内执行，而 `PRAGMA foreign_keys` 在事务内是
/// no-op；开关由 [`ensure_schema`] 在事务外统一负责。
fn create_tables(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "
        -- 剪贴板条目表
        CREATE TABLE IF NOT EXISTS clip_items (
            id                TEXT PRIMARY KEY NOT NULL,  -- UUID，永不复用
            content           TEXT,
            kind              TEXT NOT NULL DEFAULT 'text',
            created_utc       INTEGER NOT NULL,            -- UTC epoch ms
            last_modified_utc INTEGER NOT NULL,            -- UTC epoch ms
            is_deleted        INTEGER NOT NULL DEFAULT 0,  -- 墓碑：0=正常 1=软删
            deleted_at_utc    INTEGER,                     -- 软删时间（UTC epoch ms）
            text_hash         TEXT,                        -- 纯文本内容哈希，用于去重（非加密，判重用途）
            is_favorite       INTEGER NOT NULL DEFAULT 0,  -- 收藏标记：0=普通 1=收藏（§九.2 ★置顶收藏）
            html_content      TEXT                         -- 富文本 HTML（NULL=纯文本）；随整库 SQLCipher 加密
        );

        -- 图片表：缩略图/原图拆分（设计§五/§十）
        -- ON DELETE CASCADE：GC 删除 clip_items 时级联清理关联图片（§五分级清理）
        CREATE TABLE IF NOT EXISTS clip_images (
            id                TEXT PRIMARY KEY NOT NULL,  -- UUID，永不复用
            clip_item_id      TEXT REFERENCES clip_items(id) ON DELETE CASCADE,
            thumbnail         BLOB,                       -- 缩略图 BLOB（拆分存储）
            original          BLOB,                       -- 原图 BLOB（拆分存储，原样无损）
            original_present  INTEGER NOT NULL DEFAULT 0, -- 1=有原图 0=仅缩略图（降级态）
            image_hash        TEXT,                       -- 原图字节哈希，用于判重（FNV-1a 64-bit）
            created_utc       INTEGER NOT NULL,            -- UTC epoch ms
            last_modified_utc INTEGER NOT NULL,            -- UTC epoch ms
            is_deleted        INTEGER NOT NULL DEFAULT 0,  -- 墓碑
            deleted_at_utc    INTEGER,                     -- 软删时间
            is_favorite       INTEGER NOT NULL DEFAULT 0  -- 收藏标记：0=普通 1=收藏（分级清理豁免，V3-F1-A04）
        );

        -- provider 非密凭据表（§十预埋铁律）
        -- 非密字段（is_secret=false）存此表；secret 字段存 provider_secret。
        CREATE TABLE IF NOT EXISTS provider_config (
            provider_id  TEXT NOT NULL,
            field_key    TEXT NOT NULL,
            value        TEXT NOT NULL,
            PRIMARY KEY (provider_id, field_key)
        );

        -- provider secret 凭据表（本地密钥库改造，去 Keychain）。
        -- secret 字段（is_secret=true）的值存进同一张 SQLCipher 整库加密的 DB，
        -- 不再走 OS 钥匙串、连明文 JSON 都不留。展示路径用 SELECT 1 判断「已配置」不回明文。
        CREATE TABLE IF NOT EXISTS provider_secret (
            provider_id  TEXT NOT NULL,
            field_key    TEXT NOT NULL,
            value        TEXT NOT NULL,
            PRIMARY KEY (provider_id, field_key)
        );

        -- 迁移：退役 secret_presence 标记表（secret 已进 provider_secret，标记表失去意义）。
        DROP TABLE IF EXISTS secret_presence;

        -- 翻译缓存表（§4.1#5 预埋，LRU 淘汰；键=四元组哈希）
        -- cache_key = hash(source_text + source_lang + target_lang + provider_id)
        -- last_used_utc 用于 LRU 淘汰排序（命中时刷新）
        CREATE TABLE IF NOT EXISTS translation_cache (
            cache_key      TEXT PRIMARY KEY NOT NULL,
            source_text    TEXT NOT NULL,
            source_lang    TEXT NOT NULL,
            target_lang    TEXT NOT NULL,
            provider_id    TEXT NOT NULL,
            translated     TEXT NOT NULL,
            created_utc    INTEGER NOT NULL,
            last_used_utc  INTEGER NOT NULL
        );

        -- 翻译历史表（V2-F3-A14）：独立于 clip_items，记录用户主动翻译的历史
        -- 剪贴板条目一键翻译后写入此表，两者互不混入
        CREATE TABLE IF NOT EXISTS translate_history (
            id              TEXT PRIMARY KEY NOT NULL,
            source_text     TEXT NOT NULL,
            translated_text TEXT NOT NULL,
            source_lang     TEXT NOT NULL,
            target_lang     TEXT NOT NULL,
            provider_id     TEXT NOT NULL,
            created_utc     INTEGER NOT NULL
        );
        ",
    )?;
    Ok(())
}

/// 建立版本 1 基线的查询索引（`IF NOT EXISTS`；不新增/修改任何列）。
///
/// 索引决定列表首屏、去重、分级清理这三类查询是「按索引定位」还是「全表扫 + 临时排序」。
/// 每条索引的列顺序都由对应查询的 WHERE 等值前缀与 ORDER BY 决定，不是按「多建几个」堆的。
///
/// 命名统一 `idx_<表名>_*` 前缀，便于按前缀枚举与核对。
///
/// **已冻结**：这 5 条是版本 1 基线的一部分。新增索引不要写在这里（它在版本 1 的事务内
/// 只跑一次，写在这里的新索引对已升级的库永远不生效），而要追加一个迁移步骤；新索引在
/// 自己的步骤里可直接写无 `IF NOT EXISTS` 的 `CREATE INDEX`，形态由版本号保证。
///
/// # Errors
/// `DbError::Sqlite`：任一 `CREATE INDEX` 执行失败（表不存在、库只读等）
fn ensure_indexes(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "
        -- 列表主查询（list_items_with_images / list_items_full / list_ordered /
        -- list_backup_items 共用同一过滤与排序）：
        --   WHERE is_deleted = 0
        --   ORDER BY is_favorite DESC, last_modified_utc DESC, rowid DESC
        -- is_favorite 必须排在 last_modified_utc 之前：它是排序的第一键，省掉它之后
        -- SQLite 仍能把 is_deleted 走索引，却必须把全部未软删行取回来做一次临时 B-tree
        -- 排序（EXPLAIN QUERY PLAN 中的 USE TEMP B-TREE FOR ORDER BY），列表越长越慢。
        -- 三列全 ASC 建，查询反向扫描即为全 DESC；rowid 是每个索引隐含的末列，所以
        -- 「同毫秒用 rowid DESC 兜底」这一层排序同样被索引满足。
        CREATE INDEX IF NOT EXISTS idx_clip_items_live_favorite_modified
            ON clip_items (is_deleted, is_favorite, last_modified_utc);

        -- 文本去重（ingest 的命中判定、备份导入的 has_live_text_item）：
        --   WHERE text_hash = ? AND is_deleted = 0
        -- is_deleted 作为第二列不是为了过滤（哈希已近乎唯一），而是让两个等值条件都落在
        -- 索引前缀上：只建 (text_hash) 时，在没有 sqlite_stat1 统计的库上规划器会把
        -- 「按 text_hash 定位」与「按 is_deleted 定位」估算成等价代价，从而改走
        -- is_deleted 前缀索引，等于把全部未软删行扫一遍。两列前缀让这条路径稳定命中。
        CREATE INDEX IF NOT EXISTS idx_clip_items_text_hash
            ON clip_items (text_hash, is_deleted);

        -- 图片去重（ingest_image_as_clip 的命中判定、备份导入的 has_live_image_item）：
        --   WHERE image_hash = ? AND is_deleted = 0
        -- 列顺序理由与文本去重索引相同，且它可覆盖 has_live_image_item 的存在性判定。
        CREATE INDEX IF NOT EXISTS idx_clip_images_image_hash
            ON clip_images (image_hash, is_deleted);

        -- 分级清理的候选查询（image::tiered_cleanup 两级）：
        --   第一级 WHERE is_deleted = 0 AND is_favorite = 0 AND original_present = 1
        --   第二级 WHERE is_deleted = 0 AND is_favorite = 0
        --   两级都 ORDER BY created_utc ASC
        -- 两个等值列在前，两级查询都能顺扫索引直接拿到 created_utc 有序候选。
        -- original_present 不进索引：它只出现在第一级，插在 created_utc 之前会破坏
        -- 第二级的排序前缀，让第二级重新退化成临时排序。
        CREATE INDEX IF NOT EXISTS idx_clip_images_live_favorite_created
            ON clip_images (is_deleted, is_favorite, created_utc);

        -- 图片按父条目反查：list_items_with_images 的 LEFT JOIN、list_backup_items 的
        -- 相关子查询都按 clip_images.clip_item_id 定位。同时它是外键
        -- clip_images.clip_item_id 的级联索引——SQLite 执行 ON DELETE CASCADE 时，
        -- 每删一条父行都要在子表按该列找一次，缺索引就退化为逐行全表扫子表。
        CREATE INDEX IF NOT EXISTS idx_clip_images_clip_item_id
            ON clip_images (clip_item_id);
        ",
    )?;
    Ok(())
}

/// 检测某表是否已含指定列（基于 `PRAGMA table_info`）。
///
/// 仅供版本 1 的历史形态对齐使用（见 [`reconcile_html_column`]）。正常迁移步骤不需要它：
/// 版本号已经确定了库的形态。SQLite 无 `ADD COLUMN IF NOT EXISTS`，所以有它作为探测手段。
///
/// # Errors
/// `DbError::Sqlite`：PRAGMA 查询失败
fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, DbError> {
    // table_info 的列名在第 1 位（cid, name, type, ...）；table 名来自内部常量，非用户输入。
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// 版本 1 的对齐步骤：给「无版本号时代」的存量 `clip_items` 补 `html_content TEXT` 列。
///
/// 这里的探测是**整条迁移链上唯一允许的形态探测**。原因：`PRAGMA user_version` 引入之前
/// 的库共用版本号 0，其中混着三种形态——①全新库（还没有表，由 `create_tables` 直接建成
/// 版本 1 形态）②无 `html_content` 列的旧库 ③已有 `html_content` 列但版本号仍是 0 的库。
/// 三种都必须落到版本 1，只能按列是否存在区分。
///
/// 版本 2 起不再需要这种守卫（版本号已经说明库处在哪一步），新列直接在步骤里写无条件
/// `ALTER TABLE ... ADD COLUMN`。**新增列时不要复制这里的 `has_column` 判断。**
///
/// 新列默认 NULL，随整库 SQLCipher 加密。
///
/// # Errors
/// `DbError::Sqlite`：PRAGMA 或 ALTER 执行失败
fn reconcile_html_column(conn: &Connection) -> Result<(), DbError> {
    if has_column(conn, "clip_items", "html_content")? {
        return Ok(());
    }
    conn.execute_batch("ALTER TABLE clip_items ADD COLUMN html_content TEXT;")?;
    Ok(())
}

/// 将损坏文件改名备份，返回备份路径。
///
/// 备份格式：`<原路径>.corrupt-<utc_secs>`。
/// 永不删除原内容，仅 rename（OS 层原子操作）。
///
/// `pub(crate)`：setup_app_db 的「一次性重置」分支复用此函数备份旧库 / 旧 master.key。
pub(crate) fn backup_corrupt_file(path: &Path) -> Result<std::path::PathBuf, DbError> {
    let utc_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let original_name = path
        .file_name()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "backup_corrupt_file: 路径无文件名分量",
            )
        })?
        .to_string_lossy();
    let backup_name = format!("{}.corrupt-{}", original_name, utc_secs);

    let backup_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(backup_name);

    std::fs::rename(path, &backup_path)?;
    Ok(backup_path)
}

/// 计算文本判重哈希（显式稳定 FNV-1a 64-bit，跨 Rust 版本/构建一致；非加密，仅用于内容去重）。
fn text_hash(text: &str) -> String {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// 获取当前 UTC 时间戳（毫秒）。
fn current_utc_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// 将字节切片编码为小写十六进制字符串。
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, MutexGuard};

    /// 构造最小合法 2×2 RGBA PNG（用 image crate 编码，不依赖外部文件）。
    ///
    /// 返回的字节可被 `image::load_from_memory` 正确解码，用于 ingest_image_as_clip 测试。
    fn make_test_png(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageFormat, RgbaImage};
        let img = RgbaImage::new(width, height);
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .expect("test PNG 编码失败");
        buf
    }

    /// 建立内存 SQLite 并初始化 clip_items + clip_images 表（与 ensure_schema 一致）。
    ///
    /// 测试绕开加密路径直接开内存库，手动建表保持测试自包含。
    fn make_test_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS clip_items (
                 id                TEXT PRIMARY KEY NOT NULL,
                 content           TEXT,
                 kind              TEXT NOT NULL DEFAULT 'text',
                 created_utc       INTEGER NOT NULL,
                 last_modified_utc INTEGER NOT NULL,
                 is_deleted        INTEGER NOT NULL DEFAULT 0,
                 deleted_at_utc    INTEGER,
                 text_hash         TEXT,
                 is_favorite       INTEGER NOT NULL DEFAULT 0,
                 html_content      TEXT,
                 title             TEXT  -- 版本 2 增量列：手抄 DDL 必须与 CURRENT_SCHEMA_VERSION 同步
             );
             CREATE TABLE IF NOT EXISTS clip_images (
                 id                TEXT PRIMARY KEY NOT NULL,
                 clip_item_id      TEXT REFERENCES clip_items(id) ON DELETE CASCADE,
                 thumbnail         BLOB,
                 original          BLOB,
                 original_present  INTEGER NOT NULL DEFAULT 0,
                 image_hash        TEXT,
                 created_utc       INTEGER NOT NULL,
                 last_modified_utc INTEGER NOT NULL,
                 is_deleted        INTEGER NOT NULL DEFAULT 0,
                 deleted_at_utc    INTEGER,
                 is_favorite       INTEGER NOT NULL DEFAULT 0
             );",
        )
        .expect("建测试表失败");
        conn
    }

    /// T1：基础入库 — 新图应写入一条 clip_items(kind='image') 和一条
    /// clip_images，且 clip_images.clip_item_id 非 NULL 指向该 item。
    #[test]
    fn ingest_image_as_clip_inserts_item_and_links_image() {
        let conn = make_test_conn();
        let png = make_test_png(2, 2);

        let outcome = ingest_image_as_clip(&conn, 2, 2, &png, 20 * 1024 * 1024)
            .expect("ingest_image_as_clip 不应返回 Err");

        let item_id = match &outcome {
            IngestOutcome::Inserted(id) => id.clone(),
            IngestOutcome::Bumped(_) => panic!("首次入库应返回 Inserted，得到 Bumped"),
        };

        // clip_items 恰好 1 行，kind='image'，content 含尺寸
        let (kind, content): (String, String) = conn
            .query_row(
                "SELECT kind, content FROM clip_items WHERE id = ?1 AND is_deleted = 0",
                rusqlite::params![item_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("clip_items 中应有该 item");
        assert_eq!(kind, "image", "kind 应为 'image'");
        assert!(
            content.contains("2") && content.contains("×"),
            "content 应含尺寸信息，实际: {content}"
        );

        // clip_images 恰好 1 行，clip_item_id = item_id（非 NULL）
        let linked_item_id: String = conn
            .query_row(
                "SELECT clip_item_id FROM clip_images WHERE is_deleted = 0",
                [],
                |row| row.get(0),
            )
            .expect("clip_images 中应有图片行");
        assert_eq!(
            linked_item_id, item_id,
            "clip_images.clip_item_id 应指向新建的 clip_items 行"
        );

        // 总行数断言：各表恰好 1 行
        let item_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(item_count, 1, "clip_items 应恰好 1 行");
        let img_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_images WHERE is_deleted = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(img_count, 1, "clip_images 应恰好 1 行");
    }

    /// T2：去重 bump — 同一 PNG 二次调用应返回 Bumped，行数不变，
    /// 且对应 clip_item.last_modified_utc 被刷新。
    #[test]
    fn ingest_image_as_clip_dedup_bumps_timestamp() {
        let conn = make_test_conn();
        let png = make_test_png(4, 4);

        // 首次入库
        let first =
            ingest_image_as_clip(&conn, 4, 4, &png, 20 * 1024 * 1024).expect("首次入库失败");
        let first_id = match &first {
            IngestOutcome::Inserted(id) => id.clone(),
            _ => panic!("首次应 Inserted"),
        };

        // 记录首次 last_modified_utc
        let ts_before: i64 = conn
            .query_row(
                "SELECT last_modified_utc FROM clip_items WHERE id = ?1",
                rusqlite::params![first_id],
                |r| r.get(0),
            )
            .unwrap();

        // 确保时间戳可区分（至少等 1ms，系统调用精度通常满足；若并发过快此处仍可能相等，
        // 业务语义是"刷新"，相等也属正确行为，故断言用 >=）
        std::thread::sleep(std::time::Duration::from_millis(5));

        // 二次调用
        let second =
            ingest_image_as_clip(&conn, 4, 4, &png, 20 * 1024 * 1024).expect("二次调用失败");
        assert!(
            matches!(second, IngestOutcome::Bumped(_)),
            "同一 PNG 二次调用应返回 Bumped"
        );

        // clip_items 仍 1 行
        let item_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(item_count, 1, "去重后 clip_items 应仍 1 行");

        // clip_images 仍 1 行
        let img_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_images WHERE is_deleted = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(img_count, 1, "去重后 clip_images 应仍 1 行");

        // last_modified_utc 已刷新（>= 首次值）
        let ts_after: i64 = conn
            .query_row(
                "SELECT last_modified_utc FROM clip_items WHERE id = ?1",
                rusqlite::params![first_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            ts_after >= ts_before,
            "bump 后 last_modified_utc 应 >= 首次值，before={ts_before} after={ts_after}"
        );
    }

    /// T3：事务回滚 — 非法 PNG（空字节）应使 make_thumbnail 失败，
    /// 整体回滚，clip_items 无孤立行，函数返回 Err 不 panic。
    #[test]
    fn ingest_image_as_clip_invalid_png_rolls_back() {
        let conn = make_test_conn();
        let bad_png: &[u8] = b""; // 空字节，不是合法 PNG

        let result = ingest_image_as_clip(&conn, 1, 1, bad_png, 20 * 1024 * 1024);
        assert!(result.is_err(), "非法 PNG 应返回 Err");

        // clip_items 无孤立行
        let item_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clip_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            item_count, 0,
            "回滚后 clip_items 应无孤立行，实际 {item_count} 行"
        );

        // clip_images 同样无孤立行
        let img_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clip_images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            img_count, 0,
            "回滚后 clip_images 应无孤立行，实际 {img_count} 行"
        );
    }

    /// T4：孤立行领养路径 — clip_item_id=NULL 的孤立 clip_images 行应被新 item 领养。
    ///
    /// 步骤：正常入库一张图 → 手动置 clip_item_id=NULL → 再次调用 ingest_image_as_clip
    /// 同一 PNG → 断言：clip_images 仍 1 行（图片数据复用），clip_item_id 已补写为新 item_id，
    /// clip_items 新增 1 行（kind='image'），返回 Inserted。
    #[test]
    fn ingest_image_as_clip_adopts_orphaned_image_row() {
        let conn = make_test_conn();
        let png = make_test_png(3, 3);

        // 首次入库
        let first =
            ingest_image_as_clip(&conn, 3, 3, &png, 20 * 1024 * 1024).expect("首次入库失败");
        let first_item_id = match first {
            IngestOutcome::Inserted(id) => id,
            _ => panic!("首次应 Inserted"),
        };

        // 手动制造孤立行：把 clip_item_id 置 NULL
        conn.execute(
            "UPDATE clip_images SET clip_item_id = NULL WHERE clip_item_id = ?1",
            rusqlite::params![first_item_id],
        )
        .expect("置空 clip_item_id 失败");

        // 验证孤立行已存在
        let orphan_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_images WHERE clip_item_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphan_count, 1, "应有 1 条孤立行");

        // 二次调用同一 PNG — 应走 insert_image_clip 领养孤立行
        let second =
            ingest_image_as_clip(&conn, 3, 3, &png, 20 * 1024 * 1024).expect("二次调用失败");
        let second_item_id = match second {
            IngestOutcome::Inserted(id) => id,
            IngestOutcome::Bumped(_) => panic!("孤立行路径应返回 Inserted，不应 Bumped"),
        };

        // clip_images 仍 1 行（图片数据未重复存储）
        let img_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clip_images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(img_count, 1, "clip_images 应仍 1 行，图片数据不重复存储");

        // clip_item_id 已补写为新 item_id，非 NULL
        let linked_id: String = conn
            .query_row("SELECT clip_item_id FROM clip_images", [], |r| r.get(0))
            .expect("clip_item_id 不应为 NULL");
        assert_eq!(
            linked_id, second_item_id,
            "clip_images.clip_item_id 应指向新建 item"
        );

        // clip_items 现有 2 行（首次 + 孤立后新建）
        let item_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clip_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(item_count, 2, "clip_items 应有 2 行（首次 + 领养新建）");

        // 新建 item kind='image'
        let kind: String = conn
            .query_row(
                "SELECT kind FROM clip_items WHERE id = ?1",
                rusqlite::params![second_item_id],
                |r| r.get(0),
            )
            .expect("新建 clip_items 行应存在");
        assert_eq!(kind, "image", "新建 item kind 应为 'image'");
    }

    #[test]
    fn hex_encode_32_bytes_produces_64_chars() {
        let key = [0xabu8; 32];
        let hex = hex_encode(&key);
        assert_eq!(hex.len(), 64, "32 字节应编码为 64 个十六进制字符");
        assert!(
            hex.chars().all(|c| c.is_ascii_hexdigit()),
            "应全为十六进制字符"
        );
    }

    #[test]
    fn hex_encode_known_value() {
        let key = [0x07u8; 32];
        let hex = hex_encode(&key);
        assert_eq!(&hex[..2], "07", "0x07 应编码为 '07'");
    }

    /// 小阈值下入库超阈值图片，original_present 应为 0、BLOB 应为空。
    #[test]
    fn ingest_image_as_clip_respects_small_threshold() {
        let conn = make_test_conn();
        let png = make_test_png(2, 2);
        // 阈值设为 1 字节，任何图片都会超阈值
        let outcome = ingest_image_as_clip(&conn, 2, 2, &png, 1).expect("小阈值入库不应 Err");
        let item_id = match outcome {
            IngestOutcome::Inserted(id) => id,
            IngestOutcome::Bumped(_) => panic!("首次入库应返回 Inserted"),
        };

        let (original_present, original_blob): (i32, Vec<u8>) = conn
            .query_row(
                "SELECT original_present, original FROM clip_images WHERE clip_item_id = ?1",
                rusqlite::params![item_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("clip_images 行应存在");

        assert_eq!(original_present, 0, "超阈值图片 original_present 应为 0");
        assert!(original_blob.is_empty(), "超阈值图片原图 BLOB 应为空");
    }

    /// 大阈值下入库同一图片，original_present 应为 1、BLOB 非空。
    #[test]
    fn ingest_image_as_clip_respects_large_threshold() {
        let conn = make_test_conn();
        let png = make_test_png(2, 2);
        // 阈值设为 100MiB，任何测试图片都不超阈值
        let outcome =
            ingest_image_as_clip(&conn, 2, 2, &png, 100 * 1024 * 1024).expect("大阈值入库不应 Err");
        let item_id = match outcome {
            IngestOutcome::Inserted(id) => id,
            IngestOutcome::Bumped(_) => panic!("首次入库应返回 Inserted"),
        };

        let (original_present, original_blob): (i32, Vec<u8>) = conn
            .query_row(
                "SELECT original_present, original FROM clip_images WHERE clip_item_id = ?1",
                rusqlite::params![item_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("clip_images 行应存在");

        assert_eq!(original_present, 1, "未超阈值图片 original_present 应为 1");
        assert!(!original_blob.is_empty(), "未超阈值图片原图 BLOB 应非空");
    }

    /// 富文本 ingest→查询 roundtrip：带 html 的 CapturedItem 入库后，
    /// 通过 list_items_full 读回的 html_content 与原值一致、kind='richtext'。
    #[test]
    fn ingest_richtext_roundtrip_persists_html_and_kind() {
        let conn = make_test_conn();
        let html = "<b>hello</b> <i>world</i>".to_string();
        let item = CapturedItem {
            text: "hello world".to_string(),
            html: Some(html.clone()),
        };

        let outcome = ingest(&conn, &item).expect("富文本 ingest 不应 Err");
        let id = match outcome {
            IngestOutcome::Inserted(id) => id,
            IngestOutcome::Bumped(_) => panic!("首次入库应 Inserted"),
        };

        let rows = list_items_full(&conn).expect("list_items_full 不应 Err");
        let row = rows
            .iter()
            .find(|r| r.id == id)
            .expect("应能查到刚入库的行");
        assert_eq!(row.kind, "richtext", "带 html 的条目 kind 应为 richtext");
        assert_eq!(
            row.html_content.as_deref(),
            Some(html.as_str()),
            "读回的 html_content 应与入库值一致"
        );
    }

    /// 纯文本去重语义不变：同纯文本（无 html）连续 ingest 两次仍只一行（第二次 Bumped）。
    #[test]
    fn dedup_by_plaintext_unchanged() {
        let conn = make_test_conn();
        let item = CapturedItem {
            text: "same text".to_string(),
            html: None,
        };

        let first = ingest(&conn, &item).expect("首次 ingest 失败");
        let first_id = match first {
            IngestOutcome::Inserted(id) => id,
            IngestOutcome::Bumped(_) => panic!("首次应 Inserted"),
        };

        let second = ingest(&conn, &item).expect("二次 ingest 失败");
        match second {
            IngestOutcome::Bumped(id) => assert_eq!(id, first_id, "二次应 bump 同一行"),
            IngestOutcome::Inserted(_) => panic!("相同纯文本二次应 Bumped 而非新增"),
        }

        let count = count_live(&conn).expect("count_live 失败");
        assert_eq!(count, 1, "相同纯文本去重后应仅 1 行");
    }

    /// 命中补写：先 ingest 纯文本行（html=None,kind=text），再 ingest 同 text 带 html 的，
    /// 同一行被补写 html_content 且 kind 升为 richtext，不新增行。
    #[test]
    fn ingest_backfills_html_and_upgrades_kind_on_hit() {
        let conn = make_test_conn();
        let plain = CapturedItem {
            text: "rich later".to_string(),
            html: None,
        };
        let first = ingest(&conn, &plain).expect("纯文本 ingest 失败");
        let id = match first {
            IngestOutcome::Inserted(id) => id,
            IngestOutcome::Bumped(_) => panic!("首次应 Inserted"),
        };

        let rich = CapturedItem {
            text: "rich later".to_string(),
            html: Some("<u>rich later</u>".to_string()),
        };
        let second = ingest(&conn, &rich).expect("带 html 的二次 ingest 失败");
        match second {
            IngestOutcome::Bumped(bumped_id) => {
                assert_eq!(bumped_id, id, "应补写并 bump 同一行")
            }
            IngestOutcome::Inserted(_) => panic!("同纯文本带 html 应补写已有行，不新增"),
        }

        assert_eq!(
            count_live(&conn).expect("count_live 失败"),
            1,
            "应仍仅 1 行"
        );

        let rows = list_items_full(&conn).expect("list_items_full 失败");
        let row = rows.iter().find(|r| r.id == id).expect("应查到该行");
        assert_eq!(row.kind, "richtext", "补写后 kind 应升为 richtext");
        assert_eq!(
            row.html_content.as_deref(),
            Some("<u>rich later</u>"),
            "补写后 html_content 应为新值"
        );
    }

    // ===== schema 版本与迁移 =====

    /// 版本 0 时代（框架化之前）的 `clip_items` DDL：没有 `html_content` 列。
    ///
    /// 必须手抄旧 DDL、**不能**复用 `create_tables`：后者是版本 1 形态（已含
    /// `html_content`），拿它当基线就测不出 `reconcile_html_column` 那一步。
    const PRE_FRAMEWORK_CLIP_ITEMS_DDL: &str = "CREATE TABLE clip_items (
                 id                TEXT PRIMARY KEY NOT NULL,
                 content           TEXT,
                 kind              TEXT NOT NULL DEFAULT 'text',
                 created_utc       INTEGER NOT NULL,
                 last_modified_utc INTEGER NOT NULL,
                 is_deleted        INTEGER NOT NULL DEFAULT 0,
                 deleted_at_utc    INTEGER,
                 text_hash         TEXT,
                 is_favorite       INTEGER NOT NULL DEFAULT 0
             );";

    /// 内存版版本 0 旧库（`user_version` 默认 0）。
    fn make_pre_framework_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        conn.execute_batch(PRE_FRAMEWORK_CLIP_ITEMS_DDL)
            .expect("建版本 0 旧表失败");
        conn
    }

    /// 表是否存在（查 `sqlite_master`）。
    fn table_exists(conn: &rusqlite::Connection, table: &str) -> bool {
        conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            rusqlite::params![table],
            |row| row.get::<_, i64>(0),
        )
        .expect("查 sqlite_master 失败")
            > 0
    }

    /// 取某表的列名清单（`PRAGMA table_info` 第 1 列）。
    fn column_names(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("PRAGMA table_info 准备失败");
        stmt.query_map([], |row| row.get::<_, String>(1))
            .expect("query_map 失败")
            .filter_map(|r| r.ok())
            .collect()
    }

    /// 计数器：只被本模块的迁移用例累加。
    static MIGRATION_RUNS: AtomicUsize = AtomicUsize::new(0);

    /// 串行化那些共用 `MIGRATION_RUNS` 的用例：cargo test 默认多线程并行，
    /// 不加锁时彼此的计数会窜到一起（真实风险：断言偶发变成 +2）。
    static MIGRATION_COUNTER_LOCK: Mutex<()> = Mutex::new(());

    /// 取迁移计数锁。容忍 poisoning：某个用例失败不该把它后面的用例一起拖红。
    fn lock_migration_counter() -> MutexGuard<'static, ()> {
        MIGRATION_COUNTER_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 计数用迁移体：被执行一次就让 `MIGRATION_RUNS` +1，并留下可观察的 DDL 痕迹。
    fn counting_migration(tx: &Transaction<'_>) -> Result<(), DbError> {
        MIGRATION_RUNS.fetch_add(1, Ordering::SeqCst);
        tx.execute_batch("CREATE TABLE migration_probe_1 (id INTEGER PRIMARY KEY);")?;
        Ok(())
    }

    /// 第二个计数用迁移体（供「跳过已应用步骤」用例的第 2 步使用，建不同的表）。
    fn counting_migration_next(tx: &Transaction<'_>) -> Result<(), DbError> {
        MIGRATION_RUNS.fetch_add(1, Ordering::SeqCst);
        tx.execute_batch("CREATE TABLE migration_probe_2 (id INTEGER PRIMARY KEY);")?;
        Ok(())
    }

    /// 测试专用：一步即可升到版本 1 的计数迁移列表（不污染生产 `MIGRATIONS`）。
    const COUNTER_MIGRATIONS: &[Migration] = &[Migration {
        to_version: 1,
        apply: counting_migration,
    }];

    /// 第一步成功建表，第二步必然失败——用于验证整笔事务回滚。
    fn create_probe_table(tx: &Transaction<'_>) -> Result<(), DbError> {
        tx.execute_batch("CREATE TABLE migration_probe (id INTEGER PRIMARY KEY);")?;
        Ok(())
    }

    /// 必然失败的迁移体（模拟某条 DDL 在用户库上挂掉）。
    fn always_failing_migration(_tx: &Transaction<'_>) -> Result<(), DbError> {
        Err(DbError::Other("迁移步骤故意失败".to_string()))
    }

    /// 测试专用：两步骤，第二步失败，目标版本 2。
    const ROLLBACK_MIGRATIONS: &[Migration] = &[
        Migration {
            to_version: 1,
            apply: create_probe_table,
        },
        Migration {
            to_version: 2,
            apply: always_failing_migration,
        },
    ];

    /// 新库：`ensure_schema` 一次建到当前版本，版本号 / 表 / 索引 / 列齐全。
    #[test]
    fn ensure_schema_builds_fresh_db_to_current_version() {
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        assert_eq!(
            read_schema_version(&conn).expect("读版本失败"),
            0,
            "前置条件：新建的库版本号应为 0"
        );

        ensure_schema(&conn).expect("新库初始化失败");

        assert_eq!(
            read_schema_version(&conn).expect("读版本失败"),
            CURRENT_SCHEMA_VERSION,
            "新库应一次建到当前 schema 版本"
        );
        for table in [
            "clip_items",
            "clip_images",
            "provider_config",
            "provider_secret",
            "translation_cache",
            "translate_history",
        ] {
            assert!(table_exists(&conn, table), "新库应含 {table} 表");
        }
        assert_eq!(
            index_names(&conn),
            EXPECTED_INDEX_NAMES,
            "新库应恰好建出约定的 5 条索引"
        );
        assert!(
            has_column(&conn, "clip_items", "html_content").expect("查列失败"),
            "新库 clip_items 应含 html_content 列"
        );
    }

    /// 旧库升级：无 `html_content`、`user_version = 0`、已有 2 条数据。
    ///
    /// 升级后必须：版本到达当前版本、`html_content` 可写读、既有数据与去重键完好、
    /// 索引补齐。这条同时钉住「版本 1 的形态探测不会把老库 ALTER 出错」。
    #[test]
    fn ensure_schema_upgrades_pre_framework_db_keeping_rows() {
        // Arrange：版本 0 旧库 + 两条数据
        let conn = make_pre_framework_conn();
        conn.execute_batch(
            "INSERT INTO clip_items
                 (id, content, kind, created_utc, last_modified_utc, is_deleted, text_hash)
             VALUES ('old-1', '旧条目一', 'text', 1700000000000, 1700000000000, 0, 'h-old-1'),
                    ('old-2', '旧条目二', 'text', 1700000000001, 1700000000001, 0, 'h-old-2');",
        )
        .expect("写入旧数据失败");
        assert_eq!(
            read_schema_version(&conn).expect("读版本失败"),
            0,
            "前置条件：旧库版本号为 0"
        );
        assert!(
            !has_column(&conn, "clip_items", "html_content").expect("查列失败"),
            "前置条件：旧库无 html_content 列"
        );

        // Act
        ensure_schema(&conn).expect("旧库升级失败");

        // Assert：版本号、列、数据、去重键、索引
        assert_eq!(
            read_schema_version(&conn).expect("读版本失败"),
            CURRENT_SCHEMA_VERSION,
            "升级后版本号应到达当前版本"
        );
        assert!(
            has_column(&conn, "clip_items", "html_content").expect("查列失败"),
            "升级后应补上 html_content 列"
        );

        let rows = list_items_with_images(&conn).expect("升级后列表查询失败");
        assert_eq!(rows.len(), 2, "升级不得丢失既有条目");
        let mut contents: Vec<&str> = rows.iter().map(|r| r.content.as_str()).collect();
        contents.sort_unstable();
        assert_eq!(
            contents,
            vec!["旧条目一", "旧条目二"],
            "升级后既有条目的内容应保持原值"
        );

        assert!(
            has_live_text_item(&conn, "h-old-1").expect("去重查询失败"),
            "升级后原条目的去重键仍应命中，否则同一内容会被重复入库"
        );
        assert!(
            has_live_text_item(&conn, "h-old-2").expect("去重查询失败"),
            "升级后第二条的去重键同样应命中"
        );

        // 新列在旧行上可用：写入再读回
        conn.execute(
            "UPDATE clip_items SET html_content = '<b>旧条目一</b>' WHERE id = 'old-1'",
            [],
        )
        .expect("升级后应可写 html_content");
        let read: Option<String> = conn
            .query_row(
                "SELECT html_content FROM clip_items WHERE id = 'old-1'",
                [],
                |r| r.get(0),
            )
            .expect("升级后应可读 html_content");
        assert_eq!(
            read.as_deref(),
            Some("<b>旧条目一</b>"),
            "旧行上的 html_content 应可写可读"
        );

        assert_eq!(
            index_names(&conn),
            EXPECTED_INDEX_NAMES,
            "升级后索引应补齐到约定的 5 条"
        );
    }

    /// 加密文件库上的旧库升级：端到端走生产开库路径（SQLCipher 真库 + 真密钥）。
    ///
    /// 内存库用例证明不了两件事：`PRAGMA user_version` 在加密库上是否同样随事务落盘，
    /// 以及 ALTER 走真实文件库 + SQLCipher 时是否有差异。这条把两者一起钉住。
    #[test]
    fn ensure_schema_upgrades_encrypted_file_db_end_to_end() {
        let dir = tempfile::tempdir().expect("临时目录创建失败");
        let path = dir.path().join("legacy.db");
        let key = [7u8; 32];

        // Arrange：用真实密钥手工造一个版本 0 的旧库（无 html_content 列）+ 一条数据
        {
            let legacy = open_with_key(&path, &key).expect("开旧库失败");
            legacy
                .execute_batch(&format!(
                    "{PRE_FRAMEWORK_CLIP_ITEMS_DDL}
                     INSERT INTO clip_items
                         (id, content, kind, created_utc, last_modified_utc, is_deleted, text_hash)
                     VALUES ('legacy-1', '加密库旧条目', 'text', 1700000000000, 1700000000000, 0, 'h-legacy-1');"
                ))
                .expect("造版本 0 加密旧库失败");
            assert_eq!(
                read_schema_version(&legacy).expect("读版本失败"),
                0,
                "前置条件：加密旧库版本号为 0"
            );
        }

        // Act：走生产开库路径
        let conn = open_or_create(&path, &key).expect("加密旧库升级失败");

        // Assert
        assert_eq!(
            read_schema_version(&conn).expect("读版本失败"),
            CURRENT_SCHEMA_VERSION,
            "加密旧库升级后版本号应到达当前版本"
        );
        assert!(
            has_column(&conn, "clip_items", "html_content").expect("查列失败"),
            "加密旧库升级后应补上 html_content 列"
        );
        assert_eq!(
            count_live(&conn).expect("计数失败"),
            1,
            "升级不得丢失既有条目"
        );
        assert!(
            has_live_text_item(&conn, "h-legacy-1").expect("去重查询失败"),
            "加密旧库升级后去重键仍应命中"
        );
        assert_eq!(
            index_names(&conn),
            EXPECTED_INDEX_NAMES,
            "加密旧库升级后索引应补齐"
        );

        // 再开一次：版本不变（幂等落在真实文件上）
        drop(conn);
        let again = open_or_create(&path, &key).expect("重复打开失败");
        assert_eq!(
            read_schema_version(&again).expect("读版本失败"),
            CURRENT_SCHEMA_VERSION,
            "重复打开不应改变版本号"
        );
    }

    /// 版本 1 → 版本 2 的真实文件库升级：加密库 + 真密钥 + 既有数据。
    ///
    /// 这条把「真实形态的旧库」钉住：先用生产迁移链只升到版本 1（等价于上一版 QuickQuick
    /// 建出来的库），写入数据；再用当前代码 `open_or_create` 打开（= 生产升级路径）。
    ///
    /// 二次打开本身就是「迁移没有被重放」的证据：`ALTER TABLE ADD COLUMN title` 重放会
    /// 报 `duplicate column name`，能打开就说明版本号拦住了它。
    #[test]
    fn ensure_schema_upgrades_v1_file_db_to_v2_keeping_rows() {
        let dir = tempfile::tempdir().expect("临时目录创建失败");
        let path = dir.path().join("v1.db");
        let key = [7u8; 32];

        // Arrange：只升到版本 1，并写入两条数据
        {
            let v1 = open_with_key(&path, &key).expect("开 v1 库失败");
            apply_migrations(&v1, 1, &MIGRATIONS[..1]).expect("建到版本 1 失败");
            assert_eq!(
                read_schema_version(&v1).expect("读版本失败"),
                1,
                "前置条件：库应停在版本 1"
            );
            assert!(
                !has_column(&v1, "clip_items", "title").expect("查列失败"),
                "前置条件：版本 1 的库还没有 title 列"
            );
            v1.execute_batch(
                "INSERT INTO clip_items
                     (id, content, kind, created_utc, last_modified_utc, is_deleted, text_hash)
                 VALUES ('v1-a', '版本一的条目', 'text', 1700000000000, 1700000000000, 0, 'h-v1-a'),
                        ('v1-b', '版本一的第二条', 'text', 1700000000001, 1700000000001, 0, 'h-v1-b');",
            )
            .expect("写入 v1 数据失败");
        }

        // Act：用当前代码打开（生产升级路径）
        let conn = open_or_create(&path, &key).expect("v1→v2 升级失败");

        // Assert 1：版本、列、数据
        assert_eq!(
            read_schema_version(&conn).expect("读版本失败"),
            CURRENT_SCHEMA_VERSION,
            "升级后版本号应到达当前版本"
        );
        assert_eq!(CURRENT_SCHEMA_VERSION, 2, "本项把 schema 版本推进到 2");
        assert!(
            has_column(&conn, "clip_items", "title").expect("查列失败"),
            "升级后应补上 title 列"
        );
        assert_eq!(count_live(&conn).expect("计数失败"), 2, "升级不得丢数据");
        assert!(
            has_live_text_item(&conn, "h-v1-a").expect("去重查询失败")
                && has_live_text_item(&conn, "h-v1-b").expect("去重查询失败"),
            "升级后既有条目的去重键必须仍然命中"
        );

        // Assert 2：既有行的 title 是 NULL（= 尚未回填），迁移不在这里做全表 UPDATE
        let pending: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_items WHERE title IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("统计待回填失败");
        assert_eq!(pending, 2, "迁移只加列，既有行的 title 应留待回填");

        // Assert 3：二次打开不重放迁移（重放会在 ADD COLUMN 上直接报错）
        drop(conn);
        let again = open_or_create(&path, &key).expect("二次打开应成功（迁移不得重放）");
        assert_eq!(
            read_schema_version(&again).expect("读版本失败"),
            CURRENT_SCHEMA_VERSION,
            "二次打开不应改变版本号"
        );
        assert_eq!(
            column_names(&again, "clip_items")
                .iter()
                .filter(|c| c.as_str() == "title")
                .count(),
            1,
            "title 列应恰好存在一次"
        );
        assert_eq!(
            count_live(&again).expect("计数失败"),
            2,
            "二次打开不得丢数据"
        );

        // Assert 4：在升级后的库上回填一次，两条老行的 title 都补上
        assert_eq!(
            backfill_titles_batch(&again, TITLE_BACKFILL_BATCH_SIZE).expect("回填失败"),
            2,
            "回填应补齐两条升级前的老行"
        );
        let filled: Vec<String> = {
            let mut stmt = again
                .prepare("SELECT title FROM clip_items ORDER BY id")
                .expect("准备查询失败");
            stmt.query_map([], |row| row.get::<_, Option<String>>(0))
                .expect("查询失败")
                .filter_map(|r| r.ok().flatten())
                .collect()
        };
        assert_eq!(
            filled,
            vec!["版本一的条目".to_string(), "版本一的第二条".to_string()],
            "回填后的 title 应等于各自正文（两条都是单行短文本）"
        );
    }

    /// `compute_title` 的边界：全空白 / 空串都得到空串（不是空白串）。
    #[test]
    fn compute_title_collapses_blank_input_to_empty() {
        assert_eq!(compute_title("text", ""), "", "空正文的 title 应为空串");
        assert_eq!(
            compute_title("text", " \n\t\r\n "),
            "",
            "纯空白正文的 title 应为空串，不留首尾空白"
        );
        assert_eq!(
            compute_title("richtext", "富文本\n正文"),
            "富文本 正文",
            "富文本条目与纯文本同口径（kind 只区分图片）"
        );
        assert_eq!(
            compute_title(KIND_IMAGE, "[图片] 10×10"),
            "",
            "图片条目 title 固定为空串"
        );
    }

    /// 幂等：重复 `ensure_schema` 不改版本、不重复执行迁移体。
    ///
    /// 两个层面都验：生产路径（版本号 + 列只出现一次）与计数路径（迁移体只跑一次）。
    #[test]
    fn repeated_ensure_schema_does_not_rerun_migrations() {
        let _counter_guard = lock_migration_counter();
        // Arrange：生产路径先建到最新
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        ensure_schema(&conn).expect("首次初始化失败");
        let version_after_first = read_schema_version(&conn).expect("读版本失败");
        let columns_after_first = column_names(&conn, "clip_items");

        // Act：再跑一次
        ensure_schema(&conn).expect("重复初始化不应报错");

        // Assert：版本与列集合都不变，html 列只出现一次
        assert_eq!(
            read_schema_version(&conn).expect("读版本失败"),
            version_after_first,
            "重复开库不应改变 schema 版本"
        );
        assert_eq!(
            version_after_first, CURRENT_SCHEMA_VERSION,
            "首次初始化后版本应为当前版本"
        );
        assert_eq!(
            column_names(&conn, "clip_items"),
            columns_after_first,
            "重复开库不应再改表结构"
        );
        assert_eq!(
            columns_after_first
                .iter()
                .filter(|c| c.as_str() == "html_content")
                .count(),
            1,
            "html_content 列应恰好存在一次，实际列: {columns_after_first:?}"
        );

        // 计数路径：同一目标版本第二次调用不再执行迁移体
        let counted = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        let before = MIGRATION_RUNS.load(Ordering::SeqCst);
        apply_migrations(&counted, 1, COUNTER_MIGRATIONS).expect("首次迁移失败");
        let after_first = MIGRATION_RUNS.load(Ordering::SeqCst);
        apply_migrations(&counted, 1, COUNTER_MIGRATIONS).expect("重复迁移不应报错");
        let after_second = MIGRATION_RUNS.load(Ordering::SeqCst);

        assert_eq!(after_first, before + 1, "首次调用应执行 1 次迁移体");
        assert_eq!(
            after_second, after_first,
            "版本已是最新时不应再执行迁移体（否则每次开库都重放一遍 DDL）"
        );
        assert_eq!(
            read_schema_version(&counted).expect("读版本失败"),
            1,
            "计数路径的版本号应停在目标版本"
        );
    }

    /// 「已应用的步骤必须跳过」：库已在版本 1 时升到版本 2，只能跑第 2 步。
    ///
    /// 这条单独存在，是因为上面的计数用例有两条互相兜底的路径（目标已达成时提前返回、
    /// 循环里跳过已应用步骤），只拆掉其中一条它仍然绿。这里用「列表里含一条已应用步骤 +
    /// 一条新步骤」，把「跳过已应用步骤」单独钉住：一旦它失效，第 1 步会被重新执行，
    /// 版本号从 1 跳到 1 的不连续会被拒绝（或计数变成 2）。
    #[test]
    fn already_applied_steps_are_skipped_during_upgrade() {
        let _counter_guard = lock_migration_counter();
        // Arrange：先把库建到版本 1
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        apply_migrations(&conn, 1, COUNTER_MIGRATIONS).expect("首步迁移失败");
        let before = MIGRATION_RUNS.load(Ordering::SeqCst);

        // Act：目标版本 2，列表里第 1 步是已应用的同一步
        const TWO_STEPS: &[Migration] = &[
            Migration {
                to_version: 1,
                apply: counting_migration,
            },
            Migration {
                to_version: 2,
                apply: counting_migration_next,
            },
        ];
        apply_migrations(&conn, 2, TWO_STEPS).expect("升到版本 2 不应失败");

        // Assert：只执行了第 2 步
        assert_eq!(
            MIGRATION_RUNS.load(Ordering::SeqCst),
            before + 1,
            "库已在版本 1 时只应执行第 2 步，已应用的步骤必须跳过"
        );
        assert_eq!(
            read_schema_version(&conn).expect("读版本失败"),
            2,
            "升级后版本号应为 2"
        );
    }

    /// 目标版本已达成时一步都不跑：即便传入的列表里带着「未来」的步骤。
    ///
    /// 单独钉住 `apply_migrations` 的提前返回——它是「重复开库不做任何 schema 动作」
    /// 这条契约的唯一实现点（循环里的跳过条件在这里不生效，因为版本号已经等于目标）。
    #[test]
    fn reached_target_version_runs_no_step_at_all() {
        let _counter_guard = lock_migration_counter();
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        apply_migrations(&conn, 1, COUNTER_MIGRATIONS).expect("首步迁移失败");
        let before = MIGRATION_RUNS.load(Ordering::SeqCst);

        // 目标版本 1 已达成；列表里的版本 2 步骤属于未来调用，不得在本次执行
        const FUTURE_STEP: &[Migration] = &[Migration {
            to_version: 2,
            apply: always_failing_migration,
        }];
        apply_migrations(&conn, 1, FUTURE_STEP)
            .expect("目标版本已达成时不应执行任何步骤（包括未来步骤）");

        assert_eq!(
            MIGRATION_RUNS.load(Ordering::SeqCst),
            before,
            "目标版本已达成时不应执行任何迁移体"
        );
        assert_eq!(
            read_schema_version(&conn).expect("读版本失败"),
            1,
            "目标版本已达成时版本号不应变化"
        );
    }

    /// 前沿保护：库里版本高于本程序支持时明确报错，且报错前不得先动库。
    #[test]
    fn ensure_schema_rejects_future_schema_version() {
        // Arrange：把版本号设成「比本程序新」
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
            .expect("写版本号失败");

        // Act
        let err = ensure_schema(&conn).expect_err("旧程序打开新库必须报错");

        // Assert
        match err {
            DbError::SchemaTooNew { found, supported } => {
                assert_eq!(
                    found,
                    CURRENT_SCHEMA_VERSION + 1,
                    "报错应带上库里的实际版本"
                );
                assert_eq!(
                    supported, CURRENT_SCHEMA_VERSION,
                    "报错应带上本程序支持的版本"
                );
            }
            other => panic!("应报 SchemaTooNew 而不是其它错误，实际: {other}"),
        }
        assert!(
            !table_exists(&conn, "clip_items"),
            "版本超前时必须在任何 DDL 之前就报错，不能先建表再失败"
        );
    }

    /// 迁移事务性：第二步失败 → 版本号与第一步的建表一起回滚。
    #[test]
    fn failed_migration_step_rolls_back_version_and_ddl() {
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");

        let err = apply_migrations(&conn, 2, ROLLBACK_MIGRATIONS).expect_err("第二步应失败");

        match err {
            DbError::MigrationFailed {
                to_version,
                from_version,
                reason,
            } => {
                assert_eq!(to_version, 2, "报错应指向失败的那一步");
                assert_eq!(from_version, 0, "报错应带上迁移前的版本");
                assert!(
                    reason.contains("故意失败"),
                    "底层失败原因应透传，实际: {reason}"
                );
            }
            other => panic!("应报 MigrationFailed 而不是其它错误，实际: {other}"),
        }
        assert_eq!(
            read_schema_version(&conn).expect("读版本失败"),
            0,
            "回滚后版本号应停在迁移前，不能留下「表改了但版本没写」的半成品库"
        );
        assert!(
            !table_exists(&conn, "migration_probe"),
            "第一步建的表也必须随事务回滚掉"
        );
    }

    /// 迁移框架自身的一致性护栏：跳号、或列表短于目标版本，都必须硬失败且不留下痕迹。
    ///
    /// 这两种情况都是「改代码的人只做了一半」：加步骤忘了 bump 常量、或 bump 了常量忘了
    /// 加步骤。必须在开库时就硬失败，而不是把库升到一半。
    #[test]
    fn apply_migrations_rejects_inconsistent_step_lists() {
        // 跳号：库在版本 0，列表里第一个步骤直接是版本 2
        let jump = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        let err = apply_migrations(
            &jump,
            2,
            &[Migration {
                to_version: 2,
                apply: create_probe_table,
            }],
        )
        .expect_err("跳号必须报错");
        assert!(
            matches!(err, DbError::Other(_)),
            "跳号应报 Other（迁移列表本身不自洽），实际: {err}"
        );
        assert_eq!(
            read_schema_version(&jump).expect("读版本失败"),
            0,
            "跳号失败后版本号不应变化"
        );
        assert!(
            !table_exists(&jump, "migration_probe"),
            "跳号失败不应留下任何 DDL 痕迹"
        );

        // 列表短于目标版本：常量被 bump 到 2，但最高步骤仍是 1
        let short = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        let err = apply_migrations(
            &short,
            2,
            &[Migration {
                to_version: 1,
                apply: create_probe_table,
            }],
        )
        .expect_err("列表短于目标版本必须报错");
        assert!(
            matches!(err, DbError::Other(_)),
            "应报 Other（步骤缺失），实际: {err}"
        );
        assert_eq!(
            read_schema_version(&short).expect("读版本失败"),
            0,
            "失败后版本号应回滚到迁移前"
        );
        assert!(
            !table_exists(&short, "migration_probe"),
            "已经执行过的步骤也必须随事务回滚掉"
        );
    }

    /// 启动期两类 schema 错误的用户可见文案：必须各自说清真实原因，
    /// 且**不得**退化成解密失败话术（「请检查钥匙串授权」）。
    ///
    /// 启动期 `eprintln` 与传回前端的字符串都是这条 `Display` 的产物，文案说错就等于
    /// 把用户往「钥匙串/重启」的死路上引；这条用例把它钉死。
    #[test]
    fn schema_errors_explain_their_own_cause() {
        // 版本超前：说清「库比程序新」，并给出两个版本号
        let too_new = DbError::SchemaTooNew {
            found: CURRENT_SCHEMA_VERSION + 2,
            supported: CURRENT_SCHEMA_VERSION,
        };
        let too_new_msg = too_new.to_string();
        assert!(
            too_new_msg.contains("更新版本的 QuickQuick"),
            "版本超前应点明是库比程序新，实际: {too_new_msg}"
        );
        assert!(
            too_new_msg.contains(&format!(
                "v{} > v{}",
                CURRENT_SCHEMA_VERSION + 2,
                CURRENT_SCHEMA_VERSION
            )),
            "版本超前应带上库里版本与支持版本，实际: {too_new_msg}"
        );
        assert!(
            too_new_msg.contains("请更新应用"),
            "版本超前的正确出口是更新应用，实际: {too_new_msg}"
        );
        assert!(
            !too_new_msg.contains("钥匙串"),
            "版本超前不得复用解密失败话术（会误导用户去查钥匙串），实际: {too_new_msg}"
        );

        // 迁移失败：说清 from→to、透出底层原因、并告知原库未改
        let migrated = DbError::MigrationFailed {
            to_version: 2,
            from_version: 1,
            reason: "no such column: title".to_string(),
        };
        let migrated_msg = migrated.to_string();
        assert!(
            migrated_msg.contains("数据库升级失败"),
            "迁移失败应点明是升级失败，实际: {migrated_msg}"
        );
        assert!(
            migrated_msg.contains("1→2"),
            "迁移失败应带上 from→to 版本，实际: {migrated_msg}"
        );
        assert!(
            migrated_msg.contains("no such column: title"),
            "迁移失败应透出底层原因，实际: {migrated_msg}"
        );
        assert!(
            migrated_msg.contains("原库未被修改"),
            "迁移失败应告知数据未被改动（与事务回滚一致），实际: {migrated_msg}"
        );
        assert!(
            !migrated_msg.contains("钥匙串"),
            "迁移失败不得复用解密失败话术，实际: {migrated_msg}"
        );
    }

    /// 迁移列表与版本常量必须同步：从 1 连续递增到 `CURRENT_SCHEMA_VERSION`。
    ///
    /// 这条会在「加了步骤忘了 bump 常量」或「bump 了常量忘了加步骤」时直接红，
    /// 是模块头那条硬约束的机器执行版本。
    #[test]
    fn migration_list_is_contiguous_up_to_current_version() {
        let versions: Vec<i32> = MIGRATIONS.iter().map(|m| m.to_version).collect();
        let expected: Vec<i32> = (1..=CURRENT_SCHEMA_VERSION).collect();
        assert_eq!(
            versions, expected,
            "迁移步骤必须从 1 连续递增到 CURRENT_SCHEMA_VERSION；新增步骤时要同时 bump 常量"
        );
    }

    // ===== 索引与查询计划 =====

    /// 索引相关测试断言的实际索引名清单（与 `ensure_indexes` 一一对应）。
    const EXPECTED_INDEX_NAMES: [&str; 5] = [
        "idx_clip_images_clip_item_id",
        "idx_clip_images_image_hash",
        "idx_clip_images_live_favorite_created",
        "idx_clip_items_live_favorite_modified",
        "idx_clip_items_text_hash",
    ];

    /// 建立「只有表、没有任何业务索引」的内存库：复现建索引之前的状态。
    ///
    /// 直接复用生产建表语句（`create_tables`）+ 后续版本的增量列迁移体，
    /// 保证基线表结构与线上完全一致，不另外抄一份 DDL 造成漂移。
    /// 版本仍停在 0：索引类用例要的正是「已升级过 schema 但还没建索引」这个中间态，
    /// 写版本号会让后续 `ensure_schema` 直接短路、一条索引都不建。
    fn make_schema_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        create_tables(&conn).expect("建测试表失败");
        // 版本 2 起的增量列：与生产迁移步骤共用同一句 DDL（不是手抄列定义）。
        add_title_column(&conn).expect("补版本 2 的 title 列失败");
        conn
    }

    /// 建立「只到版本 1 表形态、无索引、`user_version = 0`」的内存库。
    ///
    /// 与 [`make_schema_conn`] 的差别只有「不预先补版本 2 的列」，专供那些**自己会跑
    /// `ensure_schema` 全链**的用例：表已经是 v2 形态时，版本 2 的 `ADD COLUMN` 会命中
    /// `duplicate column name`，测的就不再是「从旧库一路升上来」了。
    fn make_baseline_v1_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("内存库开启失败");
        create_tables(&conn).expect("建 v1 基线表失败");
        conn
    }

    /// 取 `EXPLAIN QUERY PLAN` 的 detail 列（第 4 列）并拼成多行文本。
    ///
    /// 断言的就是生产语句本身（由调用方传入同一常量），不另抄一份 SQL。
    /// 生产语句里的 `?n` 占位符在取计划时必须被绑定（填 NULL 即可），否则 SQLite 报参数个数不符。
    fn explain_query_plan(conn: &rusqlite::Connection, sql: &str) -> String {
        let mut stmt = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .expect("EXPLAIN QUERY PLAN 准备失败");
        let placeholders = vec![rusqlite::types::Value::Null; stmt.parameter_count()];
        let rows = stmt
            .query_map(rusqlite::params_from_iter(placeholders), |row| {
                row.get::<_, String>(3)
            })
            .expect("EXPLAIN QUERY PLAN 执行失败");
        let mut plan: Vec<String> = Vec::new();
        for row in rows {
            plan.push(row.expect("读取查询计划行失败"));
        }
        plan.join("\n")
    }

    /// 直接插入一条未软删的文本条目（构造查询计划/排序数据用）。
    fn insert_text_row(
        conn: &rusqlite::Connection,
        id: &str,
        hash: &str,
        last_modified_utc: i64,
        is_favorite: bool,
    ) {
        conn.execute(
            "INSERT INTO clip_items
                 (id, content, kind, created_utc, last_modified_utc, is_deleted, text_hash, is_favorite)
             VALUES (?1, ?2, 'text', ?3, ?3, 0, ?4, ?5)",
            rusqlite::params![
                id,
                format!("content-{id}"),
                last_modified_utc,
                hash,
                if is_favorite { 1 } else { 0 }
            ],
        )
        .expect("插入文本条目失败");
    }

    /// 列出库里 `idx_` 前缀的索引名（按名字排序）。
    fn index_names(conn: &rusqlite::Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_%' ORDER BY name")
            .expect("查询索引清单失败");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("查询索引清单失败");
        rows.map(|r| r.expect("读取索引名失败")).collect()
    }

    /// 列表主查询的计划必须「按索引定位 + 索引自带有序」，不能全表扫再排序。
    ///
    /// 同时对比建索引前后的计划：前态必须真的需要临时排序，否则这个断言就失去意义。
    #[test]
    fn list_query_plan_seeks_index_and_drops_temp_sort() {
        // Arrange：无索引库上先取一次基线计划
        let conn = make_schema_conn();
        insert_text_row(
            &conn,
            "id-old",
            "hash-old000000000",
            1_700_000_000_000,
            false,
        );
        insert_text_row(
            &conn,
            "id-new",
            "hash-new000000000",
            1_700_000_001_000,
            true,
        );
        let before = explain_query_plan(&conn, LIST_ITEMS_WITH_IMAGES_SQL);

        // Act：补齐索引后再取一次
        ensure_indexes(&conn).expect("建索引失败");
        let after = explain_query_plan(&conn, LIST_ITEMS_WITH_IMAGES_SQL);

        eprintln!("[列表主查询计划] 建索引前:\n{before}\n[列表主查询计划] 建索引后:\n{after}");

        // Assert
        assert!(
            before.contains("SCAN ci") && before.contains("TEMP B-TREE"),
            "基线（无索引）本该全表扫 + 临时排序，若这里不成立说明对比前提失效，实际计划:\n{before}"
        );
        assert!(
            !after.contains("TEMP B-TREE"),
            "建索引后 ORDER BY 必须完全由索引满足，否则条目一多就要把整表取回来排序，实际计划:\n{after}"
        );
        assert!(
            !after.contains("SCAN ci"),
            "列表查询不应再全表扫 clip_items（FROM 里别名是 ci，计划里扫表写作 `SCAN ci`，写成 `SCAN clip_items` 会变成恒真假绿），实际计划:\n{after}"
        );
        assert!(
            after.contains("idx_clip_items_live_favorite_modified (is_deleted=?)"),
            "列表查询应走 is_deleted 过滤 + is_favorite/last_modified_utc 排序的复合索引，实际计划:\n{after}"
        );
        assert!(
            after.contains("idx_clip_images_clip_item_id (clip_item_id=?)"),
            "关联图片的 LEFT JOIN 必须按 clip_item_id 走索引，否则每个条目都要扫一遍图片表，实际计划:\n{after}"
        );
        assert!(
            !after.contains("AUTOMATIC"),
            "关联图片的 LEFT JOIN 不应再依赖查询期为本次查询临时建的自动索引（那等于每次开列表都重建一遍索引），实际计划:\n{after}"
        );

        // IPC 列表走的是「只取缩略图标志」的变体：它必须与上面被钉住的 BLOB 变体同计划，
        // 否则生产路径会在没人察觉的情况下退化成全表扫 + 临时排序。
        let flag_plan = explain_query_plan(&conn, LIST_ITEMS_WITH_THUMBNAIL_FLAG_SQL);
        eprintln!("[列表主查询计划] 只取缩略图标志:\n{flag_plan}");
        assert_eq!(
            flag_plan, after,
            "只取缩略图标志的列表查询必须与 BLOB 变体走完全相同的索引计划"
        );

        // 计划串相同只证明「索引用法」一致，证明不了 WHERE/JOIN 的语义还在：
        // 两个变体由同一宏展开，删掉软删过滤或空缩略图判据时计划串逐字不变
        // （变异实证：去掉 JOIN 的 `cimg.is_deleted = 0` 后上面这条 assert_eq 依然成立）。
        // 所以对 SQL 语义本身加字面锚，条件被删时必须红。
        for (sql, needle, why) in [
            (
                LIST_ITEMS_WITH_THUMBNAIL_FLAG_SQL,
                "cimg.is_deleted = 0",
                "LEFT JOIN 必须排除已软删的图片行，否则清理掉的图会被重新 JOIN 回列表，缩略图与原图都会指向死行",
            ),
            (
                LIST_ITEMS_WITH_THUMBNAIL_FLAG_SQL,
                "length(cimg.thumbnail) > 0",
                "hasThumbnail 必须要求缩略图非空，否则前端会为永远取不到图的条目反复发请求",
            ),
            (
                LIST_ITEMS_WITH_THUMBNAIL_FLAG_SQL,
                "ci.is_deleted = 0",
                "列表必须排除已软删的父条目，否则已删除条目会重新出现在剪贴板面板里",
            ),
            (
                LIST_ITEMS_WITH_IMAGES_SQL,
                "cimg.is_deleted = 0",
                "契约测试口径的 BLOB 变体同样必须排除已软删的图片行",
            ),
        ] {
            assert!(sql.contains(needle), "{why}（SQL 中缺少 `{needle}`）");
        }
    }

    /// 文本去重查询（`ingest` 命中判定）必须按 text_hash 定位，不能扫全部未软删行。
    #[test]
    fn text_dedup_query_plan_seeks_text_hash_index() {
        let conn = make_schema_conn();
        insert_text_row(&conn, "id-a", "hash-aaaaaaaaaaaa", 1_700_000_000_000, false);
        let before = explain_query_plan(&conn, INGEST_TEXT_DEDUP_SQL);

        ensure_indexes(&conn).expect("建索引失败");
        let after = explain_query_plan(&conn, INGEST_TEXT_DEDUP_SQL);

        eprintln!("[文本去重计划] 建索引前:\n{before}\n[文本去重计划] 建索引后:\n{after}");

        assert!(
            before.contains("SCAN clip_items"),
            "基线（无索引）本该全表扫，若这里不成立说明对比前提失效，实际计划:\n{before}"
        );
        // 钉死「约束串首列是 text_hash」：只建单列 (text_hash) 或把列序写成
        // (is_deleted, text_hash) 时，本断言会红——前者会让规划器在无统计的库上改走
        // is_deleted 前缀索引（见下一条断言），后者已偏离「让哈希等值落在最左前缀」的设计。
        assert!(
            after.contains("idx_clip_items_text_hash (text_hash=? AND is_deleted=?)"),
            "文本去重必须按 (text_hash, is_deleted) 的两列前缀定位，否则每次复制都要扫全部未软删条目，实际计划:\n{after}"
        );
        assert!(
            !after.contains("idx_clip_items_live_favorite_modified"),
            "文本去重不得退化成用 is_deleted 前缀索引遍历全部未软删行——这正是只建单列 (text_hash) 时发生的退化，实际计划:\n{after}"
        );
        assert!(
            !after.contains("SCAN clip_items"),
            "文本去重不应再全表扫 clip_items，实际计划:\n{after}"
        );
    }

    /// 图片去重查询（`ingest_image_as_clip` 命中判定）必须按 image_hash 定位。
    #[test]
    fn image_dedup_query_plan_seeks_image_hash_index() {
        let conn = make_schema_conn();
        insert_text_row(&conn, "id-a", "hash-aaaaaaaaaaaa", 1_700_000_000_000, false);
        conn.execute(
            "INSERT INTO clip_images
                 (id, clip_item_id, thumbnail, original, original_present, image_hash,
                  created_utc, last_modified_utc, is_deleted, is_favorite)
             VALUES ('img-a', 'id-a', x'00', x'00', 1, 'imghash-aaaaaaaa', 1700000000000, 1700000000000, 0, 0)",
            [],
        )
        .expect("插入图片行失败");
        let before = explain_query_plan(&conn, INGEST_IMAGE_DEDUP_SQL);

        ensure_indexes(&conn).expect("建索引失败");
        let after = explain_query_plan(&conn, INGEST_IMAGE_DEDUP_SQL);

        eprintln!("[图片去重计划] 建索引前:\n{before}\n[图片去重计划] 建索引后:\n{after}");

        assert!(
            before.contains("SCAN clip_images"),
            "基线（无索引）本该全表扫图片表，若这里不成立说明对比前提失效，实际计划:\n{before}"
        );
        assert!(
            after.contains("idx_clip_images_image_hash (image_hash=? AND is_deleted=?)"),
            "图片去重必须按 (image_hash, is_deleted) 的两列前缀定位，否则每次复制图片都要扫全部图片行，实际计划:\n{after}"
        );
        assert!(
            !after.contains("idx_clip_images_live_favorite_created"),
            "图片去重不得退化成用 is_deleted 前缀索引遍历全部图片行——这正是只建单列 (image_hash) 时会发生的退化，实际计划:\n{after}"
        );
        assert!(
            !after.contains("SCAN clip_images"),
            "图片去重不应再全表扫 clip_images，实际计划:\n{after}"
        );
    }

    /// 容量裁剪候选查询必须按索引直接拿到有序候选，不能扫全表再排序。
    #[test]
    fn cleanup_keep_recent_plan_seeks_index() {
        let conn = make_schema_conn();
        insert_text_row(&conn, "id-a", "hash-aaaaaaaaaaaa", 1_700_000_000_000, false);
        insert_text_row(&conn, "id-b", "hash-bbbbbbbbbbbb", 1_700_000_002_000, false);
        let before = explain_query_plan(&conn, CLEANUP_KEEP_RECENT_CANDIDATES_SQL);

        ensure_indexes(&conn).expect("建索引失败");
        let after = explain_query_plan(&conn, CLEANUP_KEEP_RECENT_CANDIDATES_SQL);

        eprintln!("[容量裁剪计划] 建索引前:\n{before}\n[容量裁剪计划] 建索引后:\n{after}");

        assert!(
            !after.contains("TEMP B-TREE"),
            "容量裁剪按 last_modified_utc 取最旧的超出部分，索引应直接给出有序候选，实际计划:\n{after}"
        );
        assert!(
            after.contains("idx_clip_items_live_favorite_modified"),
            "容量裁剪应复用列表复合索引（is_deleted + is_favorite 双等值 + last_modified_utc 有序），实际计划:\n{after}"
        );
    }

    /// 分级清理的两级候选查询都必须顺扫图片索引拿到 created_utc 有序候选。
    ///
    /// 这两条 SQL 的生产归属在 `image::tiered_cleanup`（本项不改该文件），
    /// 故此处按同形语句断言索引足以支撑其排序，防止索引列序被改坏后清理退化为全表排序。
    #[test]
    fn tiered_cleanup_candidate_plans_seek_images_created_index() {
        let conn = make_schema_conn();
        insert_text_row(&conn, "id-a", "hash-aaaaaaaaaaaa", 1_700_000_000_000, false);
        conn.execute(
            "INSERT INTO clip_images
                 (id, clip_item_id, thumbnail, original, original_present, image_hash,
                  created_utc, last_modified_utc, is_deleted, is_favorite)
             VALUES ('img-a', 'id-a', x'00', x'00', 1, 'imghash-aaaaaaaa', 1700000000000, 1700000000000, 0, 0)",
            [],
        )
        .expect("插入图片行失败");

        let level_one = "SELECT id, COALESCE(length(original), 0) FROM clip_images
             WHERE is_deleted = 0 AND is_favorite = 0 AND original_present = 1
             ORDER BY created_utc ASC";
        let level_two = "SELECT id, clip_item_id,
                    COALESCE(length(original), 0) + COALESCE(length(thumbnail), 0)
             FROM clip_images
             WHERE is_deleted = 0 AND is_favorite = 0
             ORDER BY created_utc ASC";

        let before_one = explain_query_plan(&conn, level_one);
        let before_two = explain_query_plan(&conn, level_two);

        ensure_indexes(&conn).expect("建索引失败");

        let after_one = explain_query_plan(&conn, level_one);
        let after_two = explain_query_plan(&conn, level_two);

        eprintln!(
            "[分级清理第一级] 建索引前:\n{before_one}\n[分级清理第一级] 建索引后:\n{after_one}\n\
             [分级清理第二级] 建索引前:\n{before_two}\n[分级清理第二级] 建索引后:\n{after_two}"
        );

        for (level, plan) in [("第一级", &after_one), ("第二级", &after_two)] {
            assert!(
                !plan.contains("TEMP B-TREE"),
                "分级清理{level}按 created_utc 顺序消耗旧行，索引应直接给出有序候选，实际计划:\n{plan}"
            );
            assert!(
                plan.contains("idx_clip_images_live_favorite_created"),
                "分级清理{level}应走 (is_deleted, is_favorite, created_utc) 索引，实际计划:\n{plan}"
            );
        }
        assert!(
            before_one.contains("TEMP B-TREE") && before_two.contains("TEMP B-TREE"),
            "基线（无索引）本该需要临时排序，若这里不成立说明对比前提失效"
        );
    }

    /// `ensure_schema` 可重复执行：不报错，且索引不会被重复创建。
    #[test]
    fn ensure_schema_creates_indexes_idempotently() {
        // 用 v1 基线形态起步：这条例要跑完整条迁移链（版本 0 → 当前），
        // 表若已是当前形态，版本 2 的 ADD COLUMN 会命中 duplicate column。
        let conn = make_baseline_v1_conn();

        ensure_schema(&conn).expect("首次初始化 schema 失败");
        let first = index_names(&conn);

        ensure_schema(&conn).expect("重复初始化 schema 不应报错（IF NOT EXISTS 幂等）");
        let second = index_names(&conn);

        assert_eq!(
            first, EXPECTED_INDEX_NAMES,
            "首次初始化应恰好建出约定的 5 条索引，多建说明有冗余索引、少建说明索引漏了，实际: {first:?}"
        );
        assert_eq!(
            second, first,
            "重复执行不应新增或改名索引（幂等），否则每次开库都会堆索引、拖慢写入，实际: {second:?}"
        );

        // 与任务约定的核对方式一致：按 idx_ 前缀计数
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_%'",
                [],
                |row| row.get(0),
            )
            .expect("统计索引数失败");
        assert_eq!(
            count,
            EXPECTED_INDEX_NAMES.len() as i64,
            "idx_ 前缀索引数应等于约定条数"
        );
    }

    /// 存量库补索引：已有数据在 `ensure_schema` 重跑后仍在、且可正常查询。
    #[test]
    fn ensure_schema_rerun_keeps_existing_rows_queryable() {
        // Arrange：模拟一个只到版本 1 表形态、没有索引的旧库，并写入数据。
        // 这里刻意不走 `ingest`：它要求 `title` 列存在（版本 2 才加），
        // 而这条例要的正是「在版本 2 之前就存在的老数据」在升级后仍然可查。
        let conn = make_baseline_v1_conn();
        let first_id = "id-legacy";
        conn.execute(
            "INSERT INTO clip_items
                 (id, content, kind, created_utc, last_modified_utc, is_deleted,
                  text_hash, is_favorite)
             VALUES (?1, ?2, 'text', ?3, ?3, 0, ?4, 0)",
            rusqlite::params![
                first_id,
                "旧库里的条目",
                1_700_000_000_000i64,
                text_hash("旧库里的条目")
            ],
        )
        .expect("写入旧数据失败");
        assert_eq!(
            index_names(&conn),
            Vec::<String>::new(),
            "前置条件：旧库无任何 idx_ 索引"
        );

        // Act：重跑 ensure_schema（等价于升级后再次开库）
        ensure_schema(&conn).expect("存量库补索引不应失败");

        // Assert：数据仍在、内容不变、且能被列表与去重路径查到
        let rows = list_items_with_images(&conn).expect("补索引后列表查询失败");
        assert_eq!(rows.len(), 1, "补索引不得丢失既有条目");
        assert_eq!(rows[0].id, first_id, "补索引后条目 id 应保持不变");
        assert_eq!(rows[0].content, "旧库里的条目", "补索引后内容应保持不变");
        let title: Option<String> = conn
            .query_row(
                "SELECT title FROM clip_items WHERE id = ?1",
                [first_id],
                |row| row.get(0),
            )
            .expect("补索引后应可读 title 列");
        assert_eq!(
            title, None,
            "版本 2 之前的老行升级后 title 应是 NULL（= 尚未回填），迁移不得顺手做全表 UPDATE"
        );

        let hash = text_hash("旧库里的条目");
        assert!(
            has_live_text_item(&conn, &hash).expect("去重查询失败"),
            "补索引后原条目的去重键仍应命中，否则同一内容会被重复入库"
        );
    }

    /// 同毫秒并列时列表顺序仍按 `rowid DESC` 兜底（索引反向扫描不得破坏该语义）。
    ///
    /// 这是建索引最容易悄悄改变的行为：索引自带 rowid 隐含列，
    /// 一旦扫描方向与 ORDER BY 不一致，同时间戳条目的先后就会与索引前相反。
    #[test]
    fn list_order_tiebreak_is_rowid_desc_with_index() {
        let conn = make_schema_conn();
        ensure_indexes(&conn).expect("建索引失败");
        let ts = 1_700_000_000_000;
        insert_text_row(&conn, "id-first", "hash-first0000000", ts, false);
        insert_text_row(&conn, "id-second", "hash-second000000", ts, false);
        insert_text_row(&conn, "id-third", "hash-third0000000", ts, false);

        let ids: Vec<String> = list_items_with_images(&conn)
            .expect("列表查询失败")
            .into_iter()
            .map(|r| r.id)
            .collect();

        assert_eq!(
            ids,
            vec!["id-third", "id-second", "id-first"],
            "同毫秒并列时必须按 rowid DESC 定序（后插入的排前）；顺序一旦随索引方向翻转，用户每次刷新看到的列表顺序都会跳变"
        );
    }

    /// 规模化观测：2 万条条目（含 500 张图）下列表查询与去重查询走索引前后的耗时对比。
    ///
    /// 取 2 万条是为了让「全表扫 + 临时排序」与「索引定位」的差距脱离毫秒噪声。
    /// 绝对耗时受机器负载与构建配置影响（debug 构建、内存库、无页 I/O），**不作断言**，
    /// 只打印观测值；断言只覆盖确定性的结果集与顺序。
    #[test]
    fn list_query_scale_observation() {
        let conn = make_schema_conn();
        conn.execute_batch(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 20000)
             INSERT INTO clip_items
                 (id, content, kind, created_utc, last_modified_utc, is_deleted, text_hash, is_favorite)
             SELECT printf('id-%06d', i), 'text ' || i, 'text',
                    1700000000000 + i, 1700000000000 + i,
                    CASE WHEN i % 97 = 0 THEN 1 ELSE 0 END,
                    printf('%016x', i),
                    CASE WHEN i % 41 = 0 THEN 1 ELSE 0 END
             FROM n;
             WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 500)
             INSERT INTO clip_images
                 (id, clip_item_id, thumbnail, original, original_present, image_hash,
                  created_utc, last_modified_utc, is_deleted, is_favorite)
             SELECT printf('img-%03d', i), printf('id-%06d', i * 7), x'00', x'00', 1,
                    printf('h%015x', i), 1700000000000 + i, 1700000000000 + i, 0,
                    CASE WHEN i % 11 = 0 THEN 1 ELSE 0 END
             FROM n;",
        )
        .expect("批量造数失败");

        let live: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 0",
                [],
                |row| row.get(0),
            )
            .expect("统计未软删条目失败");
        let images: i64 = conn
            .query_row("SELECT COUNT(*) FROM clip_images", [], |row| row.get(0))
            .expect("统计图片行数失败");
        // 取最后一条的哈希：命中位置最靠后，全表扫必须扫到底才停
        let last_hash = text_hash("text 20000");

        // ---- 基线：无任何索引 ----
        let started = std::time::Instant::now();
        let before_rows = list_items_with_images(&conn).expect("基线列表查询失败");
        let list_before = started.elapsed();

        let started = std::time::Instant::now();
        let text_hit_before = has_live_text_item(&conn, &last_hash).expect("基线文本去重查询失败");
        let text_before = started.elapsed();

        let started = std::time::Instant::now();
        let image_hit_before =
            has_live_image_item(&conn, "h0000000000001f3").expect("基线图片去重失败");
        let image_before = started.elapsed();

        // ---- 建索引后 ----
        ensure_indexes(&conn).expect("建索引失败");

        let started = std::time::Instant::now();
        let after_rows = list_items_with_images(&conn).expect("建索引后列表查询失败");
        let list_after = started.elapsed();

        let started = std::time::Instant::now();
        let text_hit_after =
            has_live_text_item(&conn, &last_hash).expect("建索引后文本去重查询失败");
        let text_after = started.elapsed();

        let started = std::time::Instant::now();
        let image_hit_after =
            has_live_image_item(&conn, "h0000000000001f3").expect("建索引后图片去重失败");
        let image_after = started.elapsed();

        eprintln!(
            "[规模观测] 未软删条目 {live} 条 / 图片 {images} 条（内存库 + debug 构建，绝对耗时不作断言）\
             \n  列表查询：建索引前 {list_before:?} -> 建索引后 {list_after:?}\
             \n  文本去重：建索引前 {text_before:?} -> 建索引后 {text_after:?}\
             \n  图片去重：建索引前 {image_before:?} -> 建索引后 {image_after:?}"
        );

        // ---- 断言：只覆盖确定性的结果集与顺序 ----
        assert_eq!(
            after_rows.len() as i64,
            live,
            "建索引后列表查询必须仍返回全部未软删条目，少一条都是索引导致的漏行"
        );
        assert_eq!(
            text_hit_after, text_hit_before,
            "建索引不得改变文本去重判定结果，否则同一内容会被重复入库或反向漏判"
        );
        assert_eq!(
            image_hit_after, image_hit_before,
            "建索引不得改变图片去重判定结果，否则同图会被重复入库或反向漏判"
        );

        let before_order: Vec<(&String, bool, i64)> = before_rows
            .iter()
            .map(|r| (&r.id, r.is_favorite, r.last_modified_utc))
            .collect();
        let after_order: Vec<(&String, bool, i64)> = after_rows
            .iter()
            .map(|r| (&r.id, r.is_favorite, r.last_modified_utc))
            .collect();
        assert_eq!(
            after_order, before_order,
            "建索引不得改变列表排序结果（收藏优先 + 时间倒序 + rowid 兜底必须与索引前逐行一致）"
        );
    }

    // ===== 自动淘汰（evict_over_limit） =====

    /// 按时间戳顺序插入 `count` 条非收藏文本行，返回它们的 id（从旧到新）。
    fn insert_aged_rows(conn: &rusqlite::Connection, prefix: &str, count: usize) -> Vec<String> {
        (0..count)
            .map(|i| {
                let id = format!("{prefix}-{i:03}");
                let ts = 1_700_000_000_000 + i as i64 * 1_000;
                insert_text_row(conn, &id, &format!("hash-{id}"), ts, false);
                id
            })
            .collect()
    }

    /// 未软删的 id 列表（按 last_modified_utc 升序，即从旧到新）。
    fn live_ids_oldest_first(conn: &rusqlite::Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare(
                "SELECT id FROM clip_items WHERE is_deleted = 0
                 ORDER BY last_modified_utc ASC, rowid ASC",
            )
            .expect("准备查询失败");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("查询失败");
        rows.map(|r| r.expect("读取失败")).collect()
    }

    /// 超过上限时只淘汰最旧的非收藏条目，最新的 `max_items` 条必须原样保留。
    #[test]
    fn evict_over_limit_soft_deletes_oldest_non_favorites() {
        // Arrange：5 条非收藏条目（id-000 最旧），上限设 3
        let conn = make_schema_conn();
        let ids = insert_aged_rows(&conn, "id", 5);

        // Act
        let evicted = evict_over_limit(&conn, 3).expect("淘汰应成功");

        // Assert：删掉的是最旧的两条，保留最新三条
        assert_eq!(evicted, 2, "5 条超 3 条上限应淘汰 2 条");
        assert_eq!(
            live_ids_oldest_first(&conn),
            vec![ids[2].clone(), ids[3].clone(), ids[4].clone()],
            "留下的必须是最新的 3 条"
        );
        let deleted: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 1",
                [],
                |row| row.get(0),
            )
            .expect("计数失败");
        assert_eq!(deleted, 2, "被淘汰的行应置墓碑（软删），不是物理删除");
    }

    /// 收藏项豁免：既不计数，也永不因超限被删——哪怕它是最旧的一条。
    #[test]
    fn evict_over_limit_never_deletes_favorites() {
        // Arrange：最旧的一条是收藏；另有 4 条非收藏
        let conn = make_schema_conn();
        insert_text_row(&conn, "fav-oldest", "hash-fav", 1_000, true);
        let ids = insert_aged_rows(&conn, "id", 4);

        // Act：上限 2，非收藏有 4 条
        let evicted = evict_over_limit(&conn, 2).expect("淘汰应成功");

        // Assert：收藏还在，淘汰的是非收藏里最旧的两条
        assert_eq!(evicted, 2, "收藏不计数，4 条非收藏超 2 条上限应淘汰 2 条");
        assert_eq!(
            live_ids_oldest_first(&conn),
            vec!["fav-oldest".to_string(), ids[2].clone(), ids[3].clone()],
            "最旧的收藏必须保留，被淘汰的是非收藏中最旧的"
        );
    }

    /// 收藏项不占名额：非收藏条数正好等于上限时一条都不该删，即使总条数远超上限。
    #[test]
    fn evict_over_limit_counts_only_non_favorites() {
        // Arrange：10 条收藏 + 3 条非收藏，上限 3
        let conn = make_schema_conn();
        for i in 0..10 {
            insert_text_row(
                &conn,
                &format!("fav-{i}"),
                &format!("hash-fav-{i}"),
                1_000 + i,
                true,
            );
        }
        let ids = insert_aged_rows(&conn, "id", 3);

        // Act
        let evicted = evict_over_limit(&conn, 3).expect("淘汰应成功");

        // Assert：非收藏正好 3 条 → 0 删除；13 条活跃行全部还在
        assert_eq!(evicted, 0, "非收藏条数等于上限时不应删除任何行");
        assert_eq!(
            live_ids_oldest_first(&conn).len(),
            13,
            "收藏既不计数也不被淘汰，总活跃行数不应变化（含 3 条非收藏 {ids:?}）"
        );
    }

    /// 边界：上限为最小值 1 时，非收藏只保留最新一条。
    #[test]
    fn evict_over_limit_boundary_one_keeps_newest_only() {
        let conn = make_schema_conn();
        let ids = insert_aged_rows(&conn, "id", 3);

        let evicted = evict_over_limit(&conn, 1).expect("淘汰应成功");

        assert_eq!(evicted, 2, "3 条超 1 条上限应淘汰 2 条");
        assert_eq!(
            live_ids_oldest_first(&conn),
            vec![ids[2].clone()],
            "上限 1 只应留下最新的一条"
        );
    }

    /// 边界：上限为最大值 999 时，999 条正好不触发淘汰；第 1000 条才淘汰 1 条。
    #[test]
    fn evict_over_limit_boundary_max_999() {
        // Arrange：999 条（正好等于上限）
        let conn = make_schema_conn();
        let ids = insert_aged_rows(&conn, "id", 999);

        // Act 1：正好等于上限
        let evicted_at_limit = evict_over_limit(&conn, 999).expect("淘汰应成功");

        // Assert 1：等于上限不动手（与图片分级清理「等于不动手」同一口径）
        assert_eq!(evicted_at_limit, 0, "正好 999 条等于上限，不应淘汰");
        assert_eq!(live_ids_oldest_first(&conn).len(), 999);

        // Act 2：再多一条 → 超限 1
        insert_text_row(&conn, "id-newest", "hash-newest", 1_800_000_000_000, false);
        let evicted_over_limit = evict_over_limit(&conn, 999).expect("淘汰应成功");

        // Assert 2：只淘汰最旧那条，新插入的必须在
        assert_eq!(evicted_over_limit, 1, "1000 条超 999 上限应淘汰 1 条");
        let live = live_ids_oldest_first(&conn);
        assert_eq!(live.len(), 999, "淘汰后应回到 999 条");
        assert!(
            live.contains(&"id-newest".to_string()),
            "最新插入的条目不得被淘汰"
        );
        assert!(
            !live.contains(&ids[0]),
            "最旧的一条应被淘汰，实际仍在：{:?}",
            &live[..3]
        );
    }

    /// 未超限时零删除，且不产生墓碑（避免「每轮入库都付一次候选 id 查询」的行为退化）。
    #[test]
    fn evict_over_limit_is_noop_under_limit() {
        let conn = make_schema_conn();
        let ids = insert_aged_rows(&conn, "id", 3);

        let evicted = evict_over_limit(&conn, 10).expect("淘汰应成功");

        assert_eq!(evicted, 0);
        assert_eq!(live_ids_oldest_first(&conn), ids);
        let deleted: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 1",
                [],
                |row| row.get(0),
            )
            .expect("计数失败");
        assert_eq!(deleted, 0, "未超限不应产生任何墓碑");
    }

    /// 空库不应报错（首次启动、或全部条目被删后）。
    #[test]
    fn evict_over_limit_on_empty_db_is_noop() {
        let conn = make_schema_conn();

        let evicted = evict_over_limit(&conn, 1).expect("空库淘汰应成功");

        assert_eq!(evicted, 0);
    }

    /// 超限判据的计数语句必须走列表索引前缀，不能退化成全表扫 + 临时排序。
    ///
    /// 这是常态轮次（未超限）唯一会执行的那条 SQL，退化会直接落在每 500ms 的捕获路径上。
    #[test]
    fn evict_count_plan_seeks_index() {
        let conn = make_schema_conn();
        insert_text_row(&conn, "id-a", "hash-aaaaaaaaaaaa", 1_700_000_000_000, false);
        ensure_indexes(&conn).expect("建索引失败");

        let plan = explain_query_plan(&conn, EVICT_LIVE_NON_FAVORITE_COUNT_SQL);

        assert!(
            !plan.contains("SCAN clip_items"),
            "超限计数应走索引定位，不能全表扫，实际计划:\n{plan}"
        );
        assert!(
            plan.contains("idx_clip_items_live_favorite_modified"),
            "超限计数应复用 (is_deleted, is_favorite) 前缀的列表索引，实际计划:\n{plan}"
        );
    }
}
