//! 剪贴板 IPC 命令层
//!
//! 模式：每个命令 = 薄的 `#[tauri::command]` 包装 + 可单测的纯函数 impl。
//! 单测只测 impl 函数（传 `&Connection`），命令层把 `DbError` 映射为 `String`。
//!
//! 命令清单（前端通过 invoke 对应的命令名调用）：
//! - `list_clip_items`           — 返回全部未软删条目（收藏优先），图片条目只带 `hasThumbnail` 标志
//! - `delete_clip_item`          — 软删指定 id 的条目
//! - `toggle_favorite_clip`      — 设置或取消指定条目的收藏状态
//! - `get_clip_image_original`   — 按 image_id 取原图，返回 PNG data URL
//! - `get_clip_thumbnail`        — 按 image_id 取缩略图，返回 WebP data URL（列表按需调用）

use base64::Engine;
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::db::{self, DbError};
use crate::ipc::{with_db, AppDb};

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
    /// 关联图片行是否存在非空缩略图；文本条目为 `false`。
    pub has_thumbnail: bool,
    /// 关联 clip_images 行的 id；文本条目为 `None`。
    pub image_id: Option<String>,
    /// 富文本 HTML 串（序列化为 `htmlContent`）；纯文本/图片条目为 `None`。
    /// 透出供前端富文本预览与粘贴还原格式。
    pub html_content: Option<String>,
}

/// list_clip_items 的纯函数实现，可在测试中直接调用。
///
/// 返回所有未软删条目，按收藏优先、组内最近修改时间降序排列。
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
pub fn list_clip_items_impl(conn: &Connection) -> Result<Vec<ClipItemDto>, DbError> {
    let rows = db::list_items_with_image_flag(conn)?;
    let dtos = rows
        .into_iter()
        .map(|r| ClipItemDto {
            id: r.id,
            content: r.content,
            kind: r.kind,
            is_favorite: r.is_favorite,
            last_modified_utc: r.last_modified_utc,
            has_thumbnail: r.has_thumbnail,
            image_id: r.image_id,
            html_content: r.html_content,
        })
        .collect();
    Ok(dtos)
}

/// 取「当前应翻译的剪贴板文本」：复用 list_clip_items_impl 的排序与口径，
/// 取首条（收藏优先、组内最近），仅 text/richtext 且 trim 非空才返回 Some。
///
/// 与前端 src/trans-popover/source-text.ts 的 pickLatestText 同语义（跨语言镜像，
/// 故意复用同一取数函数保证排序口径一致，避免两边漂移）。
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

/// Tauri 命令：列出所有未软删的剪贴板条目（收藏优先）。
#[tauri::command]
pub fn list_clip_items(state: State<'_, AppDb>) -> Result<Vec<ClipItemDto>, String> {
    with_db(&state, |conn| {
        list_clip_items_impl(conn).map_err(|e| e.to_string())
    })
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
}
