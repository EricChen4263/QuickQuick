//! 备份/恢复 IPC 命令层（可移植备份包的导出与导入）
//!
//! 命令清单（前端通过 invoke 对应命令名调用）：
//! - `export_backup`            — 收集历史 → 口令加密 → 写入后端选定的目录 → 回传路径
//! - `import_backup`            — 解密 → 解析 → 判重合并入库
//! - `reveal_backup_in_finder`  — 在系统文件管理器里定位备份文件
//!
//! # 落盘路径策略
//! 由**后端**决定目录（文稿 → 下载 → 应用数据目录逐级回退），前端只展示回传的绝对路径。
//! 为什么不让用户选路径：原生保存对话框要 `tauri-plugin-dialog`，那是新的 crate 依赖；
//! 本项目的备份包路径固定可预期，用「后端落盘 + 回传路径 + 在文件管理器中显示」即可覆盖
//! 「我知道文件去哪了」这个真实诉求。
//!
//! # 导入的原子性
//! 解密与解析在**任何写操作之前**全部完成；合并阶段整体包在一个 SAVEPOINT 里，
//! 任一条失败就 ROLLBACK TO，库回到导入前的状态。因此「口令错 / 文件坏 / 中途失败」
//! 三种情况都不会留下半份备份。
//!
//! # 口令的可见性
//! 口令只以 `&str` 在函数间传递，不落日志、不进错误消息、不进任何返回 DTO。

use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::db::{self, DbError};
use crate::ipc::{with_db, AppDb};
use crate::portable::{
    self, BackupImage, BackupItem, BackupPayload, PortableError, PAYLOAD_FORMAT, PAYLOAD_VERSION,
};

/// 备份文件扩展名（与便携文件 magic "QQBP" 呼应）
pub const BACKUP_FILE_EXTENSION: &str = "qqbp";

/// 备份文件名前缀
const BACKUP_FILE_PREFIX: &str = "QuickQuick-Backup-";

/// 导入合并用的 SAVEPOINT 名称
const IMPORT_SAVEPOINT: &str = "portable_import";

/// 同名备份文件的探测上限（同一秒内重复导出最多生成 100 个不同文件名）
const MAX_NAME_PROBE: u32 = 100;

/// 导入备份包的解码后字节数上限：1 GiB。
///
/// 防的是「误选了某个超大文件 / 恶意构造的超大包」把内存打爆——base64 解码、
/// 载荷解析、JSON 反序列化各自都要再分配一份，不设限时峰值可达文件大小的 5～6 倍。
/// 这**不是**业务容量限制：真实剪贴板历史备份远到不了这个量级。
pub const MAX_BACKUP_BLOB_BYTES: usize = 1024 * 1024 * 1024;

/// 导出结果 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBackupDto {
    /// 备份包落盘的绝对路径
    pub path: String,
    /// 写入备份包的条目数
    pub item_count: usize,
    /// 因判重键不可得而未纳入备份的条目数（如实上报，不静默丢弃）
    pub skipped_count: usize,
    /// 是否因文稿/下载目录不可用而回退到了应用数据目录
    ///
    /// 由命令层回填（导出核心函数只负责落盘，不知道目录是怎么选出来的），
    /// 前端据此解释「备份位置」为什么与用户预期不同。
    pub used_fallback_dir: bool,
}

impl ExportBackupDto {
    /// 标记这次导出是否用了回退目录。
    fn with_fallback_dir(mut self, used_fallback_dir: bool) -> Self {
        self.used_fallback_dir = used_fallback_dir;
        self
    }
}

/// 导入结果 DTO。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBackupDto {
    /// 新插入的条目数
    pub inserted: usize,
    /// 因库内已存在相同内容/图片而跳过的条目数
    pub skipped: usize,
}

/// 当前 UTC 时间戳（毫秒）。
fn now_utc_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// 校验口令非空。
///
/// 空口令等价于无加密，必须在这里挡住，而不是让 Argon2 用空串派生出一个「能解」的口令。
///
/// # Errors
/// 口令为空时返回中文提示（不含口令内容）。
fn validate_passphrase(passphrase: &str) -> Result<(), String> {
    if passphrase.is_empty() {
        return Err("请输入备份口令".to_string());
    }
    Ok(())
}

/// 把便携文件错误映射成面向用户的中文提示。
///
/// 任何分支都不回显口令：`PortableError` 的各变体本身不携带口令值，
/// 这里也只输出固定文案，杜绝把用户口令拼进错误消息。
fn map_portable_error(err: PortableError) -> String {
    match err {
        PortableError::WrongPassphrase => "备份口令错误，或文件已损坏".to_string(),
        PortableError::Decrypt => "备份文件解密失败，请重新导出后再试".to_string(),
        PortableError::Format => "不是有效的 QuickQuick 备份文件（格式头不符）".to_string(),
        PortableError::Kdf => "备份密钥派生失败，请重试".to_string(),
        PortableError::Payload => "备份内容格式无效，无法解析".to_string(),
        PortableError::UnsupportedVersion { found, supported } => {
            format!("备份文件版本 {found} 高于当前支持的 {supported}，请先升级 QuickQuick")
        }
    }
}

/// 把库内未软删条目组装成备份载荷，返回（载荷, 跳过条数）。
///
/// 只有一种条目会被跳过：图片条目的判重键 `image_hash` 既不在库里、也无法由原图字节
/// 重算出来。那种条目导出后无法保证导入幂等（同一张图会插两次），
/// 因此宁可不导出并如实上报跳过条数，也不产出一个「越导越乱」的备份包。
///
/// # Errors
/// `DbError::Sqlite`：读取条目失败
pub fn build_backup_payload(
    conn: &rusqlite::Connection,
    created_utc: i64,
) -> Result<(BackupPayload, usize), DbError> {
    let rows = db::list_backup_items(conn)?;
    let mut items = Vec::with_capacity(rows.len());
    let mut skipped = 0usize;

    for row in rows {
        if row.kind == "image" {
            match build_image_item(&row) {
                Some(item) => items.push(item),
                None => skipped += 1,
            }
            continue;
        }

        // 文本/富文本：kind 由 html 是否存在推导，与入库时的推导规则一致
        items.push(BackupItem {
            kind: if row.html_content.is_some() {
                "richtext".to_string()
            } else {
                "text".to_string()
            },
            content: row.content,
            html: row.html_content,
            is_favorite: row.is_favorite,
            created_utc: row.created_utc,
            last_modified_utc: row.last_modified_utc,
            image: None,
        });
    }

    let payload = BackupPayload {
        format: PAYLOAD_FORMAT.to_string(),
        version: PAYLOAD_VERSION,
        created_utc,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        items,
    };

    Ok((payload, skipped))
}

/// 把一条图片条目转成载荷条目；判重键不可得时返回 `None`（由调用方计入跳过）。
fn build_image_item(row: &db::BackupItemRow) -> Option<BackupItem> {
    let image = row.image.as_ref()?;

    let image_hash = match image.image_hash.as_deref() {
        Some(hash) if !hash.is_empty() => hash.to_string(),
        // 库内哈希为空但原图还在：按同一算法重算，结果与入库时一致
        _ if !image.original.is_empty() => crate::image::image_hash(&image.original),
        _ => return None,
    };

    let has_original = image.original_present && !image.original.is_empty();

    Some(BackupItem {
        kind: "image".to_string(),
        content: row.content.clone(),
        html: None,
        is_favorite: row.is_favorite,
        created_utc: row.created_utc,
        last_modified_utc: row.last_modified_utc,
        image: Some(BackupImage {
            image_hash,
            original_present: has_original,
            thumbnail_base64: base64::engine::general_purpose::STANDARD.encode(&image.thumbnail),
            original_base64: has_original
                .then(|| base64::engine::general_purpose::STANDARD.encode(&image.original)),
        }),
    })
}

/// 导出备份到指定目录（目录由调用方决定，便于测试用临时目录）。
///
/// 流程：组装载荷 → 序列化明文 → 口令加密 → 创建目录 → 写文件。
///
/// # Errors
/// 口令为空、读取库、序列化、加密或写文件失败时返回中文提示（不含口令）。
pub fn export_backup_to_dir(
    conn: &rusqlite::Connection,
    passphrase: &str,
    dir: &Path,
    now_ms: i64,
) -> Result<ExportBackupDto, String> {
    validate_passphrase(passphrase)?;

    let (payload, skipped_count) =
        build_backup_payload(conn, now_ms).map_err(|e| format!("收集备份内容失败：{e}"))?;
    let item_count = payload.items.len();

    // 空库也照常导出：items 为空数组仍是一个合法备份包（导入后 0 新增）
    let plaintext = portable::encode_backup_payload(&payload)
        .map_err(|e| format!("备份内容序列化失败：{e}"))?;
    let blob = portable::export_portable(&plaintext, passphrase)
        .map_err(|e| format!("备份加密失败：{e}"))?;

    std::fs::create_dir_all(dir).map_err(|e| format!("无法创建备份目录 {}：{e}", dir.display()))?;

    let path = unique_backup_path(dir, now_ms)?;
    write_backup_file(&path, &blob)?;

    Ok(ExportBackupDto {
        path: path.to_string_lossy().into_owned(),
        item_count,
        skipped_count,
        // 目录决策不在这层：命令层拿到回退事实后用 with_fallback_dir 回填
        used_fallback_dir: false,
    })
}

/// 先写临时文件再改名，保证落盘的 `.qqbp` 永远是完整备份包。
///
/// 为什么不用裸 `fs::write`：磁盘写满或进程被杀时会留下半份文件，
/// 而半份文件在导入侧只会表现为「口令错误或文件损坏」，用户无法判断是备份本身没写成。
/// 临时文件带 `.part` 后缀，改名成功后才是可导入的备份包。
///
/// # Errors
/// 写临时文件或改名失败时返回中文提示（失败时尽力清掉临时文件）。
fn write_backup_file(path: &Path, blob: &[u8]) -> Result<(), String> {
    let temp_path = path.with_extension(format!("{BACKUP_FILE_EXTENSION}.part"));

    std::fs::write(&temp_path, blob)
        .map_err(|e| format!("写入备份文件失败（{}）：{e}", temp_path.display()))?;

    std::fs::rename(&temp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        format!("备份文件落盘失败（{}）：{e}", path.display())
    })
}

/// 生成不冲突的备份文件路径。
///
/// 命名：`QuickQuick-Backup-YYYYMMDD-HHmmss.qqbp`（时间戳取 UTC），
/// 同名已存在时依次探测 `-2`、`-3`……避免同一秒内重复导出互相覆盖。
///
/// # Errors
/// 同名文件超过 [`MAX_NAME_PROBE`] 个时返回错误（不静默覆盖既有备份）。
fn unique_backup_path(dir: &Path, now_ms: i64) -> Result<PathBuf, String> {
    let stem = format_backup_stem(now_ms);

    for seq in 1..=MAX_NAME_PROBE {
        let file_name = if seq == 1 {
            format!("{stem}.{BACKUP_FILE_EXTENSION}")
        } else {
            format!("{stem}-{seq}.{BACKUP_FILE_EXTENSION}")
        };
        let candidate = dir.join(file_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(format!(
        "备份目录内同名文件过多（{stem}），请先整理 {}",
        dir.display()
    ))
}

/// 生成备份文件名主干（不含扩展名）：`QuickQuick-Backup-YYYYMMDD-HHmmss`。
#[must_use]
fn format_backup_stem(now_ms: i64) -> String {
    let (year, month, day, hour, minute, second) = civil_from_epoch_ms(now_ms);
    format!("{BACKUP_FILE_PREFIX}{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}

/// 把 UTC epoch 毫秒拆成 `(年, 月, 日, 时, 分, 秒)`。
///
/// 自己算而不引入日期库：本项目不依赖 chrono/time，为一行文件名格式化新增 crate
/// 不值得。文件名时间戳统一取 **UTC**，与库内时间字段口径一致。
fn civil_from_epoch_ms(now_ms: i64) -> (i32, u32, u32, u32, u32, u32) {
    let secs = now_ms.div_euclid(1_000);
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);

    let (year, month, day) = civil_from_days(days);
    let hour = (secs_of_day / 3_600) as u32;
    let minute = ((secs_of_day % 3_600) / 60) as u32;
    let second = (secs_of_day % 60) as u32;

    (year, month, day, hour, minute, second)
}

/// 由「1970-01-01 起的天数」求公历年月日（Howard Hinnant 的 civil_from_days 算法）。
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    // 把纪元平移到 0000-03-01，使闰年日落在四年周期的末端
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };

    (
        (year + i64::from(month <= 2)) as i32,
        month as u32,
        day as u32,
    )
}

/// 解密并合并一份备份包。
///
/// 顺序：解密 → 解析 → （两者都成功后）SAVEPOINT 内合并。
/// 解密/解析失败时一行都不写，库保持导入前状态。
///
/// # Errors
/// 口令为空、口令错误、文件格式非法、载荷不可解析、合并失败时返回中文提示；
/// 合并失败的分支明确说明「已回滚，库内数据未改变」。
pub fn import_backup_bytes(
    conn: &rusqlite::Connection,
    blob: &[u8],
    passphrase: &str,
) -> Result<ImportBackupDto, String> {
    import_backup_bytes_with_limit(conn, blob, passphrase, MAX_BACKUP_BLOB_BYTES)
}

/// 同 [`import_backup_bytes`]，但上限由调用方给定（单测用小上限覆盖超限路径）。
///
/// 上限检查刻意排在解密**之前**：Argon2id 每次派生要占 64 MiB 并跑 3 轮，
/// 先付这笔代价再因为体积拒绝没有任何收益。
///
/// # Errors
/// 口令为空、体积超上限、口令错误、格式非法、载荷不可解析或合并失败时返回中文提示。
pub fn import_backup_bytes_with_limit(
    conn: &rusqlite::Connection,
    blob: &[u8],
    passphrase: &str,
    max_blob_bytes: usize,
) -> Result<ImportBackupDto, String> {
    validate_passphrase(passphrase)?;
    ensure_blob_within_limit(blob.len(), max_blob_bytes)?;

    let plaintext = portable::import_portable(blob, passphrase).map_err(map_portable_error)?;
    let payload = portable::decode_backup_payload(&plaintext).map_err(map_portable_error)?;

    merge_backup_payload(conn, &payload)
        .map_err(|e| format!("合并备份内容失败（已回滚，库内数据未改变）：{e}"))
}

/// 在 SAVEPOINT 内合并载荷：逐条判重后插入，任一条失败整体回滚。
///
/// 为什么用 SAVEPOINT 而不是 Transaction：项目里 `db` 层已有嵌套 SAVEPOINT 的用法，
/// 同一连接在 SAVEPOINT 下可继续复用（Transaction 会占住 `&mut Connection` 借用）。
///
/// # Errors
/// `DbError`：任一 `INSERT` 或 base64 解码失败（调用方已回滚）
fn merge_backup_payload(
    conn: &rusqlite::Connection,
    payload: &BackupPayload,
) -> Result<ImportBackupDto, DbError> {
    conn.execute_batch(&format!("SAVEPOINT {IMPORT_SAVEPOINT};"))?;

    match merge_all_items(conn, payload) {
        Ok(summary) => {
            conn.execute_batch(&format!("RELEASE SAVEPOINT {IMPORT_SAVEPOINT};"))?;
            Ok(summary)
        }
        Err(err) => {
            // 回滚失败也不能掩盖原始错误：原错误才是可诊断的那一个
            let _ = conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {IMPORT_SAVEPOINT}; RELEASE SAVEPOINT {IMPORT_SAVEPOINT};"
            ));
            Err(err)
        }
    }
}

/// 逐条合并，累计新增/跳过数量。
fn merge_all_items(
    conn: &rusqlite::Connection,
    payload: &BackupPayload,
) -> Result<ImportBackupDto, DbError> {
    let mut inserted = 0usize;
    let mut skipped = 0usize;

    for item in &payload.items {
        if merge_one_item(conn, item)? {
            inserted += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(ImportBackupDto { inserted, skipped })
}

/// 合并单条：返回 `true` 表示新插入，`false` 表示跳过。
///
/// 跳过有三种原因，都不写库：
/// - 判重键命中（库里已有同内容文本 / 同图哈希）；
/// - 图片条目缺 `image` 字段（备份本身不完整，写入半成品行比跳过更糟）；
/// - `kind` 不可识别（更高版本才有的类型，本版本不猜语义）。
///
/// 命中的条目**不 bump、不改时间**：导入是「补齐缺失」，不是「再复制一次」，
/// 把既有条目顶到最前会打乱用户当前的排序认知。
fn merge_one_item(conn: &rusqlite::Connection, item: &BackupItem) -> Result<bool, DbError> {
    match item.kind.as_str() {
        "image" => merge_image_item(conn, item),
        "text" | "richtext" => {
            let hash = db::text_hash_of(&item.content);
            if db::has_live_text_item(conn, &hash)? {
                return Ok(false);
            }
            db::insert_backup_text_item(
                conn,
                &item.content,
                item.html.as_deref(),
                item.is_favorite,
                item.created_utc,
                item.last_modified_utc,
            )?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// 合并一条图片条目。
fn merge_image_item(conn: &rusqlite::Connection, item: &BackupItem) -> Result<bool, DbError> {
    let Some(image) = item.image.as_ref() else {
        return Ok(false);
    };

    if db::has_live_image_item(conn, &image.image_hash)? {
        return Ok(false);
    }

    let thumbnail = decode_base64(&image.thumbnail_base64)
        .map_err(|e| DbError::Other(format!("缩略图 base64 解码失败：{e}")))?;
    let original = image
        .original_base64
        .as_deref()
        .map(decode_base64)
        .transpose()
        .map_err(|e| DbError::Other(format!("原图 base64 解码失败：{e}")))?
        .unwrap_or_default();

    // 标记为「有原图」但字节缺失时按降级条目存：宁可在 UI 里显示降级态，
    // 也不写入一个 original_present=1 却取不出原图的矛盾行
    let original_present = image.original_present && !original.is_empty();

    db::insert_backup_image_item(
        conn,
        &db::BackupImageInsert {
            content: &item.content,
            is_favorite: item.is_favorite,
            created_utc: item.created_utc,
            last_modified_utc: item.last_modified_utc,
            image_hash: &image.image_hash,
            thumbnail: &thumbnail,
            original: &original,
            original_present,
        },
    )?;

    Ok(true)
}

/// 校验解码后的备份字节数不超过上限。
///
/// 口径是**解码后字节数**而不是 base64 字符串长度：字符串长度只是估算
/// （4 字符对应 3 字节，还受 padding/换行影响），判定必须用真实长度。
///
/// # Errors
/// 超限时返回带真实字节数与上限的中文提示。
fn ensure_blob_within_limit(len: usize, max_bytes: usize) -> Result<(), String> {
    const BYTES_PER_MIB: usize = 1024 * 1024;

    if len > max_bytes {
        return Err(format!(
            "备份文件过大（{} MiB，上限 {} MiB），请确认选择的是 QuickQuick 备份文件",
            len / BYTES_PER_MIB,
            max_bytes / BYTES_PER_MIB
        ));
    }
    Ok(())
}

/// base64 解码（标准字母表，与导出时的编码方式对应）。
fn decode_base64(text: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::engine::general_purpose::STANDARD.decode(text)
}

/// 校验「在文件管理器中显示」的目标路径。
///
/// 只放行本应用写出的备份包：必须是已存在的普通文件，且扩展名为 `.qqbp`，
/// 避免把任意路径交给系统 shell 打开。
///
/// # Errors
/// 扩展名不符或文件不存在时返回中文提示。
fn validate_reveal_path(path: &Path) -> Result<(), String> {
    if path.extension().and_then(|ext| ext.to_str()) != Some(BACKUP_FILE_EXTENSION) {
        return Err("只能在文件管理器中显示 .qqbp 备份文件".to_string());
    }
    if !path.is_file() {
        return Err(format!("备份文件不存在：{}", path.display()));
    }
    Ok(())
}

/// 解析备份落盘目录：文稿 → 下载 → 应用数据目录（逐级回退）。
///
/// 这三个目录都由系统约定给出，用户可在文件管理器里按回传路径直接找到。
/// 回退到应用数据目录时会 `eprintln!` 留痕，并把「用了回退目录」这一事实
/// 通过 [`ExportBackupDto::used_fallback_dir`] 带回前端——用户看到的路径与
/// 「备份存在文稿里」的预期不一致时，原因必须是可见的，而不是静默降级。
fn resolve_backup_dir(app: &AppHandle) -> Result<(PathBuf, bool), String> {
    let paths = app.path();
    let fallback = paths
        .app_data_dir()
        .map_err(|e| format!("无法确定备份目录：{e}"))?;

    let (dir, used_fallback) = pick_backup_dir(
        vec![paths.document_dir().ok(), paths.download_dir().ok()],
        fallback,
    );

    if used_fallback {
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("无法创建备份目录 {}：{e}", dir.display()))?;
        eprintln!(
            "[QuickQuick] 备份目录回退：文稿/下载目录不可用，改用应用数据目录 {}",
            dir.display()
        );
    }

    Ok((dir, used_fallback))
}

/// 按候选顺序挑第一个真实存在的目录；都不存在时用回退目录。
///
/// 拆成纯决策函数是为了可单测：只有它能区分「首选目录可用」与「全部回退」，
/// `AppHandle` 那层胶水在单测里拿不到。
fn pick_backup_dir(candidates: Vec<Option<PathBuf>>, fallback: PathBuf) -> (PathBuf, bool) {
    for candidate in candidates.into_iter().flatten() {
        if candidate.is_dir() {
            return (candidate, false);
        }
    }
    (fallback, true)
}

/// Tauri 命令：导出加密备份包，返回落盘路径与条目统计。
#[tauri::command]
pub fn export_backup(
    app: AppHandle,
    state: State<'_, AppDb>,
    passphrase: String,
) -> Result<ExportBackupDto, String> {
    let (dir, used_fallback) = resolve_backup_dir(&app)?;
    let now_ms = now_utc_ms();
    with_db(&state, |conn| {
        export_backup_to_dir(conn, &passphrase, &dir, now_ms)
            .map(|dto| dto.with_fallback_dir(used_fallback))
    })
}

/// Tauri 命令：导入备份包并合并进当前库。
///
/// 前端用 `<input type="file">` 读文件字节后 base64 传入（不引入 dialog 插件）。
#[tauri::command]
pub fn import_backup(
    state: State<'_, AppDb>,
    blob_base64: String,
    passphrase: String,
) -> Result<ImportBackupDto, String> {
    let blob = decode_base64(&blob_base64).map_err(|e| format!("备份文件读取失败：{e}"))?;
    with_db(&state, |conn| import_backup_bytes(conn, &blob, &passphrase))
}

/// Tauri 命令：在系统文件管理器中显示备份文件。
#[tauri::command]
pub fn reveal_backup_in_finder(path: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    validate_reveal_path(&target)?;
    tauri_plugin_opener::reveal_item_in_dir(&target)
        .map_err(|e| format!("无法在文件管理器中显示备份文件：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::CapturedItem;
    use crate::db::IngestOutcome;

    /// 测试用口令：可识别的非空脏值，用于断言它不会出现在错误消息/DTO 里。
    const PASSPHRASE: &str = "SENTINEL_PASS_DEADBEEF";

    /// 错误口令：同样用可识别脏值。
    const WRONG_PASSPHRASE: &str = "SENTINEL_WRONG_CAFE";

    /// 构造最小合法 RGBA PNG（用 image crate 编码，不依赖外部文件）。
    fn make_test_png(pixel: u8) -> Vec<u8> {
        use image::{ImageFormat, RgbaImage};
        let mut img = RgbaImage::new(2, 2);
        for p in img.pixels_mut() {
            *p = image::Rgba([pixel, pixel, pixel, 255]);
        }
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .expect("测试 PNG 编码失败");
        buf
    }

    /// 在临时目录建一个空加密库。
    fn make_db(dir: &tempfile::TempDir) -> rusqlite::Connection {
        crate::db::open_or_create(&dir.path().join("quickquick.db"), &[7u8; 32])
            .expect("建库应成功")
    }

    /// 往库里写三条样本：中文纯文本、富文本、图片（后者带原图）。
    fn seed_source_db(conn: &rusqlite::Connection) {
        let text_id = match db::ingest(
            conn,
            &CapturedItem {
                text: "中文条目一：换机也不能丢".to_string(),
                html: None,
            },
        )
        .expect("文本入库应成功")
        {
            IngestOutcome::Inserted(id) => id,
            IngestOutcome::Bumped(id) => id,
        };
        db::set_favorite(conn, &text_id, true).expect("设置收藏应成功");

        match db::ingest(
            conn,
            &CapturedItem {
                text: "富文本条目".to_string(),
                html: Some("<b>富文本条目</b><br/>第二行".to_string()),
            },
        )
        .expect("富文本入库应成功")
        {
            IngestOutcome::Inserted(id) | IngestOutcome::Bumped(id) => id,
        };

        db::ingest_image_as_clip(conn, 2, 2, &make_test_png(200), 20 * 1024 * 1024)
            .expect("图片入库应成功");
    }

    /// 取与 id 无关的业务字段快照（内容/类型/收藏位/两个时间戳/HTML）。
    ///
    /// 刻意返回**已排序**的集合：同一毫秒内的条目之间，列表顺序由 `rowid DESC`
    /// 决定，重新插入后 rowid 必然变化，那个相对顺序无法跨库保持；
    /// 有意义的断言是「条目集合与每条的时间戳/收藏位一致」。
    /// 顺序保持另由 `import_preserves_order_of_distinct_timestamps` 覆盖。
    fn business_snapshot(
        conn: &rusqlite::Connection,
    ) -> Vec<(String, String, bool, i64, i64, Option<String>)> {
        let mut snapshot = db::list_backup_items(conn)
            .expect("读取条目应成功")
            .into_iter()
            .map(|row| {
                (
                    row.content,
                    row.kind,
                    row.is_favorite,
                    row.created_utc,
                    row.last_modified_utc,
                    row.html_content,
                )
            })
            .collect::<Vec<_>>();
        snapshot.sort();
        snapshot
    }

    /// 导出 → 导入到空库：条目数、内容、HTML、收藏位、时间戳全部还原
    #[test]
    fn export_then_import_into_empty_db_restores_items() {
        // Arrange
        let source_dir = tempfile::tempdir().expect("临时目录");
        let target_dir = tempfile::tempdir().expect("临时目录");
        let source = make_db(&source_dir);
        seed_source_db(&source);
        let target = make_db(&target_dir);

        // Act: 导出
        let exported =
            export_backup_to_dir(&source, PASSPHRASE, source_dir.path(), 1_700_000_000_000)
                .expect("导出应成功");
        let blob = std::fs::read(&exported.path).expect("备份文件应可读");

        // Act: 导入到空库
        let imported = import_backup_bytes(&target, &blob, PASSPHRASE).expect("导入应成功");

        // Assert
        assert_eq!(exported.item_count, 3, "导出应包含三条");
        assert_eq!(exported.skipped_count, 0, "样本数据不应有任何条目被跳过");
        assert_eq!(imported.inserted, 3);
        assert_eq!(imported.skipped, 0);
        assert_eq!(db::count_live(&target).expect("计数应成功"), 3);
        assert_eq!(
            business_snapshot(&target),
            business_snapshot(&source),
            "导入后内容/类型/收藏位/时间戳必须与源库一致"
        );
        assert!(blob.starts_with(b"QQBP"), "落盘文件应带便携文件 magic 头");
    }

    /// 图片原图字节随备份往返（不是只恢复缩略图）
    #[test]
    fn import_restores_image_original_bytes() {
        // Arrange
        let source_dir = tempfile::tempdir().expect("临时目录");
        let target_dir = tempfile::tempdir().expect("临时目录");
        let source = make_db(&source_dir);
        let png = make_test_png(77);
        db::ingest_image_as_clip(&source, 2, 2, &png, 20 * 1024 * 1024).expect("图片入库应成功");
        let target = make_db(&target_dir);

        // Act
        let exported =
            export_backup_to_dir(&source, PASSPHRASE, source_dir.path(), 1_700_000_000_000)
                .expect("导出应成功");
        let blob = std::fs::read(&exported.path).expect("备份文件应可读");
        import_backup_bytes(&target, &blob, PASSPHRASE).expect("导入应成功");

        // Assert: 目标库里的原图 BLOB 与源 PNG 完全一致
        let restored = db::list_backup_items(&target).expect("读取条目应成功");
        let image = restored
            .first()
            .and_then(|row| row.image.as_ref())
            .expect("导入后应有关联图片行");
        assert_eq!(image.original, png, "原图字节必须逐字节还原");
        assert!(image.original_present, "原图应标记为已存");
        assert_eq!(
            image.image_hash.as_deref(),
            Some(crate::image::image_hash(&png).as_str()),
            "判重键必须与源库一致"
        );
    }

    /// 时间戳互不相同的条目：导入后列表顺序与源库一致（不打乱既有排序）
    #[test]
    fn import_preserves_order_of_distinct_timestamps() {
        // Arrange: 三条时间戳互不相同的文本条目（新→旧）
        let source_dir = tempfile::tempdir().expect("临时目录");
        let target_dir = tempfile::tempdir().expect("临时目录");
        let source = make_db(&source_dir);
        for (index, content) in ["最新条目", "中间条目", "最旧条目"].iter().enumerate()
        {
            let stamp = 1_700_000_000_000 - (index as i64) * 1_000;
            db::insert_backup_text_item(&source, content, None, false, stamp, stamp)
                .expect("构造源数据应成功");
        }
        let target = make_db(&target_dir);

        // Act
        let exported =
            export_backup_to_dir(&source, PASSPHRASE, source_dir.path(), 1_700_000_000_000)
                .expect("导出应成功");
        let blob = std::fs::read(&exported.path).expect("备份文件应可读");
        import_backup_bytes(&target, &blob, PASSPHRASE).expect("导入应成功");

        // Assert: 两条库的列表顺序（按列表 API 的排序）逐一相同
        let source_order: Vec<String> = db::list_backup_items(&source)
            .expect("读取源库应成功")
            .into_iter()
            .map(|row| row.content)
            .collect();
        let target_order: Vec<String> = db::list_backup_items(&target)
            .expect("读取目标库应成功")
            .into_iter()
            .map(|row| row.content)
            .collect();
        assert_eq!(
            target_order, source_order,
            "时间戳不同时导入必须保持原有排序"
        );
        assert_eq!(source_order, vec!["最新条目", "中间条目", "最旧条目"]);
    }

    /// 同一份备份导入两次：第二次 0 新增，库内条目与顺序不变
    #[test]
    fn import_twice_is_idempotent() {
        // Arrange
        let source_dir = tempfile::tempdir().expect("临时目录");
        let target_dir = tempfile::tempdir().expect("临时目录");
        let source = make_db(&source_dir);
        seed_source_db(&source);
        let target = make_db(&target_dir);

        let exported =
            export_backup_to_dir(&source, PASSPHRASE, source_dir.path(), 1_700_000_000_000)
                .expect("导出应成功");
        let blob = std::fs::read(&exported.path).expect("备份文件应可读");

        // Act
        let first = import_backup_bytes(&target, &blob, PASSPHRASE).expect("首次导入应成功");
        let snapshot_after_first = business_snapshot(&target);
        let second = import_backup_bytes(&target, &blob, PASSPHRASE).expect("二次导入应成功");

        // Assert
        assert_eq!(first.inserted, 3);
        assert_eq!(second.inserted, 0, "二次导入不应新增任何条目");
        assert_eq!(second.skipped, 3, "二次导入应把三条全部判为已存在");
        assert_eq!(
            business_snapshot(&target),
            snapshot_after_first,
            "二次导入不得改动既有条目（不打乱顺序、不改时间）"
        );
    }

    /// 库里已有同 text_hash 条目：导入跳过它，只补新的那条
    #[test]
    fn import_skips_item_already_present_by_text_hash() {
        // Arrange
        let source_dir = tempfile::tempdir().expect("临时目录");
        let target_dir = tempfile::tempdir().expect("临时目录");
        let source = make_db(&source_dir);
        db::ingest(
            &source,
            &CapturedItem {
                text: "两边都有的条目".to_string(),
                html: None,
            },
        )
        .expect("入库应成功");
        db::ingest(
            &source,
            &CapturedItem {
                text: "只有备份里有的条目".to_string(),
                html: None,
            },
        )
        .expect("入库应成功");

        let target = make_db(&target_dir);
        db::ingest(
            &target,
            &CapturedItem {
                text: "两边都有的条目".to_string(),
                html: None,
            },
        )
        .expect("目标库入库应成功");

        let exported =
            export_backup_to_dir(&source, PASSPHRASE, source_dir.path(), 1_700_000_000_000)
                .expect("导出应成功");
        let blob = std::fs::read(&exported.path).expect("备份文件应可读");

        // Act
        let result = import_backup_bytes(&target, &blob, PASSPHRASE).expect("导入应成功");

        // Assert
        assert_eq!(result.inserted, 1, "只应补入备份中独有的那条");
        assert_eq!(result.skipped, 1, "库里已有的那条应被跳过");
        assert_eq!(
            db::count_live(&target).expect("计数应成功"),
            2,
            "不应重复插入"
        );
    }

    /// 口令错误：明确中文报错、库内条目数不变、错误消息不含任何口令
    #[test]
    fn wrong_passphrase_reports_clear_error_and_leaves_db_untouched() {
        // Arrange
        let source_dir = tempfile::tempdir().expect("临时目录");
        let target_dir = tempfile::tempdir().expect("临时目录");
        let source = make_db(&source_dir);
        seed_source_db(&source);
        let target = make_db(&target_dir);
        db::ingest(
            &target,
            &CapturedItem {
                text: "目标库原有条目".to_string(),
                html: None,
            },
        )
        .expect("目标库入库应成功");
        let before = business_snapshot(&target);

        let exported =
            export_backup_to_dir(&source, PASSPHRASE, source_dir.path(), 1_700_000_000_000)
                .expect("导出应成功");
        let blob = std::fs::read(&exported.path).expect("备份文件应可读");

        // Act
        let result = import_backup_bytes(&target, &blob, WRONG_PASSPHRASE);

        // Assert
        let message = result.expect_err("错误口令必须返回 Err");
        assert_eq!(message, "备份口令错误，或文件已损坏");
        assert!(!message.contains(PASSPHRASE), "错误消息不得回显正确口令");
        assert!(
            !message.contains(WRONG_PASSPHRASE),
            "错误消息不得回显所输口令"
        );
        assert_eq!(
            db::count_live(&target).expect("计数应成功"),
            1,
            "导入失败不得改变库内条目数"
        );
        assert_eq!(
            business_snapshot(&target),
            before,
            "导入失败不得改动既有条目"
        );
    }

    /// 空口令被拒绝（空口令等价于不加密）
    #[test]
    fn empty_passphrase_is_rejected() {
        // Arrange
        let dir = tempfile::tempdir().expect("临时目录");
        let conn = make_db(&dir);

        // Act
        let export_result = export_backup_to_dir(&conn, "", dir.path(), 1_700_000_000_000);
        let import_result = import_backup_bytes(&conn, b"whatever", "");

        // Assert
        assert_eq!(
            export_result.expect_err("空口令导出应失败"),
            "请输入备份口令"
        );
        assert_eq!(
            import_result.expect_err("空口令导入应失败"),
            "请输入备份口令"
        );
    }

    /// 合并中途失败整体回滚：一条坏图片条目不能让前面的文本条目留下来
    #[test]
    fn merge_rolls_back_all_items_when_one_item_fails() {
        // Arrange: 第一条合法文本，第二条图片的 base64 非法
        let dir = tempfile::tempdir().expect("临时目录");
        let conn = make_db(&dir);
        let payload = BackupPayload {
            format: PAYLOAD_FORMAT.to_string(),
            version: PAYLOAD_VERSION,
            created_utc: 1_700_000_000_000,
            app_version: "0.3.8".to_string(),
            items: vec![
                BackupItem {
                    kind: "text".to_string(),
                    content: "回滚前写入的文本条目".to_string(),
                    html: None,
                    is_favorite: false,
                    created_utc: 1,
                    last_modified_utc: 2,
                    image: None,
                },
                BackupItem {
                    kind: "image".to_string(),
                    content: "[图片] 2×2".to_string(),
                    html: None,
                    is_favorite: false,
                    created_utc: 3,
                    last_modified_utc: 4,
                    image: Some(BackupImage {
                        image_hash: "0011223344556677".to_string(),
                        original_present: true,
                        // 非法 base64：解码失败必须让整批回滚
                        thumbnail_base64: "!!!not-base64!!!".to_string(),
                        original_base64: None,
                    }),
                },
            ],
        };

        // Act
        let result = merge_backup_payload(&conn, &payload);

        // Assert
        assert!(result.is_err(), "坏图片条目必须让合并失败");
        assert_eq!(
            db::count_live(&conn).expect("计数应成功"),
            0,
            "SAVEPOINT 必须把已插入的文本条目一并回滚"
        );
    }

    /// 备份文件名：带 UTC 时间戳、固定扩展名；同秒重复导出自动加序号不覆盖
    #[test]
    fn backup_file_name_is_timestamped_and_never_overwrites() {
        // Arrange
        let dir = tempfile::tempdir().expect("临时目录");
        let conn = make_db(&dir);

        // Act: 同一时间戳连续导出两次
        let first = export_backup_to_dir(&conn, PASSPHRASE, dir.path(), 1_700_000_000_000)
            .expect("首次导出应成功");
        let second = export_backup_to_dir(&conn, PASSPHRASE, dir.path(), 1_700_000_000_000)
            .expect("二次导出应成功");

        // Assert: 1700000000000 ms = 2023-11-14 22:13:20 UTC（独立于实现的已知换算）
        let first_name = Path::new(&first.path)
            .file_name()
            .and_then(|n| n.to_str())
            .expect("路径应含文件名");
        assert_eq!(first_name, "QuickQuick-Backup-20231114-221320.qqbp");
        assert_ne!(
            second.path, first.path,
            "同秒导出必须换名，不得覆盖既有备份"
        );
        assert_eq!(
            Path::new(&second.path).file_name().and_then(|n| n.to_str()),
            Some("QuickQuick-Backup-20231114-221320-2.qqbp")
        );
        assert!(Path::new(&first.path).is_file() && Path::new(&second.path).is_file());

        // 临时文件不得残留：改名成功后目录里只剩正式备份包
        let leftovers: Vec<String> = std::fs::read_dir(dir.path())
            .expect("读目录应成功")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".part"))
            .collect();
        assert!(leftovers.is_empty(), "不应残留临时文件：{leftovers:?}");
    }

    /// 导出结果 DTO 不含口令（口令原文不得进入任何返回结构）
    #[test]
    fn export_dto_does_not_contain_passphrase() {
        // Arrange
        let dir = tempfile::tempdir().expect("临时目录");
        let conn = make_db(&dir);

        // Act
        let dto = export_backup_to_dir(&conn, PASSPHRASE, dir.path(), 1_700_000_000_000)
            .expect("导出应成功");

        // Assert: Debug + 序列化两种呈现都不含口令脏值
        let debug = format!("{dto:?}");
        let json = serde_json::to_string(&dto).expect("DTO 应可序列化");
        assert!(!debug.contains(PASSPHRASE), "Debug 输出不得含口令");
        assert!(!json.contains(PASSPHRASE), "JSON 输出不得含口令");
    }

    /// 「在文件管理器中显示」只放行已存在的 .qqbp 文件
    #[test]
    fn reveal_path_rejects_foreign_or_missing_files() {
        // Arrange
        let dir = tempfile::tempdir().expect("临时目录");
        let foreign = dir.path().join("notes.txt");
        std::fs::write(&foreign, b"hi").expect("写文件应成功");
        let missing = dir.path().join("nope.qqbp");

        // Act & Assert
        assert!(
            validate_reveal_path(&foreign).is_err(),
            "非 .qqbp 文件必须被拒绝"
        );
        assert!(
            validate_reveal_path(&missing).is_err(),
            "不存在的文件必须被拒绝"
        );

        let legit = dir.path().join("QuickQuick-Backup-20231114-221320.qqbp");
        std::fs::write(&legit, b"QQBP").expect("写文件应成功");
        assert!(validate_reveal_path(&legit).is_ok(), "合法备份文件应放行");
    }

    /// 文件名时间戳换算：与已知 UTC 时刻对齐（独立于实现的固定向量）
    #[test]
    fn civil_from_epoch_ms_matches_known_utc_instants() {
        // Act & Assert
        assert_eq!(civil_from_epoch_ms(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(
            civil_from_epoch_ms(1_700_000_000_000),
            (2023, 11, 14, 22, 13, 20)
        );
        // 闰日：2024-02-29T12:34:56Z
        assert_eq!(
            civil_from_epoch_ms(1_709_210_096_000),
            (2024, 2, 29, 12, 34, 56)
        );
    }

    /// 超上限的备份包：在解密与写库之前就被拒绝，库内条目数不变
    ///
    /// 上限通过 `import_backup_bytes_with_limit` 注入一个小值来构造超限场景
    /// （真造 1 GiB 文件会让单测本身把内存打爆），两条断言分别锚定：
    /// 1. 报的是体积错误而不是口令错误 → 体积检查排在解密之前；
    /// 2. 库内条目数/快照不变 → 没有进入写库阶段（更没进 SAVEPOINT）。
    #[test]
    fn import_rejects_oversized_blob_before_decrypting() {
        // Arrange
        let source_dir = tempfile::tempdir().expect("临时目录");
        let target_dir = tempfile::tempdir().expect("临时目录");
        let source = make_db(&source_dir);
        seed_source_db(&source);
        let exported =
            export_backup_to_dir(&source, PASSPHRASE, source_dir.path(), 1_700_000_000_000)
                .expect("导出应成功");
        let blob = std::fs::read(&exported.path).expect("备份文件应可读");

        let target = make_db(&target_dir);
        db::ingest(
            &target,
            &CapturedItem {
                text: "目标库原有条目".to_string(),
                html: None,
            },
        )
        .expect("目标库入库应成功");
        let before = business_snapshot(&target);

        // Act: 上限比真实字节数少 1 → 超限
        let result = import_backup_bytes_with_limit(&target, &blob, PASSPHRASE, blob.len() - 1);

        // Assert
        let message = result.expect_err("超限备份必须被拒绝");
        assert!(message.contains("备份文件过大"), "实际错误：{message}");
        assert!(
            !message.contains("口令错误"),
            "体积检查必须排在解密之前，实际错误：{message}"
        );
        assert_eq!(
            db::count_live(&target).expect("计数应成功"),
            1,
            "不得写入任何条目"
        );
        assert_eq!(business_snapshot(&target), before, "不得改动既有条目");
    }

    /// 上限是闭区间：恰好等于上限的备份包正常导入（不是 >= 就拒）
    #[test]
    fn import_size_limit_boundary_is_inclusive() {
        // Arrange
        let source_dir = tempfile::tempdir().expect("临时目录");
        let target_dir = tempfile::tempdir().expect("临时目录");
        let source = make_db(&source_dir);
        seed_source_db(&source);
        let exported =
            export_backup_to_dir(&source, PASSPHRASE, source_dir.path(), 1_700_000_000_000)
                .expect("导出应成功");
        let blob = std::fs::read(&exported.path).expect("备份文件应可读");
        let target = make_db(&target_dir);

        // Act: 上限恰好等于真实字节数
        let result = import_backup_bytes_with_limit(&target, &blob, PASSPHRASE, blob.len());

        // Assert
        assert_eq!(result.expect("恰好等于上限应放行").inserted, 3);
    }

    /// 生产上限常量走的是同一套边界逻辑（1 GiB 处放行、多 1 字节拒绝）
    #[test]
    fn backup_blob_limit_boundary_uses_production_constant() {
        // Assert: 边界行为挂在真实常量上，而不是测试里另造的数字
        assert!(ensure_blob_within_limit(MAX_BACKUP_BLOB_BYTES, MAX_BACKUP_BLOB_BYTES).is_ok());
        let message = ensure_blob_within_limit(MAX_BACKUP_BLOB_BYTES + 1, MAX_BACKUP_BLOB_BYTES)
            .expect_err("超 1 字节必须被拒绝");
        assert!(message.contains("备份文件过大"), "实际错误：{message}");
        assert!(
            message.contains("1024 MiB"),
            "错误里应带上限，便于诊断：{message}"
        );
    }

    /// 目录决策：首选目录存在就用它，且不标记回退
    #[test]
    fn pick_backup_dir_prefers_first_existing_candidate() {
        // Arrange
        let root = tempfile::tempdir().expect("临时目录");
        let documents = root.path().join("Documents");
        let downloads = root.path().join("Downloads");
        std::fs::create_dir_all(&documents).expect("建目录应成功");
        std::fs::create_dir_all(&downloads).expect("建目录应成功");
        let fallback = root.path().join("AppData");

        // Act
        let (dir, used_fallback) =
            pick_backup_dir(vec![Some(documents.clone()), Some(downloads)], fallback);

        // Assert
        assert_eq!(dir, documents);
        assert!(!used_fallback, "首选目录可用时不应标记回退");
    }

    /// 目录决策：首选缺失时跳过它用下一个，不算回退
    #[test]
    fn pick_backup_dir_skips_missing_candidate() {
        // Arrange
        let root = tempfile::tempdir().expect("临时目录");
        let documents = root.path().join("Documents"); // 故意不创建
        let downloads = root.path().join("Downloads");
        std::fs::create_dir_all(&downloads).expect("建目录应成功");
        let fallback = root.path().join("AppData");

        // Act
        let (dir, used_fallback) =
            pick_backup_dir(vec![Some(documents), Some(downloads.clone())], fallback);

        // Assert
        assert_eq!(dir, downloads);
        assert!(!used_fallback, "仍落在系统约定目录内，不算回退");
    }

    /// 目录决策：候选全不可用时回退并标记，供前端解释路径为何不同
    #[test]
    fn pick_backup_dir_falls_back_when_all_candidates_missing() {
        // Arrange
        let root = tempfile::tempdir().expect("临时目录");
        let fallback = root.path().join("AppData");

        // Act
        let (dir, used_fallback) = pick_backup_dir(
            vec![Some(root.path().join("Documents")), None],
            fallback.clone(),
        );

        // Assert
        assert_eq!(dir, fallback);
        assert!(used_fallback, "回退到应用数据目录时必须标记");
    }

    /// DTO 的回退标记能被置位，且 JSON 键名是前端约定的 usedFallbackDir
    #[test]
    fn export_dto_reports_fallback_dir_with_camel_case_key() {
        // Arrange
        let dir = tempfile::tempdir().expect("临时目录");
        let conn = make_db(&dir);

        // Act
        let dto = export_backup_to_dir(&conn, PASSPHRASE, dir.path(), 1_700_000_000_000)
            .expect("导出应成功")
            .with_fallback_dir(true);

        // Assert
        assert!(dto.used_fallback_dir);
        let json = serde_json::to_string(&dto).expect("DTO 应可序列化");
        assert!(
            json.contains("\"usedFallbackDir\":true"),
            "实际 JSON：{json}"
        );
    }
}
