//! 集成测试：预计算 `title` 落库 + 分批幂等回填 + 列表 payload 不因本项变化。
//!
//! 覆盖验收项（P4-5a，后端部分）：
//! - 新入库条目的 title 正确：正文空白规范化、超过 1000 字符截断、图片条目留空
//! - 回填幂等：只补 `title IS NULL` 的行，二次回填 0 行
//! - 回填可续跑：跑一半停下再跑，只补剩下的行，已补行不被重算（触发器审计锚定）
//! - 列表 payload 与改前**逐字节一致**：DTO 不新增 title 字段
//!
//! 迁移 v1→v2 的「真实文件库 + SQLCipher」端到端升级在 `src/db.rs` 的单元测试里
//! （需要私有开库入口与 `MIGRATIONS`），这里只覆盖数据面。
//!
//! 已知缺口（本项不做，已登记进 dev-log）：`title` 截断到 1000 字符后，
//! 超过 1000 字符的正文尾部在 title 上不可搜；等列表改成「只传 title、正文按需取」
//! 的两段式加载时，必须同时解决长正文尾部的可搜性（候选：后端 LIKE / FTS5）。

use std::time::Instant;

use quickquick_lib::clipboard::{rgba_to_png_for_test, CapturedItem};
use quickquick_lib::db;
use quickquick_lib::ipc::clipboard::{list_clip_items_impl, ClipItemDto};
use rusqlite::Connection;
use serde::Serialize;
use tempfile::tempdir;

/// 固定 32 字节测试密钥（全 7），不依赖钥匙串。
const TEST_KEY: [u8; 32] = [7u8; 32];

/// 建一个走生产开库路径（= 当前 schema 版本）的临时加密库。
fn open_db() -> (tempfile::TempDir, Connection) {
    let dir = tempdir().expect("tempdir 创建失败");
    let conn =
        db::open_or_create(&dir.path().join("quickquick.db"), &TEST_KEY).expect("开库应成功");
    (dir, conn)
}

/// 4×4 不透明图的 PNG 字节（图片条目入库用）。
fn test_png() -> Vec<u8> {
    let rgba = vec![0x40u8; 4 * 4 * 4];
    rgba_to_png_for_test(4, 4, &rgba).expect("测试 PNG 编码应成功")
}

/// 读某一行的 title（NULL 与空串在这个文件里是两种必须区分的东西）。
fn read_title(conn: &Connection, id: &str) -> Option<String> {
    conn.query_row("SELECT title FROM clip_items WHERE id = ?1", [id], |row| {
        row.get::<_, Option<String>>(0)
    })
    .expect("读 title 失败")
}

/// 待回填行数（`title IS NULL`）。
fn count_pending_titles(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM clip_items WHERE title IS NULL",
        [],
        |row| row.get(0),
    )
    .expect("统计待回填行失败")
}

/// 按生产批大小循环回填直到没有待补行，返回本次补齐的行数。
fn run_backfill(conn: &Connection) -> usize {
    let mut filled = 0usize;
    loop {
        let written =
            db::backfill_titles_batch(conn, db::TITLE_BACKFILL_BATCH_SIZE).expect("回填批次失败");
        if written == 0 {
            return filled;
        }
        filled += written;
    }
}

/// 直接插一条「升级前形态」的文本行：`title` 留 NULL（NULL = 尚未回填）。
fn insert_pending_text_row(conn: &Connection, id: &str, content: &str) {
    conn.execute(
        "INSERT INTO clip_items
             (id, content, kind, created_utc, last_modified_utc, is_deleted, text_hash, is_favorite)
         VALUES (?1, ?2, 'text', 1700000000000, 1700000000000, 0, ?3, 0)",
        rusqlite::params![id, content, format!("hash-{id}")],
    )
    .expect("插入待回填行失败");
}

/// 入库一条纯文本，返回新行 id。
fn ingest_text(conn: &Connection, text: &str) -> String {
    match db::ingest(
        conn,
        &CapturedItem {
            text: text.to_string(),
            html: None,
        },
    )
    .expect("入库应成功")
    {
        db::IngestOutcome::Inserted(id) => id,
        db::IngestOutcome::Bumped(_) => panic!("内容重复会导致 Bumped，测试数据应两两不同"),
    }
}

// ===== 新入库条目的 title =====

/// 入库路径给新条目写 title，且做空白规范化（多行/缩进折叠成单空格）。
#[test]
fn ingest_writes_normalized_title() {
    // Arrange
    let (_dir, conn) = open_db();

    // Act
    let id = ingest_text(&conn, "  第一行\n\t第二行   第三行  ");

    // Assert
    assert_eq!(
        read_title(&conn, &id).as_deref(),
        Some("第一行 第二行 第三行"),
        "title 应是正文的空白规范化结果（去首尾、连续空白折叠为单空格）"
    );
}

/// 入库路径截断到 1000 字符，且截断按字符（不是字节）进行。
#[test]
fn ingest_truncates_title_to_1000_chars() {
    // Arrange：1200 个汉字，UTF-8 下 3600 字节——按字节切会切进多字节字符
    let (_dir, conn) = open_db();
    let text = "汉".repeat(1200);

    // Act
    let id = ingest_text(&conn, &text);

    // Assert：1000 是产品约定的字面量，不引用常量——拿常量当期望值等于
    // 「实现改成 100 也绿」，钉不住「截断长度就是 1000」这条口径。
    assert_eq!(
        db::TITLE_MAX_CHARS,
        1000,
        "TITLE_MAX_CHARS 是产品约定值（1000），改它要同步改本条断言与活动卡"
    );
    let title = read_title(&conn, &id).expect("新入库条目 title 不应为 NULL");
    assert_eq!(
        title.chars().count(),
        1000,
        "title 应恰好截断到 1000 个字符，实际 {}",
        title.chars().count()
    );
    assert!(
        text.starts_with(&title),
        "截断后的 title 必须是正文的前缀（按字符边界切，不能切进多字节字符）"
    );
}

/// 图片条目按约定 title 留空——但必须是**空串**而不是 NULL。
///
/// 若留 NULL，回填的 `WHERE title IS NULL` 会永远挑到图片行，回填永不收敛。
#[test]
fn image_item_title_is_empty_string_not_null() {
    // Arrange
    let (_dir, conn) = open_db();

    // Act
    let outcome =
        db::ingest_image_as_clip(&conn, 4, 4, &test_png(), 10_000_000).expect("图片入库应成功");
    let db::IngestOutcome::Inserted(id) = outcome else {
        panic!("首次图片入库应 Inserted");
    };

    // Assert
    assert_eq!(
        read_title(&conn, &id),
        Some(String::new()),
        "图片条目 title 应为空串（已定值且按约定为空），而不是 NULL（未回填）"
    );
    assert_eq!(
        count_pending_titles(&conn),
        0,
        "图片条目不得留在待回填集合里，否则回填永远收敛不了"
    );
}

// ===== 回填 =====

/// 回填补齐升级前的老行，二次回填 0 行（幂等）。
#[test]
fn backfill_fills_legacy_rows_and_second_run_is_noop() {
    // Arrange
    let (_dir, conn) = open_db();
    insert_pending_text_row(&conn, "legacy-1", "  老条目\n一  ");
    insert_pending_text_row(&conn, "legacy-2", "老条目二");
    assert_eq!(count_pending_titles(&conn), 2, "前置条件：两条待回填行");

    // Act
    let filled = run_backfill(&conn);

    // Assert
    assert_eq!(filled, 2, "首次回填应补齐两条老行");
    assert_eq!(count_pending_titles(&conn), 0, "回填后不应再有待补行");
    assert_eq!(
        read_title(&conn, "legacy-1").as_deref(),
        Some("老条目 一"),
        "回填必须走与入库同一条规范化逻辑（不能各写一份）"
    );
    assert_eq!(read_title(&conn, "legacy-2").as_deref(), Some("老条目二"));

    // 幂等：再跑一次一行都不写
    assert_eq!(run_backfill(&conn), 0, "没有待补行时二次回填应写入 0 行");
}

/// 回填中断后可续跑，且已补行不被重算（用触发器审计「谁被写了」）。
///
/// 模拟中断的方式：先只跑一批（等价于「进程在第二批之前被杀掉」），
/// 然后完整回填一次。审计表记录每一次 `UPDATE OF title` 的目标行，
/// 因此「已补行被重算」有可观察痕迹，而不是靠肉眼推断 SQL。
#[test]
fn backfill_resumes_after_interruption_without_recomputing_filled_rows() {
    // Arrange
    let (_dir, conn) = open_db();
    conn.execute_batch(
        "CREATE TABLE title_update_audit (id TEXT NOT NULL);
         CREATE TRIGGER audit_title_update AFTER UPDATE OF title ON clip_items
         BEGIN
             INSERT INTO title_update_audit (id) VALUES (NEW.id);
         END;",
    )
    .expect("建审计触发器失败");

    const TOTAL: usize = 500;
    for i in 0..TOTAL {
        insert_pending_text_row(&conn, &format!("row-{i:04}"), &format!("  内容 {i}  "));
    }

    // Act 1：只跑一批 = 模拟跑到一半进程被杀（本批已提交、后续批未开始）
    let first_batch =
        db::backfill_titles_batch(&conn, db::TITLE_BACKFILL_BATCH_SIZE).expect("首批回填失败");
    assert_eq!(
        first_batch,
        db::TITLE_BACKFILL_BATCH_SIZE,
        "首批应写满一批（{} 行）",
        db::TITLE_BACKFILL_BATCH_SIZE
    );
    let audited_after_first: i64 = conn
        .query_row("SELECT COUNT(*) FROM title_update_audit", [], |row| {
            row.get(0)
        })
        .expect("统计审计行失败");
    assert_eq!(
        audited_after_first, first_batch as i64,
        "首批应恰好写 {first_batch} 行"
    );

    // Act 2：重启后续跑
    let resumed = run_backfill(&conn);

    // Assert 1：剩余行被补齐，总数守恒
    assert_eq!(
        resumed,
        TOTAL - first_batch,
        "续跑只应补剩下的 {} 行，实际 {resumed}",
        TOTAL - first_batch
    );
    assert_eq!(count_pending_titles(&conn), 0, "续跑后不应再有待补行");
    assert_eq!(
        read_title(&conn, "row-0499").as_deref(),
        Some("内容 499"),
        "最后一条也应被补齐且规范化"
    );

    // Assert 2：已补行没有被重算——审计行总数等于 TOTAL（每行只被写一次）
    let audited_total: i64 = conn
        .query_row("SELECT COUNT(*) FROM title_update_audit", [], |row| {
            row.get(0)
        })
        .expect("统计审计行失败");
    assert_eq!(
        audited_total, TOTAL as i64,
        "每行应只被写一次；大于 {TOTAL} 说明已补行在续跑时被重算（WHERE title IS NULL 失效）"
    );
    let distinct: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT id) FROM title_update_audit",
            [],
            |row| row.get(0),
        )
        .expect("统计去重审计行失败");
    assert_eq!(distinct, TOTAL as i64, "审计里的行应两两不同（无重复写入）");
}

// ===== 量化：列表 payload 不得因本项变化 =====

/// 「改前」的列表 DTO 结构：本项之前的字段集，一个不多一个不少。
///
/// 用它做逐字节对照，而不是只比长度：只要 DTO 里多出任何字段（例如 `title`），
/// 序列化结果就会多出那个 key，字符串比对立刻红。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClipItemDtoBeforeTitle {
    id: String,
    content: String,
    kind: String,
    is_favorite: bool,
    last_modified_utc: i64,
    has_thumbnail: bool,
    image_id: Option<String>,
    html_content: Option<String>,
}

/// 确定性伪随机（LCG）：不引入额外依赖，且每次跑出同一批数据。
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }

    fn range(&mut self, low: usize, high: usize) -> usize {
        low + (self.next() as usize) % (high - low + 1)
    }

    /// 一个 2–8 字符的「词」（不含空白，保证单行文本的空白规范化是恒等变换）。
    fn word(&mut self) -> String {
        const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let len = self.range(2, 8);
        (0..len)
            .map(|_| ALPHABET[(self.next() as usize) % ALPHABET.len()] as char)
            .collect()
    }

    /// 单行：词用单个半角空格连接（无首尾空白、无连续空白）。
    fn single_line(&mut self, words: usize) -> String {
        (0..words)
            .map(|_| self.word())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// 多行：每行一个单行词串，行间用换行连接。
    fn multi_line(&mut self, lines: usize, words_per_line: usize) -> String {
        (0..lines)
            .map(|_| self.single_line(words_per_line))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// 造 2000 条数据，取列表 DTO 的 JSON 字节数；`multi_only` 决定数据形态。
///
/// 两种形态各测一遍，因为「title 与 content 不同」的行占比决定 payload 增量：
/// - 混合形态：55% 单行短文本（10–40 词）+ 30% 多行短文本（3–12 行）
///   + 10% 中等文本（30–60 行）+ 5% 长文本（120–160 行）
/// - 全多行：100% 多行短文本（3–12 行）——最坏形态，title 与 content 几乎必然不同
fn measure_payload(multi_only: bool) -> (String, String, u128) {
    const ITEMS: usize = 2000;
    let (_dir, conn) = open_db();
    let mut rng = Lcg(2026_1025);
    let started = Instant::now();
    for i in 0..ITEMS {
        let shape = i % 100;
        let body = if multi_only {
            let lines = rng.range(3, 12);
            let words_per_line = rng.range(4, 8);
            rng.multi_line(lines, words_per_line)
        } else if shape <= 54 {
            let words = rng.range(10, 40);
            rng.single_line(words)
        } else if shape <= 84 {
            let lines = rng.range(3, 12);
            let words_per_line = rng.range(4, 8);
            rng.multi_line(lines, words_per_line)
        } else if shape <= 94 {
            let lines = rng.range(30, 60);
            rng.multi_line(lines, 10)
        } else {
            let lines = rng.range(120, 160);
            rng.multi_line(lines, 10)
        };
        // 末尾追加序号保证 text_hash 两两不同（否则会被去重成一条）。
        ingest_text(&conn, &format!("{body} #{i}"));
    }
    let seeded_ms = started.elapsed().as_millis();

    assert_eq!(
        db::count_live(&conn).expect("计数失败"),
        ITEMS as i64,
        "前置条件：应造出 {ITEMS} 条互不重复的条目"
    );
    assert_eq!(count_pending_titles(&conn), 0, "入库路径应写满 title");

    let items = list_clip_items_impl(&conn).expect("列表查询失败");
    let before = json_body(&before_dtos(&items));
    let after = json_body(&items);
    (before, after, seeded_ms)
}

fn json_body(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("序列化 DTO 失败")
}

fn before_dtos(items: &[ClipItemDto]) -> Vec<ClipItemDtoBeforeTitle> {
    items
        .iter()
        .map(|item| ClipItemDtoBeforeTitle {
            id: item.id.clone(),
            content: item.content.clone(),
            kind: item.kind.clone(),
            is_favorite: item.is_favorite,
            last_modified_utc: item.last_modified_utc,
            has_thumbnail: item.has_thumbnail,
            image_id: item.image_id.clone(),
            html_content: item.html_content.clone(),
        })
        .collect()
}

/// 2000 条数据的列表 payload：改前/改后逐字节一致（本项不得让 payload 变大）。
///
/// 数字记录（2026-10-25，本机实测）：混合形态改前 **2,105,877 B**、
/// 全多行形态改前 **923,648 B**；两者改后与改前**完全相同**。
/// 作为对照，早先「DTO 无条件下发 title」的版本在同样的数据上是 +82.8% / +62.2%，
/// 这正是本项最终**不改 DTO** 的原因：`content` 仍全量下发，再带一份 title 只有成本。
#[test]
fn list_payload_is_byte_identical_to_before() {
    let (before_mixed, after_mixed, seeded_ms) = measure_payload(false);
    let (before_multi, after_multi, _) = measure_payload(true);

    eprintln!(
        "[P4-5a payload] 混合形态 改前 {} B / 改后 {} B（造数据 {seeded_ms} ms）；\
         全多行形态 改前 {} B / 改后 {} B",
        before_mixed.len(),
        after_mixed.len(),
        before_multi.len(),
        after_multi.len()
    );

    assert_eq!(
        before_mixed.len(),
        2_105_877,
        "混合形态的基线字节数应与活动卡记录一致（造数脚本变了就同步更新两处）"
    );
    assert_eq!(
        before_multi.len(),
        923_648,
        "全多行形态的基线字节数应与活动卡记录一致"
    );
    assert_eq!(
        after_mixed, before_mixed,
        "列表 payload 必须与改前逐字节一致：DTO 不得新增 title 字段"
    );
    assert_eq!(
        after_multi, before_multi,
        "列表 payload 必须与改前逐字节一致（全多行形态）"
    );
}
