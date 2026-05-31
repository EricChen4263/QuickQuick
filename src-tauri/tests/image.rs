//! 集成测试：图片捕获入库
//!
//! 覆盖验收项：
//! - V3-F1-A01 image_capture_lossless_split
//! - V3-F1-A02 thumbnail_spec_webp_256
//! - V3-F1-A03 oversize_skip_original

use quickquick_lib::image as img;
use tempfile::tempdir;
use quickquick_lib::db;

/// 固定 32 字节测试密钥，不依赖钥匙串
const TEST_KEY: [u8; 32] = [7u8; 32];

/// V3-F1-A01：图片入库——原图无损存、缩略图/原图拆分两字段、字节哈希判重
#[test]
fn image_capture_lossless_split_insert_dedup_and_different() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    let original_bytes: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x01, 0x02, 0x03, 0x04];
    let thumbnail_bytes: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x0A, 0x0B];

    // Act 1：首次入库
    let outcome1 = img::ingest_image(&conn, &original_bytes, &thumbnail_bytes)
        .expect("ingest_image 应成功");

    // Assert 1：首次入库返回 Inserted
    let first_id = match outcome1 {
        img::IngestImageOutcome::Inserted(ref id) => id.clone(),
        img::IngestImageOutcome::Bumped(_) => panic!("首次入库应返回 Inserted，实际返回 Bumped"),
    };

    // Assert 2：原图逐字节无损（原图和缩略图拆分存储，分别取回）
    let fetched_original = img::get_image_original(&conn, &first_id)
        .expect("get_image_original 应成功")
        .expect("应能取到原图");
    assert_eq!(
        fetched_original, original_bytes,
        "原图应逐字节与输入相同（无损存储）"
    );

    // Assert 3：缩略图逐字节与输入相同（拆分存储在独立字段）
    let fetched_thumbnail = img::get_image_thumbnail(&conn, &first_id)
        .expect("get_image_thumbnail 应成功")
        .expect("应能取到缩略图");
    assert_eq!(
        fetched_thumbnail, thumbnail_bytes,
        "缩略图应逐字节与输入相同（拆分存储在独立字段）"
    );

    // Assert 4：计数为 1
    let count1 = img::image_count(&conn).expect("image_count 应成功");
    assert_eq!(count1, 1, "首次入库后计数应为 1");

    // Act 2：相同 original 字节再次入库（字节哈希判重）
    let outcome2 = img::ingest_image(&conn, &original_bytes, &thumbnail_bytes)
        .expect("第二次 ingest_image 应成功");

    // Assert 5：相同原图字节返回 Bumped（不新建行）
    match outcome2 {
        img::IngestImageOutcome::Bumped(ref id) => {
            assert_eq!(
                id, &first_id,
                "Bumped 返回的 id 应与首次 Inserted 的 id 相同"
            );
        }
        img::IngestImageOutcome::Inserted(_) => {
            panic!("相同字节再次入库应返回 Bumped，实际返回 Inserted（字节哈希判重失效）")
        }
    }

    // Assert 6：计数仍为 1（未新建行）
    let count2 = img::image_count(&conn).expect("image_count 应成功");
    assert_eq!(count2, 1, "相同原图字节再次入库后计数应仍为 1（字节哈希判重）");

    // Act 3：不同 original 字节入库
    let different_original: Vec<u8> = vec![0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00];
    let outcome3 = img::ingest_image(&conn, &different_original, &thumbnail_bytes)
        .expect("第三次 ingest_image 应成功");

    // Assert 7：不同字节返回 Inserted
    match outcome3 {
        img::IngestImageOutcome::Inserted(_) => {}
        img::IngestImageOutcome::Bumped(_) => {
            panic!("不同原图字节应返回 Inserted，实际返回 Bumped")
        }
    }

    // Assert 8：计数变为 2
    let count3 = img::image_count(&conn).expect("image_count 应成功");
    assert_eq!(count3, 2, "不同原图字节入库后计数应为 2");
}

/// V3-F1-A02：缩略图规格——WebP / 最长边 ~256px(retina 320) / 质量 ~75
///
/// 用 image crate 程序生成一张 400x300 纯色 PNG 字节作为输入（宽边 > 256，触发缩放）。
/// 断言：① WebP 魔数（RIFF....WEBP）；② 解码缩略图后最长边 ≤ 320；③ 输出非空且可解码。
#[test]
fn thumbnail_spec_webp_256_format_and_size() {
    // Arrange：程序生成 400x300 纯橙色 PNG 字节（宽 > 256，触发最长边缩放）
    let src_png = make_test_png(400, 300, [255u8, 128u8, 0u8]);

    // Act
    let thumb_webp = img::make_thumbnail(&src_png).expect("make_thumbnail 应成功");

    // Assert 1：输出非空
    assert!(!thumb_webp.is_empty(), "缩略图不应为空");

    // Assert 2：WebP 魔数（RIFF + WEBP），前 4 字节 = b"RIFF"，第 8-12 字节 = b"WEBP"
    assert!(
        thumb_webp.len() >= 12,
        "WebP 文件至少 12 字节（含 RIFF+size+WEBP 头）"
    );
    assert_eq!(&thumb_webp[0..4], b"RIFF", "缩略图前 4 字节应为 RIFF（WebP 魔数）");
    assert_eq!(&thumb_webp[8..12], b"WEBP", "缩略图第 8-12 字节应为 WEBP（WebP 魔数）");

    // Assert 3：解码缩略图后最长边 ≤ 256（THUMB_MAX_EDGE 精确上限）
    let decoded = image::load_from_memory(&thumb_webp).expect("缩略图应可被 image crate 解码");
    let max_edge = decoded.width().max(decoded.height());
    assert!(
        max_edge <= 256,
        "缩略图最长边应 ≤ 256px（THUMB_MAX_EDGE），实际 {max_edge}px"
    );
}

/// V3-F1-A02 负向：损坏字节解码应返回 Err(ImageError::Decode)，不 panic，不 Ok
#[test]
fn make_thumbnail_returns_err_on_corrupt_bytes() {
    // Arrange：非法字节序列，无法被任何图片格式解码
    let corrupt = b"not_an_image";

    // Act
    let result = img::make_thumbnail(corrupt);

    // Assert：应为 Err，且是 Decode 变体（非 panic、非 Ok）
    assert!(result.is_err(), "损坏字节应返回 Err，实际返回 Ok");
    match result {
        Err(img::ImageError::Decode(_)) => {}
        Err(img::ImageError::Encode(_)) => {
            panic!("损坏字节应返回 Decode 错误，而非 Encode 错误");
        }
        Ok(_) => panic!("损坏字节不应返回 Ok"),
    }
}

/// V3-F1-A03：超大图处理——超阈值跳过原图、只留缩略图、标 original_present=0；阈值可配
///
/// 用小阈值（10 字节）验证可配性：传 20 字节 PNG 模拟超大图。
/// 正常大小路径（阈值 = usize::MAX）验证 original_present=1。
#[test]
fn oversize_skip_original_policy_configurable() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    // 生成一张最小 PNG（10×10 纯色），实际字节数通常 > 10 字节，触发小阈值
    let small_png = make_test_png(10, 10, [0u8, 128u8, 255u8]);

    // Act 1：小阈值（10 字节），small_png.len() > 10 → 应跳过原图
    let tiny_policy = img::OversizePolicy { max_original_bytes: 10 };
    let outcome_over = img::ingest_image_with_policy(&conn, &small_png, &tiny_policy)
        .expect("ingest_image_with_policy 应成功（超大跳过路径）");

    let id_over = match outcome_over {
        img::IngestImageOutcome::Inserted(ref id) => id.clone(),
        img::IngestImageOutcome::Bumped(ref id) => id.clone(),
    };

    // Assert 1：original_present = 0（跳过原图）
    let present_over = img::get_original_present(&conn, &id_over)
        .expect("get_original_present 应成功")
        .expect("应能取到行");
    assert_eq!(present_over, 0, "超阈值时 original_present 应为 0（原图过大未存）");

    // Assert 2：original BLOB 为空/NULL
    let orig_over = img::get_image_original(&conn, &id_over)
        .expect("get_image_original 应成功");
    assert!(
        orig_over.is_none_or(|v| v.is_empty()),
        "超阈值时原图 BLOB 应为空"
    );

    // Assert 3：缩略图仍在（非空）
    let thumb_over = img::get_image_thumbnail(&conn, &id_over)
        .expect("get_image_thumbnail 应成功")
        .expect("超阈值时缩略图应仍存在");
    assert!(!thumb_over.is_empty(), "超阈值时缩略图仍应非空");

    // Act 2：不同颜色图片 + 宽松阈值（usize::MAX），应正常存原图
    let another_png = make_test_png(10, 10, [255u8, 0u8, 0u8]);
    let big_policy = img::OversizePolicy { max_original_bytes: usize::MAX };
    let outcome_normal = img::ingest_image_with_policy(&conn, &another_png, &big_policy)
        .expect("ingest_image_with_policy 应成功（正常路径）");

    let id_normal = match outcome_normal {
        img::IngestImageOutcome::Inserted(ref id) => id.clone(),
        img::IngestImageOutcome::Bumped(ref id) => id.clone(),
    };

    // Assert 4：original_present = 1（正常存原图）
    let present_normal = img::get_original_present(&conn, &id_normal)
        .expect("get_original_present 应成功")
        .expect("应能取到行");
    assert_eq!(present_normal, 1, "未超阈值时 original_present 应为 1");

    // Assert 5：原图 BLOB 与输入相同（无损）
    let orig_normal = img::get_image_original(&conn, &id_normal)
        .expect("get_image_original 应成功")
        .expect("未超阈值时应能取到原图");
    assert_eq!(orig_normal, another_png, "未超阈值时原图应无损存储");
}

/// V3-F1-A04：分级清理 + 三态归一
///
/// 验证：
/// 1. 第一级：最旧非收藏的原图被 strip（original_present=0、original BLOB 空、缩略图仍在）
/// 2. 收藏豁免：收藏项原图和整条记录都保持不变
/// 3. 三态归一：strip 后 is_degraded=true（与超大图未存同一状态）
/// 4. 第二级：缩略图也满时整条删最旧非收藏
#[test]
fn tiered_cleanup_and_state_unify_strips_oldest_nonfavorite_preserves_favorite() {
    use quickquick_lib::image::{CleanupPolicy, tiered_cleanup, is_degraded, get_original_present};

    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    // 生成 3 张 PNG：oldest（最旧）、middle（中间）、newest（最新，收藏）
    let oldest_png = make_test_png(10, 10, [255u8, 0u8, 0u8]);
    let middle_png = make_test_png(10, 10, [0u8, 255u8, 0u8]);
    let newest_png = make_test_png(10, 10, [0u8, 0u8, 255u8]);

    // 按时间顺序插入（created_utc 递增）
    let oldest_id = insert_image_with_ts(&conn, &oldest_png, 1000, false);
    let _middle_id = insert_image_with_ts(&conn, &middle_png, 2000, false);
    // newest 标记为收藏
    let newest_id = insert_image_with_ts(&conn, &newest_png, 3000, true);

    // 确认初始状态：3 条记录都有原图
    assert_eq!(img::image_count(&conn).expect("image_count"), 3);
    assert_eq!(img::get_original_present(&conn, &oldest_id).expect("ok").expect("some"), 1);
    assert_eq!(img::get_original_present(&conn, &newest_id).expect("ok").expect("some"), 1);

    // 计算当前总量，设 limit = 总量 - oldest原图大小，
    // 使得 strip oldest 原图后总量刚好 ≤ limit（触发第一级但不触发第二级）。
    // 这样可以断言 oldest 被 strip 后行仍存活（is_deleted=0）。
    let total_before = img::total_image_bytes(&conn).expect("total_image_bytes 应成功");
    // 从 DB 查询 length(original)，固化"DB 实际存储大小"而非依赖内存 PNG 字节数推算
    let oldest_orig_size: i64 = conn.query_row(
        "SELECT COALESCE(length(original), 0) FROM clip_images WHERE id = ?1",
        rusqlite::params![oldest_id],
        |row| row.get(0),
    ).expect("查询 oldest 原图大小应成功");
    // limit = 总量 - oldest原图大小（strip后总量降至此值，刚好 ≤ limit）
    let limit = total_before - oldest_orig_size;
    let policy = CleanupPolicy { max_total_bytes: limit };
    let report = tiered_cleanup(&conn, &policy).expect("tiered_cleanup 应成功");

    // Assert 1：第一级 strip 了原图（oldest 最旧非收藏先被 strip）
    assert!(report.stripped_originals >= 1, "应至少 strip 1 条原图，实际: {}", report.stripped_originals);

    // Assert 2：oldest 被 strip——original_present=0、BLOB 空
    let oldest_present = get_original_present(&conn, &oldest_id)
        .expect("get_original_present 应成功")
        .expect("oldest 行应仍存在");
    assert_eq!(oldest_present, 0, "oldest 的 original_present 应被置为 0");

    let oldest_orig = img::get_image_original(&conn, &oldest_id).expect("ok");
    assert!(
        oldest_orig.is_none_or(|v| v.is_empty()),
        "oldest 的原图 BLOB 应被清空"
    );

    // Assert 3：oldest 缩略图仍在（不删缩略图）
    let oldest_thumb = img::get_image_thumbnail(&conn, &oldest_id)
        .expect("ok")
        .expect("oldest 缩略图应仍存在");
    assert!(!oldest_thumb.is_empty(), "oldest 缩略图不应被清空");

    // Assert 4：三态归一——is_degraded=true（与超大图未存同一状态）
    assert!(
        is_degraded(&conn, &oldest_id).expect("is_degraded 应成功"),
        "strip 后 is_degraded 应为 true（三态归一）"
    );

    // Assert 5：收藏豁免——newest 原图和整条记录完整
    let newest_present = get_original_present(&conn, &newest_id)
        .expect("ok")
        .expect("newest 行应仍存在");
    assert_eq!(newest_present, 1, "收藏项 original_present 应仍为 1（豁免）");

    let newest_orig = img::get_image_original(&conn, &newest_id)
        .expect("ok")
        .expect("收藏项原图应仍存在");
    assert!(!newest_orig.is_empty(), "收藏项原图不应被清空");

    // Assert 6：收藏整条存活
    let fav_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM clip_images WHERE id = ?1 AND is_deleted = 0",
        rusqlite::params![newest_id],
        |row| row.get(0),
    ).expect("查询应成功");
    assert_eq!(fav_count, 1, "收藏项整条不应被删");
}

/// V3-F1-A04（第二级）：缩略图也超限时整条删最旧非收藏
#[test]
fn tiered_cleanup_deletes_whole_row_when_thumbnails_also_exceed_limit() {
    use quickquick_lib::image::{CleanupPolicy, tiered_cleanup};

    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    // 插入 2 张非收藏图片，设 0 字节上限强制第二级触发
    let png1 = make_test_png(10, 10, [100u8, 0u8, 0u8]);
    let png2 = make_test_png(10, 10, [0u8, 100u8, 0u8]);
    let id1 = insert_image_with_ts(&conn, &png1, 1000, false);
    let _id2 = insert_image_with_ts(&conn, &png2, 2000, false);

    // max_total_bytes=0：所有原图 strip 后缩略图还是 > 0 → 触发第二级整条删
    let policy = CleanupPolicy { max_total_bytes: 0 };
    let report = tiered_cleanup(&conn, &policy).expect("tiered_cleanup 应成功");

    // Assert：第二级至少整条删了最旧的 id1
    assert!(report.deleted_rows >= 1, "应至少整条删 1 行，实际: {}", report.deleted_rows);

    // Assert：id1 的行已软删或物理删（is_deleted=1 或不存在）
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM clip_images WHERE id = ?1 AND is_deleted = 0",
        rusqlite::params![id1],
        |row| row.get(0),
    ).expect("查询应成功");
    assert_eq!(count, 0, "最旧非收藏行应被整条删除（软删）");
}

/// 辅助：插入一条 clip_images 行（含真实缩略图），返回 id。
/// created_utc 用于控制排序（越小越旧）。
fn insert_image_with_ts(
    conn: &rusqlite::Connection,
    original: &[u8],
    created_utc: i64,
    is_favorite: bool,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let thumbnail = img::make_thumbnail(original).expect("make_thumbnail 应成功");
    let hash = img::image_hash(original);
    let fav: i32 = if is_favorite { 1 } else { 0 };

    conn.execute(
        "INSERT INTO clip_images
             (id, thumbnail, original, original_present, image_hash,
              created_utc, last_modified_utc, is_deleted, is_favorite)
         VALUES (?1, ?2, ?3, 1, ?4, ?5, ?5, 0, ?6)",
        rusqlite::params![id, thumbnail, original, hash, created_utc, fav],
    ).expect("插入 clip_images 应成功");

    id
}

/// 程序生成指定尺寸纯色 PNG 字节（用于测试，无需磁盘文件）。
fn make_test_png(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
    use image::{ImageBuffer, Rgb};
    use std::io::Cursor;

    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_fn(width, height, |_, _| Rgb(rgb));

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("程序生成测试 PNG 不应失败");
    buf.into_inner()
}
