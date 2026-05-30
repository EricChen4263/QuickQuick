//! 集成测试：schema 预埋字段 + 软删 + GC
//!
//! 覆盖验收项：
//! - V0-F3-A04 schema_preembed_columns
//! - V0-F3-A05 soft_delete_and_gc

use quickquick_lib::db;
use tempfile::tempdir;
use uuid::Uuid;

/// 固定 32 字节测试密钥，不依赖钥匙串
const TEST_KEY: [u8; 32] = [7u8; 32];

// ── A04：schema 预埋列断言 ────────────────────────────────────────────────────

/// 辅助：从 PRAGMA table_info 结果中提取列名集合
///
/// 白名单校验表名，防止测试辅助函数被误用于不受信任的字符串（安全§10 输入校验）。
fn table_columns(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
    assert!(
        matches!(table, "clip_items" | "clip_images"),
        "未知表名 '{table}'，仅允许 clip_items / clip_images"
    );
    let sql = format!("PRAGMA table_info({})", table);
    let mut stmt = conn.prepare(&sql).expect("PRAGMA table_info 应成功");
    stmt.query_map([], |row| row.get::<_, String>(1))
        .expect("query_map 应成功")
        .filter_map(|r| r.ok())
        .collect()
}

/// V0-F3-A04：clip_items 表含 UUID/created/last_modified/墓碑 必要列
#[test]
fn schema_preembed_columns_clip_items() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    // Act
    let cols = table_columns(&conn, "clip_items");

    // Assert：UUID 主键
    assert!(
        cols.contains(&"id".to_string()),
        "clip_items 应含 id 列（UUID 主键）；实际列: {:?}",
        cols
    );
    // Assert：created_utc
    assert!(
        cols.contains(&"created_utc".to_string()),
        "clip_items 应含 created_utc 列；实际列: {:?}",
        cols
    );
    // Assert：last_modified_utc
    assert!(
        cols.contains(&"last_modified_utc".to_string()),
        "clip_items 应含 last_modified_utc 列；实际列: {:?}",
        cols
    );
    // Assert：墓碑 is_deleted
    assert!(
        cols.contains(&"is_deleted".to_string()),
        "clip_items 应含 is_deleted 墓碑列；实际列: {:?}",
        cols
    );
    // Assert：deleted_at_utc
    assert!(
        cols.contains(&"deleted_at_utc".to_string()),
        "clip_items 应含 deleted_at_utc 列；实际列: {:?}",
        cols
    );
}

/// V0-F3-A04：clip_images 表含缩略图/原图两字段及墓碑列
#[test]
fn schema_preembed_columns_clip_images() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    // Act
    let cols = table_columns(&conn, "clip_images");

    // Assert：UUID 主键
    assert!(
        cols.contains(&"id".to_string()),
        "clip_images 应含 id 列；实际列: {:?}",
        cols
    );
    // Assert：缩略图 BLOB
    assert!(
        cols.contains(&"thumbnail".to_string()),
        "clip_images 应含 thumbnail BLOB 列；实际列: {:?}",
        cols
    );
    // Assert：原图 BLOB
    assert!(
        cols.contains(&"original".to_string()),
        "clip_images 应含 original BLOB 列；实际列: {:?}",
        cols
    );
    // Assert：降级态标记
    assert!(
        cols.contains(&"original_present".to_string()),
        "clip_images 应含 original_present 列；实际列: {:?}",
        cols
    );
    // Assert：created_utc
    assert!(
        cols.contains(&"created_utc".to_string()),
        "clip_images 应含 created_utc 列；实际列: {:?}",
        cols
    );
    // Assert：last_modified_utc
    assert!(
        cols.contains(&"last_modified_utc".to_string()),
        "clip_images 应含 last_modified_utc 列；实际列: {:?}",
        cols
    );
    // Assert：墓碑
    assert!(
        cols.contains(&"is_deleted".to_string()),
        "clip_images 应含 is_deleted 列；实际列: {:?}",
        cols
    );
}

// ── A05：软删 + GC ────────────────────────────────────────────────────────────

/// V0-F3-A05：插入 → soft_delete → 行仍在且 is_deleted=1 → gc 后物理消失
#[test]
fn soft_delete_and_gc_full_lifecycle() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    let id = Uuid::new_v4().to_string();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("系统时间应在 epoch 之后")
        .as_millis() as i64;

    // 插入一条记录
    conn.execute(
        "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc, is_deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        rusqlite::params![id, "hello world", "text", now_ms, now_ms],
    )
    .expect("插入应成功");

    // Act 1：软删
    db::soft_delete(&conn, &id).expect("soft_delete 应成功");

    // Assert 1：行仍存在（非物理删）
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clip_items WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .expect("查询应成功");
    assert_eq!(count, 1, "soft_delete 后行应仍在（非物理删）");

    // Assert 2：is_deleted=1
    let is_deleted: i64 = conn
        .query_row(
            "SELECT is_deleted FROM clip_items WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .expect("查询 is_deleted 应成功");
    assert_eq!(is_deleted, 1, "soft_delete 后 is_deleted 应为 1");

    // Act 2：GC 物理清理
    let purged = db::gc_purge_deleted(&conn).expect("gc_purge_deleted 应成功");

    // Assert 3：返回清理条数 >= 1
    assert!(purged >= 1, "gc_purge_deleted 应返回清理条数 >= 1，实际: {}", purged);

    // Assert 4：行物理消失
    let count_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clip_items WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .expect("查询应成功");
    assert_eq!(count_after, 0, "gc 后行应物理消失");
}

// ── I-03：外键约束 + foreign_keys PRAGMA 验证 ────────────────────────────────

/// I-03：foreign_keys PRAGMA 在开库后应为 ON（值 = 1）
#[test]
fn foreign_keys_pragma_is_enabled_after_open() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    // Act
    let fk_on: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("PRAGMA foreign_keys 查询应成功");

    // Assert：外键约束必须在运行期开启
    assert_eq!(fk_on, 1, "foreign_keys PRAGMA 应为 1（ON），实际: {}", fk_on);
}

/// I-03：clip_images 向不存在的 clip_item_id 插入应被外键约束拒绝（ON DELETE CASCADE 语义验证）
#[test]
fn foreign_key_rejects_dangling_clip_item_id() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    let image_id = Uuid::new_v4().to_string();
    let nonexistent_clip_id = Uuid::new_v4().to_string(); // 故意不插入 clip_items
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("系统时间应在 epoch 之后")
        .as_millis() as i64;

    // Act：插入悬空外键，应被拒绝
    let result = conn.execute(
        "INSERT INTO clip_images
             (id, clip_item_id, created_utc, last_modified_utc, is_deleted, original_present)
         VALUES (?1, ?2, ?3, ?4, 0, 0)",
        rusqlite::params![image_id, nonexistent_clip_id, now_ms, now_ms],
    );

    // Assert：外键约束应拒绝悬空引用
    assert!(
        result.is_err(),
        "插入悬空 clip_item_id 应被外键约束拒绝，但实际成功"
    );
}

/// I-03：删除 clip_items 行时，关联的 clip_images 应级联删除（ON DELETE CASCADE）
#[test]
fn gc_cascade_deletes_clip_images_on_clip_item_removal() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    let clip_id = Uuid::new_v4().to_string();
    let image_id = Uuid::new_v4().to_string();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("系统时间应在 epoch 之后")
        .as_millis() as i64;

    // 先插入父行
    conn.execute(
        "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc, is_deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        rusqlite::params![clip_id, "cascade test", "text", now_ms, now_ms],
    )
    .expect("插入 clip_items 应成功");

    // 插入关联子行
    conn.execute(
        "INSERT INTO clip_images
             (id, clip_item_id, created_utc, last_modified_utc, is_deleted, original_present)
         VALUES (?1, ?2, ?3, ?4, 0, 0)",
        rusqlite::params![image_id, clip_id, now_ms, now_ms],
    )
    .expect("插入 clip_images 应成功");

    // Act：GC 物理删除软删的 clip_items（应级联删 clip_images）
    let purged = db::gc_purge_deleted(&conn).expect("gc_purge_deleted 应成功");

    // Assert：clip_items 已删
    assert_eq!(purged, 1, "gc 应删除 1 条 clip_items，实际: {}", purged);

    // Assert：clip_images 因级联也消失
    let img_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clip_images WHERE id = ?1",
            rusqlite::params![image_id],
            |row| row.get(0),
        )
        .expect("查询 clip_images 应成功");
    assert_eq!(
        img_count, 0,
        "clip_items 删除后，关联的 clip_images 应级联删除，实际仍有: {}",
        img_count
    );
}

/// V0-F3-A05：非软删行不被 GC 清理
#[test]
fn soft_delete_gc_does_not_affect_live_rows() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = db::open_or_create(&db_path, &TEST_KEY).expect("建库应成功");

    let id = Uuid::new_v4().to_string();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("系统时间应在 epoch 之后")
        .as_millis() as i64;

    conn.execute(
        "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc, is_deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        rusqlite::params![id, "live content", "text", now_ms, now_ms],
    )
    .expect("插入应成功");

    // Act：GC（未软删的行不应被清理）
    let purged = db::gc_purge_deleted(&conn).expect("gc_purge_deleted 应成功");

    // Assert：清理条数为 0
    assert_eq!(purged, 0, "无软删行时 gc 清理条数应为 0");

    // Assert：行仍存在
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clip_items WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .expect("查询应成功");
    assert_eq!(count, 1, "未软删的行不应被 gc 清理");
}
