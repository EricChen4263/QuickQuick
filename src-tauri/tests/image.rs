//! 集成测试：图片捕获入库
//!
//! 覆盖验收项：
//! - V3-F1-A01 image_capture_lossless_split

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
