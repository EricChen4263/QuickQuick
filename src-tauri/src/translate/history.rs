//! 翻译历史模块（V2-F3-A14）
//!
//! 职责：
//! 1. 向独立的 `translate_history` 表写入翻译历史条目
//! 2. 支持剪贴板条目「一键翻译」：读 clip_items 原文，写入 translate_history
//! 3. 翻译历史与剪贴板历史严格分开存储，互不混入
//! 4. 查询按页取（上限 200 条），写入后按保留上限（500 条）best-effort 清理最旧

use rusqlite::Connection;
use uuid::Uuid;

use crate::db::DbError;

/// 单次查询返回的翻译历史条数上限。
///
/// 取 200：历史栏是「最近用过什么」的参考，不需要全量；历史栏宽 300px、每条约 64px 高，
/// 200 条已经远超一屏可见范围（连滚带翻也看不完），把 IPC payload 与 SQLite 结果集
/// 钉在这个量级，就能避免历史无界增长后每次开页都全量搬运。
pub const TRANSLATE_HISTORY_PAGE_LIMIT: i64 = 200;

/// 表内保留的翻译历史条数上限（超过则删最旧）。
///
/// 取 500：等于分页上限的 2.5 倍——既覆盖「翻回去找前些天用过的一条」这类真实回溯
/// （查询一次只取最近 200 条，其余仍留在库里），又给表体量一个确定上界
/// （单条历史是一段原文 + 一段译文，500 条的体量在加密库里可忽略）。
/// 保留上限高于分页上限是刻意的：分页限制的是「一次搬多少」，不是「能存多少」。
pub const TRANSLATE_HISTORY_RETENTION: i64 = 500;

/// 向 `translate_history` 表插入一条翻译历史，返回新记录的 id。
///
/// 所有文本字段均使用参数化查询，不拼接 SQL（安全§10）。
///
/// 写入后会触发一次保留上限清理（[`prune_translate_history`]），清理失败不影响返回：
/// 历史是「翻译成功后顺手记一笔」的旁路，不能因为它让用户丢掉已经拿到的译文。
///
/// 旧版本留下的超量历史在这里收敛：这是唯一的写入口（`translate_clip_item` 与
/// 翻译主链路都经由本函数），因此「旧库超量」会在用户下一次翻译时按新上限收敛。
/// 查询路径不做写操作：`list_translate_history` 可能被前端在挂载与事件刷新时反复调用，
/// 在读路径里顺手 DELETE 会把「纯读」变成隐式写（还可能撞上别人的写锁），不值得。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn add_translate_history(
    conn: &Connection,
    source_text: &str,
    translated_text: &str,
    source_lang: &str,
    target_lang: &str,
    provider_id: &str,
) -> Result<String, DbError> {
    let id = Uuid::new_v4().to_string();
    let now_ms = current_utc_ms();
    conn.execute(
        "INSERT INTO translate_history
             (id, source_text, translated_text, source_lang, target_lang, provider_id, created_utc)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            id,
            source_text,
            translated_text,
            source_lang,
            target_lang,
            provider_id,
            now_ms
        ],
    )?;
    prune_translate_history(conn);
    Ok(id)
}

/// 超过保留上限时删除最旧的历史条目（best-effort）。
///
/// 触发与开销控制：先用 `COUNT(*)` 判断是否真的超限，超限才执行一次带
/// `LIMIT -1 OFFSET <保留上限>` 的 `DELETE`。之所以不在每次写入都跑
/// `DELETE ... WHERE id IN (SELECT ... OFFSET N)`，是因为那条语句在未超限时
/// 仍要扫一遍表才能确认「没有可删的行」；而本函数把表体量钉在
/// [`TRANSLATE_HISTORY_RETENTION`] 量级，`COUNT(*)` 在无索引的 `translate_history`
/// 上只是一次 O(500) 的 B-tree 计数，属于常量级开销。真正超限时（含旧库遗留的超量数据）
/// 删除语句一次只删「超出上限的那几条」，不做全表重写。
///
/// 缺失索引是已知账：`translate_history` 上没有 `created_utc` 索引（建索引属于 schema
/// 变更，不在本次范围内），因此这次 `COUNT(*)` 与删除排序都要扫表；但表体量上界是 500，
/// 扫描代价与索引方案同阶，等后续给该表补 `(created_utc)` 索引时这两条语句会一并受益。
///
/// 失败只留痕不传播：清理是维护性副作用，译文已经拿到、历史也已写入，
/// 不能因为一次 DELETE 失败就让本次翻译返回错误（数据库只读、锁冲突、触发器阻断都属此类）。
fn prune_translate_history(conn: &Connection) {
    if let Err(e) = prune_translate_history_impl(conn) {
        eprintln!("[QuickQuick] 翻译历史清理失败，本次翻译照常返回: {e}");
    }
}

/// [`prune_translate_history`] 的可测实现，返回实际删除的条数（未超限为 0）。
///
/// 排序与查询侧完全一致（`created_utc DESC, rowid DESC`），保证「留下的是最新 N 条」
/// 这个口径只有一处定义；同毫秒并列时按 rowid 倒序保留后写入的那些。
///
/// # Errors
/// `DbError::Sqlite`：计数或删除语句执行失败
fn prune_translate_history_impl(conn: &Connection) -> Result<usize, DbError> {
    let total = translate_history_count(conn)?;
    if total <= TRANSLATE_HISTORY_RETENTION {
        return Ok(0);
    }
    let deleted = conn.execute(
        "DELETE FROM translate_history
         WHERE id IN (
             SELECT id FROM translate_history
             ORDER BY created_utc DESC, rowid DESC
             LIMIT -1 OFFSET ?1
         )",
        rusqlite::params![TRANSLATE_HISTORY_RETENTION],
    )?;
    Ok(deleted)
}

/// 查询 `translate_history` 表的记录总数。
///
/// 用于测试断言和业务层「翻译历史条目数」查询。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn translate_history_count(conn: &Connection) -> Result<i64, DbError> {
    let n = conn.query_row("SELECT COUNT(*) FROM translate_history", [], |row| {
        row.get(0)
    })?;
    Ok(n)
}

/// 剪贴板条目「一键翻译」：读取 clip_items 中指定条目的原文，写入 translate_history。
///
/// 返回新翻译历史条目的 id。
/// clip_items 本身不受影响（独立存储语义）。
///
/// # Errors
/// - `DbError::Sqlite`：SQL 执行失败，或 `clip_id` 不存在（query_row 返回 QueryReturnedNoRows）
pub fn translate_clip_item(
    conn: &Connection,
    clip_id: &str,
    translated_text: &str,
    source_lang: &str,
    target_lang: &str,
    provider_id: &str,
) -> Result<String, DbError> {
    let source_text: String = conn.query_row(
        "SELECT content FROM clip_items WHERE id = ?1 AND is_deleted = 0",
        rusqlite::params![clip_id],
        |row| row.get(0),
    )?;
    add_translate_history(
        conn,
        &source_text,
        translated_text,
        source_lang,
        target_lang,
        provider_id,
    )
}

/// 按创建时间倒序返回 `translate_history` 表中最近的条目，最多
/// [`TRANSLATE_HISTORY_PAGE_LIMIT`] 条。
///
/// 返回的顺序为最新翻译排在最前，供翻译页历史栏（A08）回填使用。
/// 排序次键 `rowid DESC` 是同毫秒并列时的确定性兜底：一次连击可能在同一毫秒写入多条，
/// 只有按 rowid 倒序才能保证「后写入的排在前」在多行取页边界上也稳定可复现。
///
/// 查询只读，不改库：超量旧数据在写入路径收敛（见 [`add_translate_history`]）。
///
/// # Errors
/// `DbError::Sqlite`：SQL 执行失败
pub fn list_translate_history(conn: &Connection) -> Result<Vec<TranslateHistoryRow>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, source_text, translated_text, source_lang, target_lang, provider_id, created_utc
         FROM translate_history
         ORDER BY created_utc DESC, rowid DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![TRANSLATE_HISTORY_PAGE_LIMIT], |row| {
        Ok(TranslateHistoryRow {
            id: row.get(0)?,
            source_text: row.get(1)?,
            translated_text: row.get(2)?,
            source_lang: row.get(3)?,
            target_lang: row.get(4)?,
            provider_id: row.get(5)?,
            created_utc: row.get(6)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// 翻译历史一行的原始数据（供 IPC 层转 DTO 使用）。
pub struct TranslateHistoryRow {
    pub id: String,
    pub source_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub provider_id: String,
    pub created_utc: i64,
}

/// 获取当前 UTC 时间戳（毫秒）。
fn current_utc_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 建一张与 `db.rs` 中 `translate_history` 列名一致的内存库，隔离被测函数。
    fn setup_history_db() -> Connection {
        let conn = Connection::open_in_memory().expect("内存 DB 打开失败");
        conn.execute_batch(
            "CREATE TABLE translate_history (
                id              TEXT PRIMARY KEY,
                source_text     TEXT NOT NULL,
                translated_text TEXT NOT NULL,
                source_lang     TEXT NOT NULL,
                target_lang     TEXT NOT NULL,
                provider_id     TEXT NOT NULL,
                created_utc     INTEGER NOT NULL
            );",
        )
        .expect("建表失败");
        conn
    }

    /// 直接插入一条历史（created_utc 由调用方钉死），避免依赖真实时钟。
    fn insert_history(conn: &Connection, id: &str, created_utc: i64) {
        conn.execute(
            "INSERT INTO translate_history
                 (id, source_text, translated_text, source_lang, target_lang, provider_id, created_utc)
             VALUES (?1, ?2, ?3, 'en', 'zh', 'lingva', ?4)",
            rusqlite::params![id, format!("src-{id}"), format!("dst-{id}"), created_utc],
        )
        .expect("插入历史应成功");
    }

    /// 按 `0..count` 顺序插入 count 行，created_utc 用给定函数生成，返回 id 列表（与插入序一致）。
    fn insert_many(
        conn: &Connection,
        count: usize,
        created_utc: impl Fn(usize) -> i64,
    ) -> Vec<String> {
        (0..count)
            .map(|i| {
                let id = format!("row-{i:04}");
                insert_history(conn, &id, created_utc(i));
                id
            })
            .collect()
    }

    /// 统计指定 id 的行数（用于断言「某条已被删掉 / 仍在」）。
    fn count_by_id(conn: &Connection, id: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM translate_history WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .expect("按 id 计数应成功")
    }

    #[test]
    fn list_translate_history_returns_only_the_newest_page() {
        // Arrange：LIMIT + 5 条，created_utc 随插入序递增（后者更新）
        let conn = setup_history_db();
        let total = TRANSLATE_HISTORY_PAGE_LIMIT as usize + 5;
        let ids = insert_many(&conn, total, |i| 1_000 + i as i64);

        // Act
        let rows = list_translate_history(&conn).expect("查历史应成功");

        // Assert：只返回最新的一页，首条是最新、末条是「第 LIMIT 新」那条
        assert_eq!(
            rows.len(),
            TRANSLATE_HISTORY_PAGE_LIMIT as usize,
            "超过 LIMIT 时只应返回 LIMIT 条"
        );
        assert_eq!(rows[0].id, ids[total - 1], "首条应是最新一条");
        assert_eq!(
            rows[TRANSLATE_HISTORY_PAGE_LIMIT as usize - 1].id,
            ids[5],
            "末条应是第 LIMIT 新的那条（最旧 5 条被截断）"
        );
        // Assert：整页严格按 created_utc 倒序，没有乱序
        let timestamps: Vec<i64> = rows.iter().map(|r| r.created_utc).collect();
        let mut expected = timestamps.clone();
        expected.sort_unstable_by(|a, b| b.cmp(a));
        assert_eq!(timestamps, expected, "整页应按 created_utc 倒序");
    }

    #[test]
    fn list_translate_history_uses_rowid_desc_as_tiebreaker_at_page_boundary() {
        // Arrange：LIMIT + 5 条全部落在同一毫秒，只有 rowid 能区分先后
        let conn = setup_history_db();
        let total = TRANSLATE_HISTORY_PAGE_LIMIT as usize + 5;
        let ids = insert_many(&conn, total, |_| 1_700_000_000_000);

        // Act
        let rows = list_translate_history(&conn).expect("查历史应成功");

        // Assert：同毫秒并列时按 rowid 倒序，截断边界也落在这个口径上
        assert_eq!(rows.len(), TRANSLATE_HISTORY_PAGE_LIMIT as usize);
        assert_eq!(
            rows[0].id,
            ids[total - 1],
            "同毫秒并列时后写入的应排在最前（rowid DESC）"
        );
        assert_eq!(
            rows[TRANSLATE_HISTORY_PAGE_LIMIT as usize - 1].id,
            ids[5],
            "截断边界应按 rowid 倒序保留后写入的 200 条"
        );
    }

    #[test]
    fn add_translate_history_prunes_down_to_retention_limit() {
        // Arrange：先塞进超过保留上限的旧历史（模拟旧版本遗留的超量数据）
        let conn = setup_history_db();
        let total = TRANSLATE_HISTORY_RETENTION as usize + 40;
        let ids = insert_many(&conn, total, |i| 1_000 + i as i64);

        // Act：一次正常写入，应触发清理
        let new_id = add_translate_history(&conn, "new source", "新译文", "en", "zh", "lingva")
            .expect("写入历史应成功");

        // Assert：总数收敛到保留上限
        assert_eq!(
            translate_history_count(&conn).expect("计数应成功"),
            TRANSLATE_HISTORY_RETENTION,
            "写入触发清理后总数应等于保留上限（{} + 1 - 41 条）",
            total
        );
        // Assert：删掉的是最旧的，保留边界两侧各验证一次
        assert_eq!(count_by_id(&conn, &ids[0]), 0, "最旧一条应被清理");
        assert_eq!(count_by_id(&conn, &ids[40]), 0, "第 41 旧的应被清理");
        assert_eq!(count_by_id(&conn, &ids[41]), 1, "落在保留区内的应留下");
        // Assert：新写入的这条没有被自己触发的清理删掉，且排在最前
        assert_eq!(count_by_id(&conn, &new_id), 1, "新写入的行必须保留");
        let rows = list_translate_history(&conn).expect("查历史应成功");
        assert_eq!(rows[0].id, new_id, "最新一条应是刚写入的那条");
    }

    #[test]
    fn add_translate_history_succeeds_when_prune_fails() {
        // Arrange：超量历史 + 一个必定失败的 BEFORE DELETE 触发器。
        // 触发器先往审计表写一笔再 RAISE(FAIL)，既让 DELETE 报错、又留下「清理真的执行过」的证据，
        // 避免「清理压根没触发」时测试也绿（恒真假绿）。
        let conn = setup_history_db();
        let total = TRANSLATE_HISTORY_RETENTION as usize + 10;
        let ids = insert_many(&conn, total, |i| 1_000 + i as i64);
        conn.execute_batch(
            "CREATE TABLE prune_audit (attempt INTEGER NOT NULL);
             CREATE TRIGGER block_translate_history_delete
             BEFORE DELETE ON translate_history
             BEGIN
                 INSERT INTO prune_audit (attempt) VALUES (1);
                 SELECT RAISE(FAIL, '清理被测试触发器阻断');
             END;",
        )
        .expect("建阻断触发器应成功");

        // Act：清理必然失败，但写入本身必须成功
        let result = add_translate_history(&conn, "new source", "新译文", "en", "zh", "lingva");

        // Assert：主链路不受影响——返回成功，且新行确实落库
        let new_id = result.expect("清理失败不应让写入历史失败");
        assert_eq!(count_by_id(&conn, &new_id), 1, "清理失败时新行仍应写入");
        // Assert：清理确实被尝试过（触发器留下的审计行），而不是压根没走到
        let attempts: i64 = conn
            .query_row("SELECT COUNT(*) FROM prune_audit", [], |row| row.get(0))
            .expect("查审计表应成功");
        assert!(attempts >= 1, "应至少尝试过一次清理，实际 {attempts} 次");
        // Assert：删除失败后一行都没少，表仍是「写入后」的条数
        assert_eq!(
            translate_history_count(&conn).expect("计数应成功"),
            (total + 1) as i64,
            "清理失败时不应删掉任何行"
        );
        for id in &ids {
            assert_eq!(
                count_by_id(&conn, id),
                1,
                "被清理失败的旧行应原样保留: {id}"
            );
        }
    }
}
