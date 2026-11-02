//! 剪贴板 IPC 命令层
//!
//! 模式：每个命令 = 薄的 `#[tauri::command]` 包装 + 可单测的纯函数 impl。
//! 单测只测 impl 函数（传 `&Connection`），命令层把 `DbError` 映射为 `String`。
//!
//! 命令清单（前端通过 invoke 对应的命令名调用）：
//! - `list_clip_items`           — 返回全部未软删条目（收藏优先，组内顺序见 `sort_order` 设置），
//!   图片条目只带 `hasThumbnail` 标志
//! - `delete_clip_item`          — 软删指定 id 的条目
//! - `toggle_favorite_clip`      — 设置或取消指定条目的收藏状态
//! - `get_clip_image_original`   — 按 image_id 取原图，返回 PNG data URL
//! - `get_clip_thumbnail`        — 按 image_id 取缩略图，返回 WebP data URL（列表按需调用）
//! - `get_clip_image_preview`    — 按 image_id 取原图并缩放为 WebP 预览图（Popover 大图预览按需调用）

use base64::Engine;
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::db::{self, DbError};
use crate::ipc::{with_db, AppDb};
use crate::settings::SortOrder;

/// 前端展示用 DTO：剪贴板条目
///
/// 字段用 camelCase 序列化，与前端 TypeScript 接口对齐。
/// 缩略图**不进**这个结构：列表每次刷新都会重传一遍 base64，
/// 图片条目只带 `hasThumbnail` 标志，字节由 `get_clip_thumbnail` 按需取。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItemDto {
    pub id: String,
    pub content: String,
    pub kind: String,
    pub is_favorite: bool,
    pub last_modified_utc: i64,
    /// 首次入库时间（序列化为 `createdUtc`）。
    ///
    /// 弹窗在「首次复制」排序下按它划分「今天/更早」，使分组口径与排序口径一致；
    /// 「最近复制」排序下仍按 `lastModifiedUtc` 分组（默认行为不变）。
    pub created_utc: i64,
    /// 关联图片行是否存在非空缩略图；文本条目为 `false`。
    pub has_thumbnail: bool,
    /// 关联 clip_images 行的 id；文本条目为 `None`。
    pub image_id: Option<String>,
    /// 富文本 HTML 串（序列化为 `htmlContent`）；纯文本/图片条目为 `None`。
    /// 透出供前端富文本预览与粘贴还原格式。
    pub html_content: Option<String>,
}

/// list_clip_items 的纯函数实现（默认排序：最近复制优先），可在测试中直接调用。
///
/// 等价于 [`list_clip_items_ordered_impl`] 传 [`SortOrder::Recent`]。单独保留这个入口是
/// 为了让「按最近取数」的调用方（如翻译取当前文本）**不随用户的展示排序偏好漂移**。
///
/// # Errors
/// 数据库查询失败时返回 `DbError`。
pub fn list_clip_items_impl(conn: &Connection) -> Result<Vec<ClipItemDto>, DbError> {
    list_clip_items_ordered_impl(conn, SortOrder::Recent)
}

/// list_clip_items 的纯函数实现：按给定排序方式返回所有未软删条目。
///
/// 排序只在**已取回的行**上做一次稳定重排，不改写 SQL：
/// - [`SortOrder::Recent`]：直接用库里的 `(is_favorite DESC, last_modified_utc DESC, rowid DESC)`，
///   与引入本设置之前的列表顺序逐条一致，也不做任何额外排序；
/// - [`SortOrder::FirstCopied`]：按 `(is_favorite DESC, created_utc DESC)` 稳定重排，
///   同一内容再次复制只推进 `last_modified_utc`，位置不动。
///
/// 为什么不在 SQL 里换 ORDER BY 列：列表主查询的索引是
/// `(is_deleted, is_favorite, last_modified_utc)`，改按 `created_utc` 排序会退化成
/// 全表扫 + 临时 B-tree；而新建索引要动迁移与 schema，超出本项范围。
/// 行数 = 历史上限（默认 500、最高 999）+ 收藏条目数（收藏既不计数也不被淘汰，见 P4-4），
/// 因此行数并无硬上界；内存里一次稳定排序的代价仍可忽略（O(n log n)）。
///
/// 图片条目的 `has_thumbnail` 透传自图片行、`image_id` 透传；
/// 文本条目两者分别为 `false` / `None`。
///
/// 这里刻意不做 base64 编码：列表 payload 每多 1 KB，每次刷新就多传 1 KB。
///
/// 也刻意**不带**库里预计算的 `title`：`content` 目前仍然全量随列表下发，
/// 再带一份 title 只会让 payload 变大而信息量为零（2000 条实测：无条件带 title
/// 让列表 JSON +82.8%）。等列表改成分段加载、正文不再随每次刷新传输时，
/// title 才作为「列表只传它、正文按需取」的那一半接进来。
///
/// # Errors
/// 数据库查询失败时返回 `DbError`。
pub fn list_clip_items_ordered_impl(
    conn: &Connection,
    order: SortOrder,
) -> Result<Vec<ClipItemDto>, DbError> {
    let mut rows = db::list_items_with_image_flag(conn)?;
    sort_rows_for_order(&mut rows, order);
    let dtos = rows
        .into_iter()
        .map(|r| ClipItemDto {
            id: r.id,
            content: r.content,
            kind: r.kind,
            is_favorite: r.is_favorite,
            last_modified_utc: r.last_modified_utc,
            created_utc: r.created_utc,
            has_thumbnail: r.has_thumbnail,
            image_id: r.image_id,
            html_content: r.html_content,
        })
        .collect();
    Ok(dtos)
}

/// 按排序方式重排列表行（原地、稳定）。
///
/// 入参必须已经是库里的默认顺序：`Recent` 直接原样返回，`FirstCopied` 依赖
/// 「同 key 的行沿用默认顺序」这一兜底（稳定排序，不额外比较 rowid）。
fn sort_rows_for_order(rows: &mut [db::ClipItemRowWithImageFlag], order: SortOrder) {
    if order != SortOrder::FirstCopied {
        return;
    }
    rows.sort_by(|a, b| {
        b.is_favorite
            .cmp(&a.is_favorite)
            .then_with(|| b.created_utc.cmp(&a.created_utc))
    });
}

/// 取「当前应翻译的剪贴板文本」：复用 list_clip_items_impl 的排序与口径，
/// 取首条（收藏优先、组内最近），仅 text/richtext 且 trim 非空才返回 Some。
///
/// 与前端 src/trans-popover/source-text.ts 的 pickLatestText 同语义（跨语言镜像，
/// 故意复用同一取数函数保证排序口径一致，避免两边漂移）。
///
/// 刻意固定走 [`SortOrder::Recent`]：「当前该翻译哪条」是取数语义，不是展示偏好。
/// 用户把列表切成「首次复制」时，翻译源仍应是最近复制的内容。
///
/// # Errors
/// 数据库查询失败时返回 `DbError`。
pub fn pick_latest_translate_text_impl(conn: &Connection) -> Result<Option<String>, DbError> {
    let items = list_clip_items_impl(conn)?;
    let Some(item) = items.first() else {
        return Ok(None);
    };
    if item.kind != "text" && item.kind != "richtext" {
        return Ok(None);
    }
    let trimmed = item.content.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed.to_string()))
}

/// delete_clip_item 的纯函数实现，可在测试中直接调用。
///
/// 对 id 做前置校验：空串或全空白直接返回 Err，不执行 SQL。
/// 这样避免无效 id 静默通过，也保证不会因空 id 触发非预期行为。
///
/// # Errors
/// - id 为空或全空白时返回 `DbError::Other`
/// - 数据库操作失败时返回 `DbError::Sqlite`
pub fn delete_clip_item_impl(conn: &Connection, id: &str) -> Result<(), DbError> {
    validate_id(id)?;
    db::soft_delete(conn, id)
}

/// toggle_favorite_clip 的纯函数实现，可在测试中直接调用。
///
/// 对 id 做前置校验：空串或全空白直接返回 Err，不执行 SQL。
///
/// # Errors
/// - id 为空或全空白时返回 `DbError::Other`
/// - 数据库操作失败时返回 `DbError::Sqlite`
pub fn toggle_favorite_clip_impl(
    conn: &Connection,
    id: &str,
    favorite: bool,
) -> Result<(), DbError> {
    validate_id(id)?;
    db::set_favorite(conn, id, favorite)
}

/// 校验 id 参数：空串或全空白视为非法输入。
///
/// 为何在命令层校验而非 db 层：db 层函数语义是「执行 SQL」，
/// 合法/非法输入判定属于 API 契约，应在边界层（IPC 命令层）强制。
fn validate_id(id: &str) -> Result<(), DbError> {
    if id.trim().is_empty() {
        return Err(DbError::Other("id 不能为空或全空白".to_string()));
    }
    Ok(())
}

/// Tauri 命令：列出所有未软删的剪贴板条目（收藏优先，组内顺序按 `sort_order` 设置）。
#[tauri::command]
pub fn list_clip_items(
    app: AppHandle,
    state: State<'_, AppDb>,
) -> Result<Vec<ClipItemDto>, String> {
    let order = resolve_sort_order(&app);
    with_db(&state, |conn| {
        list_clip_items_ordered_impl(conn, order).map_err(|e| e.to_string())
    })
}

/// 读当前排序方式；配置目录解析失败时回退默认（最近复制）。
///
/// 排序是纯展示偏好，读不到设置不该让整个列表加载失败——
/// 按默认顺序把列表给出去，比「设置文件路径算不出来 → 剪贴板空白」好得多。
fn resolve_sort_order(app: &AppHandle) -> SortOrder {
    match crate::ipc::settings::resolve_config_path(app, "settings.json") {
        Ok(path) => crate::ipc::settings::read_sort_order_from_path(&path),
        Err(e) => {
            eprintln!("[QuickQuick] 读取排序设置失败，回退最近复制顺序：{e}");
            SortOrder::default()
        }
    }
}

/// Tauri 命令：软删指定剪贴板条目。
#[tauri::command]
pub fn delete_clip_item(state: State<'_, AppDb>, id: String) -> Result<(), String> {
    with_db(&state, |conn| {
        delete_clip_item_impl(conn, &id).map_err(|e| e.to_string())
    })
}

/// Tauri 命令：设置或取消剪贴板条目的收藏状态。
#[tauri::command]
pub fn toggle_favorite_clip(
    state: State<'_, AppDb>,
    id: String,
    favorite: bool,
) -> Result<(), String> {
    with_db(&state, |conn| {
        toggle_favorite_clip_impl(conn, &id, favorite).map_err(|e| e.to_string())
    })
}

/// get_clip_image_original 的纯函数实现，可在测试中直接调用。
///
/// 按 image_id 取原图 BLOB，编码为 data URL（`data:image/png;base64,{b64}`）。
/// 未找到或已软删时返回 `Ok(None)`。
///
/// # Errors
/// 数据库查询失败时返回 `DbError`。
pub fn get_clip_image_original_impl(
    conn: &Connection,
    image_id: &str,
) -> Result<Option<String>, DbError> {
    use base64::Engine;
    match crate::image::get_image_original(conn, image_id)? {
        Some(bytes) if !bytes.is_empty() => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Ok(Some(format!("data:image/png;base64,{b64}")))
        }
        // None 或空 BLOB（降级图 original_present=0 时写入 X''）均视为无原图，前端回退缩略图
        _ => Ok(None),
    }
}

/// Tauri 命令：取原图并返回 PNG data URL。
///
/// image_id 对应 clip_images 表中的 id（非 clip_items.id）。
/// 未找到或已软删时返回 `Ok(None)`。
#[tauri::command]
pub fn get_clip_image_original(
    state: State<'_, AppDb>,
    image_id: String,
) -> Result<Option<String>, String> {
    with_db(&state, |conn| {
        get_clip_image_original_impl(conn, &image_id).map_err(|e| e.to_string())
    })
}

/// get_clip_thumbnail 的纯函数实现，可在测试中直接调用。
///
/// 按 image_id 取缩略图 BLOB，编码为 data URL（`data:image/webp;base64,{b64}`）。
/// 未找到、已软删或 BLOB 为空时返回 `Ok(None)`——前端据此保持 `[图片]` 占位，
/// 不把「这张图没有缩略图」当成错误上报（列表里总有降级图与历史脏数据）。
///
/// 取数复用 `image::get_image_thumbnail`，与列表/清理路径同一条 SQL，不另写一份。
///
/// # Errors
/// 数据库查询失败时返回 `DbError`。
pub fn get_clip_thumbnail_impl(
    conn: &Connection,
    image_id: &str,
) -> Result<Option<String>, DbError> {
    match crate::image::get_image_thumbnail(conn, image_id)? {
        Some(bytes) if !bytes.is_empty() => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Ok(Some(format!("data:image/webp;base64,{b64}")))
        }
        _ => Ok(None),
    }
}

/// 把原图字节编码为 Popover 大图预览用的 WebP data URL。
///
/// 纯计算（解码 + 缩放 + 编码），不碰数据库：调用方应先释放 DB 锁再调用，
/// 避免大图解码期间阻塞其他 IPC。
///
/// 无原图（`None` 或空 BLOB，即降级图）返回 `Ok(None)`，前端据此回落缩略图。
/// 原图存在但解码失败返回 `Err`：这是数据异常，错误要带上下文上报，而不是静默吞掉。
///
/// # Errors
/// 原图无法解码或 WebP 编码失败时返回带上下文的错误文案。
pub fn encode_preview_data_url(original: Option<Vec<u8>>) -> Result<Option<String>, String> {
    let bytes = match original {
        Some(bytes) if !bytes.is_empty() => bytes,
        _ => return Ok(None),
    };
    let webp = crate::image::make_preview(&bytes).map_err(|e| format!("预览生成失败: {e}"))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&webp);
    Ok(Some(format!("data:image/webp;base64,{b64}")))
}

/// Tauri 命令：取原图并生成 Popover 大图预览（WebP data URL，最长边有上限）。
///
/// DB 锁只在读取原图字节时持有；解码与编码放到阻塞线程池执行，
/// 避免大图处理卡住 IPC 线程。image_id 对应 clip_images 行 ID。
/// 无原图时返回 `Ok(None)`，前端回落到缩略图。
#[tauri::command]
pub async fn get_clip_image_preview(
    state: State<'_, AppDb>,
    image_id: String,
) -> Result<Option<String>, String> {
    let original = with_db(&state, |conn| {
        crate::image::get_image_original(conn, &image_id).map_err(|e| e.to_string())
    })?;
    tauri::async_runtime::spawn_blocking(move || encode_preview_data_url(original))
        .await
        .map_err(|e| format!("预览生成任务异常结束: {e}"))?
}

/// Tauri 命令：取缩略图并返回 WebP data URL。
///
/// image_id 对应 clip_images 表中的 id（非 clip_items.id）。
/// 未找到、已软删或缩略图为空时返回 `Ok(None)`。
#[tauri::command]
pub fn get_clip_thumbnail(
    state: State<'_, AppDb>,
    image_id: String,
) -> Result<Option<String>, String> {
    with_db(&state, |conn| {
        get_clip_thumbnail_impl(conn, &image_id).map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{ingest, ingest_image_as_clip};

    /// 构造最小合法 PNG（用 image crate 编码，不依赖外部文件）。
    fn make_test_png(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageFormat, RgbaImage};
        let img = RgbaImage::new(width, height);
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .expect("test PNG 编码失败");
        buf
    }

    /// 建立内存 SQLite，初始化 clip_items + clip_images 表（与 ensure_schema 一致）。
    fn make_test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("内存库开启失败");
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

    /// T1：list_clip_items_impl — 列表 DTO 不再内联缩略图 base64。
    ///
    /// 三件事：图片条目 `hasThumbnail=true` 且 `imageId` 非 None；文本条目两者为 `false`/`None`；
    /// 整份列表序列化后不含 `data:`（只断言字段名消失的话，将来谁把 base64 换个字段名
    /// 塞回 payload 就测不出来，所以直接扫 JSON 串）。
    #[test]
    fn list_clip_items_impl_drops_inline_thumbnail_base64() {
        let conn = make_test_conn();
        let png = make_test_png(2, 2);

        // 写入文本条目
        let clip_item = crate::clipboard::CapturedItem {
            text: "hello".to_string(),
            html: None,
        };
        ingest(&conn, &clip_item).expect("文本条目入库失败");

        // 写入图片条目
        ingest_image_as_clip(&conn, 2, 2, &png, 20 * 1024 * 1024).expect("图片条目入库失败");

        let dtos = list_clip_items_impl(&conn).expect("list_clip_items_impl 不应失败");
        assert!(!dtos.is_empty(), "应有条目返回");

        let image_dto = dtos
            .iter()
            .find(|d| d.kind == "image")
            .expect("应有图片条目");
        let text_dto = dtos
            .iter()
            .find(|d| d.kind == "text")
            .expect("应有文本条目");

        // 图片条目：有缩略图标志为真，imageId 非 None
        assert!(image_dto.has_thumbnail, "图片条目 hasThumbnail 应为 true");
        assert!(image_dto.image_id.is_some(), "图片条目 imageId 不应为 None");

        // 文本条目：无缩略图、无 imageId
        assert!(!text_dto.has_thumbnail, "文本条目 hasThumbnail 应为 false");
        assert!(text_dto.image_id.is_none(), "文本条目 imageId 应为 None");

        // 序列化硬证据：payload 里不得出现任何 data URL
        let json = serde_json::to_string(&dtos).expect("列表 DTO 序列化失败");
        assert!(
            !json.contains("data:"),
            "列表 JSON 不得含 data URL（缩略图必须按需拉取），实际：{json}"
        );
        assert!(
            !json.contains("base64"),
            "列表 JSON 不得含 base64 内容，实际：{json}"
        );
        assert!(
            json.contains("\"hasThumbnail\":true"),
            "图片条目的 hasThumbnail 必须以 camelCase 出现在 JSON 里，实际：{json}"
        );
    }

    /// T1b：缩略图 BLOB 为空（脏数据 / 降级写入残留）时 `hasThumbnail` 必须为 false。
    ///
    /// 若 SQL 只判 `IS NOT NULL`，前端会为一个永远取不到图的 id 反复开请求。
    #[test]
    fn list_clip_items_impl_reports_no_thumbnail_for_empty_blob() {
        let conn = make_test_conn();

        conn.execute(
            "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc)
             VALUES ('item-empty-thumb', NULL, 'image', 1000, 1000)",
            [],
        )
        .expect("clip_items 插入失败");
        conn.execute(
            "INSERT INTO clip_images
                 (id, clip_item_id, thumbnail, created_utc, last_modified_utc)
             VALUES ('img-empty-thumb', 'item-empty-thumb', X'', 1000, 1000)",
            [],
        )
        .expect("clip_images 插入失败");

        let dtos = list_clip_items_impl(&conn).expect("list_clip_items_impl 不应失败");
        let dto = dtos
            .iter()
            .find(|d| d.id == "item-empty-thumb")
            .expect("应有该图片条目");
        assert!(
            !dto.has_thumbnail,
            "零字节缩略图必须判为「没有缩略图」，否则前端会为它反复请求"
        );
        assert!(
            dto.image_id.is_some(),
            "hasThumbnail=false 不代表没有图片行，imageId 仍应透传"
        );
    }

    /// T2：get_clip_image_original_impl — 存在的 image_id 返回 Some 且以 PNG data URL 前缀开头。
    #[test]
    fn get_clip_image_original_impl_returns_data_url_for_existing_image() {
        let conn = make_test_conn();
        let png = make_test_png(2, 2);

        ingest_image_as_clip(&conn, 2, 2, &png, 20 * 1024 * 1024).expect("图片入库失败");

        // 从 clip_images 查 image_id
        let image_id: String = conn
            .query_row(
                "SELECT id FROM clip_images WHERE is_deleted = 0 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("应能查到 clip_images 行");

        let result = get_clip_image_original_impl(&conn, &image_id).expect("不应返回 Err");
        let data_url = result.expect("存在的 image_id 应返回 Some");
        assert!(
            data_url.starts_with("data:image/png;base64,"),
            "应以 data:image/png;base64, 开头，实际：{data_url}"
        );
    }

    /// T3：get_clip_image_original_impl — 不存在的 image_id 返回 Ok(None)。
    #[test]
    fn get_clip_image_original_impl_returns_none_for_missing_id() {
        let conn = make_test_conn();
        let result = get_clip_image_original_impl(&conn, "non-existent-id").expect("不应返回 Err");
        assert!(result.is_none(), "不存在的 id 应返回 None");
    }

    /// T4：get_clip_image_original_impl — 降级图（original_present=0，original=空 BLOB）应返回 Ok(None)，
    /// 而不是 Some("data:image/png;base64,")，前端依赖此行为回退到缩略图。
    #[test]
    fn get_clip_image_original_impl_returns_none_for_downgraded_image() {
        let conn = make_test_conn();
        let thumbnail = make_test_png(4, 4);

        // 手动 INSERT 降级图：original_present=0，original=空 BLOB
        conn.execute(
            "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc)
             VALUES ('item-downgrade', NULL, 'image', 1000, 1000)",
            [],
        )
        .expect("clip_items 插入失败");
        conn.execute(
            "INSERT INTO clip_images
                 (id, clip_item_id, thumbnail, original, original_present, created_utc, last_modified_utc)
             VALUES ('img-downgrade', 'item-downgrade', ?1, X'', 0, 1000, 1000)",
            rusqlite::params![thumbnail],
        )
        .expect("clip_images 插入失败");

        let result = get_clip_image_original_impl(&conn, "img-downgrade").expect("不应返回 Err");
        assert!(
            result.is_none(),
            "降级图（空 BLOB）应返回 None，实际返回：{result:?}"
        );
    }

    /// T4b：get_clip_thumbnail_impl — 存在的 image_id 返回 Some 且以 WebP data URL 前缀开头。
    #[test]
    fn get_clip_thumbnail_impl_returns_webp_data_url_for_existing_image() {
        let conn = make_test_conn();
        let png = make_test_png(2, 2);
        ingest_image_as_clip(&conn, 2, 2, &png, 20 * 1024 * 1024).expect("图片入库失败");

        let image_id: String = conn
            .query_row(
                "SELECT id FROM clip_images WHERE is_deleted = 0 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("应能查到 clip_images 行");

        let result = get_clip_thumbnail_impl(&conn, &image_id).expect("不应返回 Err");
        let data_url = result.expect("存在的 image_id 应返回 Some");
        assert!(
            data_url.starts_with("data:image/webp;base64,"),
            "缩略图应以 data:image/webp;base64, 开头，实际：{data_url}"
        );
        let payload = data_url
            .strip_prefix("data:image/webp;base64,")
            .expect("前缀已断言");
        assert!(
            !payload.is_empty(),
            "data URL 的 base64 段不应为空（空串意味着取到了零字节缩略图却当成成功）"
        );
    }

    /// T4c：get_clip_thumbnail_impl — 不存在的 image_id 返回 Ok(None)，不报错。
    #[test]
    fn get_clip_thumbnail_impl_returns_none_for_missing_id() {
        let conn = make_test_conn();
        let result = get_clip_thumbnail_impl(&conn, "non-existent-id").expect("不应返回 Err");
        assert!(result.is_none(), "不存在的 id 应返回 None");
    }

    /// T4d：get_clip_thumbnail_impl — 已软删的图片行返回 Ok(None)。
    ///
    /// 前置断言该行缩略图非空：否则「软删返回 None」与「本来就没有缩略图」无法区分，
    /// 把 `is_deleted = 0` 从 SQL 里删掉这个用例也仍然全绿（假绿）。
    #[test]
    fn get_clip_thumbnail_impl_returns_none_for_soft_deleted_row() {
        let conn = make_test_conn();
        let thumbnail = make_test_png(4, 4);

        conn.execute(
            "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc)
             VALUES ('item-soft-thumb', NULL, 'image', 1000, 1000)",
            [],
        )
        .expect("clip_items 插入失败");
        conn.execute(
            "INSERT INTO clip_images
                 (id, clip_item_id, thumbnail, created_utc, last_modified_utc, is_deleted)
             VALUES ('img-soft-thumb', 'item-soft-thumb', ?1, 1000, 1000, 1)",
            rusqlite::params![thumbnail],
        )
        .expect("clip_images 插入失败");

        let thumb_len: i64 = conn
            .query_row(
                "SELECT length(thumbnail) FROM clip_images WHERE id = 'img-soft-thumb'",
                [],
                |row| row.get(0),
            )
            .expect("应能查到该行缩略图长度");
        assert!(thumb_len > 0, "前置条件：该行确实有非空缩略图");

        let result = get_clip_thumbnail_impl(&conn, "img-soft-thumb").expect("不应返回 Err");
        assert!(result.is_none(), "已软删行应返回 None，实际：{result:?}");
    }

    /// T4e：get_clip_thumbnail_impl — 零字节缩略图返回 Ok(None)，不返回空 data URL。
    #[test]
    fn get_clip_thumbnail_impl_returns_none_for_empty_blob() {
        let conn = make_test_conn();
        conn.execute(
            "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc)
             VALUES ('item-empty-thumb-2', NULL, 'image', 1000, 1000)",
            [],
        )
        .expect("clip_items 插入失败");
        conn.execute(
            "INSERT INTO clip_images
                 (id, clip_item_id, thumbnail, created_utc, last_modified_utc)
             VALUES ('img-empty-thumb-2', 'item-empty-thumb-2', X'', 1000, 1000)",
            [],
        )
        .expect("clip_images 插入失败");

        let result = get_clip_thumbnail_impl(&conn, "img-empty-thumb-2").expect("不应返回 Err");
        assert!(
            result.is_none(),
            "零字节缩略图应返回 None（前端保持占位），实际：{result:?}"
        );
    }

    /// T5：pick_latest_translate_text_impl — 文本条目返回 Some(trim 后的内容)。
    #[test]
    fn pick_latest_translate_text_impl_returns_trimmed_text_for_text_item() {
        let conn = make_test_conn();
        let clip_item = crate::clipboard::CapturedItem {
            text: "  hello world  ".to_string(),
            html: None,
        };
        ingest(&conn, &clip_item).expect("文本条目入库失败");

        let result = pick_latest_translate_text_impl(&conn)
            .expect("pick_latest_translate_text_impl 不应失败");
        assert_eq!(
            result,
            Some("hello world".to_string()),
            "文本条目应返回 trim 后的内容"
        );
    }

    /// T6：pick_latest_translate_text_impl — 图片条目返回 None（图片不可译）。
    #[test]
    fn pick_latest_translate_text_impl_returns_none_for_image_item() {
        let conn = make_test_conn();
        let png = make_test_png(2, 2);
        ingest_image_as_clip(&conn, 2, 2, &png, 20 * 1024 * 1024).expect("图片条目入库失败");

        let result = pick_latest_translate_text_impl(&conn)
            .expect("pick_latest_translate_text_impl 不应失败");
        assert!(result.is_none(), "图片条目应返回 None");
    }

    /// T7：pick_latest_translate_text_impl — 空库返回 None。
    #[test]
    fn pick_latest_translate_text_impl_returns_none_for_empty_db() {
        let conn = make_test_conn();

        let result = pick_latest_translate_text_impl(&conn)
            .expect("pick_latest_translate_text_impl 不应失败");
        assert!(result.is_none(), "空库应返回 None");
    }

    /// T8：pick_latest_translate_text_impl — 纯空白 content 返回 None。
    #[test]
    fn pick_latest_translate_text_impl_returns_none_for_whitespace_only() {
        let conn = make_test_conn();
        // 手动插入纯空白文本条目（ingest 可能拒收空白，故直接 INSERT 保证用例独立）
        conn.execute(
            "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc)
             VALUES ('item-ws', '   ', 'text', 1000, 1000)",
            [],
        )
        .expect("clip_items 插入失败");

        let result = pick_latest_translate_text_impl(&conn)
            .expect("pick_latest_translate_text_impl 不应失败");
        assert!(result.is_none(), "纯空白 content 应返回 None");
    }

    /// 插入一条文本条目：created 与 last_modified 分开给，用于构造
    /// 「首次复制先后」与「最近复制先后」不一致的数据（再次复制只推进 last_modified）。
    fn insert_item(
        conn: &Connection,
        id: &str,
        created_utc: i64,
        last_modified_utc: i64,
        is_favorite: bool,
    ) {
        conn.execute(
            "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc, is_favorite)
             VALUES (?1, ?2, 'text', ?3, ?4, ?5)",
            rusqlite::params![
                id,
                format!("content-{id}"),
                created_utc,
                last_modified_utc,
                if is_favorite { 1 } else { 0 }
            ],
        )
        .expect("clip_items 插入失败");
    }

    /// 取列表里的 id 序列，便于断言顺序。
    fn ids(dtos: &[ClipItemDto]) -> Vec<String> {
        dtos.iter().map(|d| d.id.clone()).collect()
    }

    /// 数据布局：A 最早创建、最近复制；B 次早创建、从未再复制；C 最新创建、最近复制。
    /// 「最近复制」顺序 = [C, A, B]，「首次复制」顺序 = [C, B, A]。
    fn insert_order_fixture(conn: &Connection) {
        insert_item(
            conn,
            "a-oldest-created-recently-copied",
            1_000,
            9_000,
            false,
        );
        insert_item(conn, "b-middle-created-never-recopied", 2_000, 2_000, false);
        insert_item(
            conn,
            "c-newest-created-recently-copied",
            3_000,
            8_000,
            false,
        );
    }

    /// 列表 DTO 必须带出首次入库时间（弹窗「首次复制」分组依赖它），
    /// 且它必须与 lastModified 可区分：再次复制只推进后者。
    #[test]
    fn list_clip_items_exposes_created_utc_distinct_from_last_modified() {
        let conn = make_test_conn();
        insert_item(&conn, "recopied-item", 1_000, 9_000, false);

        let dtos = list_clip_items_impl(&conn).expect("列表查询不应失败");

        assert_eq!(dtos[0].created_utc, 1_000, "首次入库时间应原样带出");
        assert_eq!(dtos[0].last_modified_utc, 9_000, "最近复制时间应原样带出");
        let json = serde_json::to_string(&dtos).expect("列表 DTO 序列化失败");
        assert!(
            json.contains("\"createdUtc\":1000"),
            "createdUtc 必须以 camelCase 出现在列表 JSON 里，实际：{json}"
        );
    }

    /// 默认排序（Recent）必须与引入本设置之前的列表顺序一致：最近复制在前。
    #[test]
    fn list_clip_items_recent_order_is_most_recently_copied_first() {
        let conn = make_test_conn();
        insert_order_fixture(&conn);

        let dtos = list_clip_items_impl(&conn).expect("列表查询不应失败");

        assert_eq!(
            ids(&dtos),
            vec![
                "a-oldest-created-recently-copied",
                "c-newest-created-recently-copied",
                "b-middle-created-never-recopied",
            ],
            "默认排序应按 last_modified_utc 降序（与改动前逐条一致）"
        );
    }

    /// 首次复制排序：按 created_utc 降序，再次复制（只推进 last_modified）不改变位置。
    #[test]
    fn list_clip_items_first_copied_order_ignores_recopy_bump() {
        let conn = make_test_conn();
        insert_order_fixture(&conn);

        let dtos =
            list_clip_items_ordered_impl(&conn, SortOrder::FirstCopied).expect("列表查询不应失败");

        assert_eq!(
            ids(&dtos),
            vec![
                "c-newest-created-recently-copied",
                "b-middle-created-never-recopied",
                "a-oldest-created-recently-copied",
            ],
            "首次复制排序应按 created_utc 降序，最近复制时间不参与"
        );
    }

    /// 两档排序在同一条数据上必须真的不同——否则「各有测试」是结构性空转。
    #[test]
    fn recent_and_first_copied_orders_differ_on_same_data() {
        let conn = make_test_conn();
        insert_order_fixture(&conn);

        let recent = ids(&list_clip_items_impl(&conn).expect("最近复制查询不应失败"));
        let first = ids(&list_clip_items_ordered_impl(&conn, SortOrder::FirstCopied)
            .expect("首次复制查询不应失败"));

        assert_ne!(
            recent, first,
            "两档排序必须产生不同顺序，否则测试无法区分分支，最近={recent:?} 首次={first:?}"
        );
    }

    /// 收藏优先在「首次复制」排序下同样成立：收藏项整体在前，不因 created 更旧而落到后面。
    #[test]
    fn first_copied_order_keeps_favorites_first() {
        let conn = make_test_conn();
        insert_item(&conn, "pinned-oldest", 500, 500, true);
        insert_item(&conn, "normal-newest", 9_000, 9_000, false);

        let dtos =
            list_clip_items_ordered_impl(&conn, SortOrder::FirstCopied).expect("列表查询不应失败");

        assert_eq!(
            ids(&dtos),
            vec!["pinned-oldest", "normal-newest"],
            "收藏项必须整体排在非收藏项之前（收藏优先级高于首次复制时间）"
        );
    }

    /// Recent 排序是**严格不重排**：库返回什么顺序就输出什么顺序。
    ///
    /// 用「created 与 last_modified 反向」的数据验证：若有人把 Recent 分支
    /// 也接上按 created 的重排，这里会红。
    #[test]
    fn recent_order_does_not_reorder_by_created() {
        let conn = make_test_conn();
        insert_item(&conn, "created-late-modified-early", 9_000, 1_000, false);
        insert_item(&conn, "created-early-modified-late", 1_000, 9_000, false);

        let dtos =
            list_clip_items_ordered_impl(&conn, SortOrder::Recent).expect("列表查询不应失败");

        assert_eq!(
            ids(&dtos),
            vec!["created-early-modified-late", "created-late-modified-early",],
            "最近复制排序只看 last_modified_utc"
        );
    }

    /// 翻译取数不受展示排序影响：即使「首次复制」会把老条目排到最前，
    /// 「当前该翻译的文本」仍取最近复制的那条。
    #[test]
    fn pick_latest_translate_text_ignores_first_copied_display_order() {
        let conn = make_test_conn();
        // 老条目：最早创建、最近复制；新条目：最近创建、很久没复制
        insert_item(&conn, "old-created-recent-copied", 1_000, 9_000, false);
        insert_item(&conn, "new-created-stale-copied", 5_000, 6_000, false);

        let first =
            ids(&list_clip_items_ordered_impl(&conn, SortOrder::FirstCopied)
                .expect("列表查询不应失败"));
        assert_eq!(
            first.first().map(String::as_str),
            Some("new-created-stale-copied"),
            "前提：首次复制排序会把最近创建的条目排到最前"
        );

        let picked = pick_latest_translate_text_impl(&conn).expect("取数不应失败");
        assert_eq!(
            picked.as_deref(),
            Some("content-old-created-recent-copied"),
            "翻译取数必须取最近复制的那条，不随展示排序漂移"
        );
    }

    /// 解码预览返回的 WebP data URL，返回其真实像素宽高（用于断言缩放结果）。
    fn decode_webp_dims(data_url: &str) -> (u32, u32) {
        let payload = data_url
            .strip_prefix("data:image/webp;base64,")
            .expect("应为 WebP data URL");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .expect("base64 应可解码");
        let decoded = webp::Decoder::new(&bytes)
            .decode()
            .expect("预览应是合法 WebP");
        (decoded.width(), decoded.height())
    }

    /// P1：超过上限的原图按最长边缩放到 PREVIEW_MAX_EDGE（1600），短边等比。
    #[test]
    fn encode_preview_scales_long_edge_to_cap_keeping_aspect_ratio() {
        let original = make_test_png(3000, 2000);

        let url = encode_preview_data_url(Some(original))
            .expect("合法原图不应报错")
            .expect("有原图时应返回 Some");

        assert_eq!(
            decode_webp_dims(&url),
            (1600, 1067),
            "3000×2000 按最长边 1600 等比缩放，短边 2000×1600/3000≈1067"
        );
    }

    /// P2：不超过上限的原图不放大，保持原始尺寸。
    #[test]
    fn encode_preview_does_not_upscale_small_image() {
        let original = make_test_png(120, 80);

        let url = encode_preview_data_url(Some(original))
            .expect("合法原图不应报错")
            .expect("有原图时应返回 Some");

        assert_eq!(decode_webp_dims(&url), (120, 80));
    }

    /// P3：无原图（None）或空 BLOB（降级图）返回 Ok(None)，前端据此回落缩略图。
    #[test]
    fn encode_preview_returns_none_without_original_bytes() {
        assert_eq!(encode_preview_data_url(None), Ok(None));
        assert_eq!(encode_preview_data_url(Some(Vec::new())), Ok(None));
    }

    /// P4：原图损坏无法解码时返回带上下文的 Err，不静默降级成 Ok(None)。
    #[test]
    fn encode_preview_reports_error_for_undecodable_original() {
        let err = encode_preview_data_url(Some(b"not an image".to_vec()))
            .expect_err("损坏原图必须返回 Err");

        assert!(
            err.starts_with("预览生成失败"),
            "错误应带上下文前缀，实际：{err}"
        );
    }

    /// P5：端到端取数链路——入库图片 → 取原图字节 → 生成预览，结果为 WebP 且受上限约束。
    #[test]
    fn stored_image_round_trips_into_capped_webp_preview() {
        let conn = make_test_conn();
        let png = make_test_png(2400, 1200);
        ingest_image_as_clip(&conn, 2400, 1200, &png, 20 * 1024 * 1024).expect("图片入库失败");
        let image_id: String = conn
            .query_row(
                "SELECT id FROM clip_images WHERE is_deleted = 0 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("应能查到 clip_images 行");

        let original = crate::image::get_image_original(&conn, &image_id).expect("取原图不应失败");
        let url = encode_preview_data_url(original)
            .expect("预览生成不应失败")
            .expect("有原图时应返回 Some");

        assert_eq!(decode_webp_dims(&url), (1600, 800));
    }
}
