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
