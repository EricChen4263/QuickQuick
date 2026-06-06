//! 本地 ECDICT 词典数据库访问层（DAO）。
//!
//! 取代原 `pot-app.com/api/dict` 远程接口：把 ECDICT 词库打包为本地 SQLite，
//! 查词走只读本地查询，零网络、零第三方依赖。表结构见 `tools/gen_ecdict_db.py`：
//! `ecdict(word, phonetic, translation, exchange)`，`LOWER(word)` 上有表达式索引。

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use super::providers::{group_definitions_by_pos, parse_ecdict_exchange};
use super::{DictEntry, TranslateError};

/// 本地 ECDICT 词典只读 DAO。
///
/// 持有数据库文件路径而非长连接：每次 `lookup` 临时只读开库，避免跨线程共享
/// 非 `Send` 的 `Connection`（SQLCipher Connection 不是 Send），与既有 `AppDb`
/// 用 `Mutex` 包裹连接的取舍不同——本库为只读、查询极快，临时开库开销可接受且更简单。
pub struct EcdictDb {
    db_path: PathBuf,
}

impl EcdictDb {
    /// 用数据库文件路径构造 DAO（此时不开库，首次 `lookup` 才开）。
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    /// 查一个英文单词的词条（大小写不敏感）。
    ///
    /// - 命中且 `translation` 非空 → `Ok(Some(DictEntry))`
    /// - 未收录、或 `translation` 为空（视为未命中）→ `Ok(None)`
    /// - 库打不开（文件缺失/损坏）→ `Err(TranslateError::Network)`
    ///
    /// 查询用参数化 `WHERE LOWER(word) = LOWER(?1)` 防注入，并命中 `LOWER(word)` 索引。
    ///
    /// # Errors
    /// 库无法打开或查询失败时返回 `TranslateError::Network`（对齐远程源网络失败语义，
    /// 让上层按「源不可用」处理而非崩溃）。
    pub fn lookup(&self, word: &str) -> Result<Option<DictEntry>, TranslateError> {
        let conn = open_readonly(&self.db_path)?;

        let row = conn
            .query_row(
                "SELECT phonetic, translation, exchange \
                 FROM ecdict WHERE LOWER(word) = LOWER(?1) LIMIT 1",
                [word],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional_row()?;

        Ok(row.and_then(|(phonetic, translation, exchange)| {
            build_dict_entry(phonetic, translation, exchange)
        }))
    }
}

/// 以只读 + 无互斥模式打开本地词典库（明文 SQLite，无需密钥）。
///
/// 库为构建期生成的只读资源、查询不写入，用 `NO_MUTEX` 省去连接级锁开销。
fn open_readonly(path: &Path) -> Result<Connection, TranslateError> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| TranslateError::Network(format!("ECDICT 本地词典库不可用: {e}")))
}

/// 把查询行映射为 `DictEntry`；`translation` 空白视为未命中（返回 None）。
///
/// 复用既有 `group_definitions_by_pos`（按词性前缀分组）与 `parse_ecdict_exchange`
/// （`s:glaciers/p:glacial` → 词形列表），与原远程 ECDICT 源映射逻辑逐字一致。
fn build_dict_entry(
    phonetic: Option<String>,
    translation: Option<String>,
    exchange: Option<String>,
) -> Option<DictEntry> {
    let translation = translation.unwrap_or_default();
    let translation = translation.trim();
    if translation.is_empty() {
        return None;
    }

    let phonetic = phonetic
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let explains: Vec<&str> = translation
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let definitions = group_definitions_by_pos(&explains);

    let inflections = parse_ecdict_exchange(exchange.as_deref().unwrap_or(""));

    Some(DictEntry {
        phonetic,
        definitions,
        examples: vec![],
        audio: None,
        inflections,
    })
}

/// `query_row` 的 `Option` 适配：把 `QueryReturnedNoRows` 归一为 `Ok(None)`，
/// 其余错误归一为 `TranslateError::Network`（库损坏/IO 失败按源不可用处理）。
trait OptionalRow<T> {
    fn optional_row(self) -> Result<Option<T>, TranslateError>;
}

impl<T> OptionalRow<T> for Result<T, rusqlite::Error> {
    fn optional_row(self) -> Result<Option<T>, TranslateError> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(TranslateError::Network(format!(
                "ECDICT 本地词典查询失败: {e}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// 建一个临时 ECDICT 库并写入给定行，返回 (持有目录的守卫, 库路径)。
    ///
    /// 守卫必须由调用方持有到测试结束，否则 TempDir drop 会删库导致查询失败。
    fn fixture_db(rows: &[(&str, &str, &str, &str)]) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().expect("创建临时目录");
        let path = dir.path().join("ecdict.db");
        let conn = Connection::open(&path).expect("建库");
        conn.execute_batch(
            "CREATE TABLE ecdict (\
                word        TEXT NOT NULL,\
                phonetic    TEXT,\
                translation TEXT,\
                exchange    TEXT\
            );\
            CREATE INDEX idx_ecdict_lower_word ON ecdict (LOWER(word));",
        )
        .expect("建表");
        for (word, phonetic, translation, exchange) in rows {
            conn.execute(
                "INSERT INTO ecdict (word, phonetic, translation, exchange) \
                 VALUES (?1, ?2, ?3, ?4)",
                [word, phonetic, translation, exchange],
            )
            .expect("插入行");
        }
        (dir, path)
    }

    #[test]
    fn lookup_hit_returns_dict_entry_with_pos_and_inflections() {
        let (_dir, path) = fixture_db(&[(
            "glacier",
            "ˈɡleɪʃər",
            "n. 冰川，冰河\nvt. 测试动词义",
            "s:glaciers/p:glacial",
        )]);
        let db = EcdictDb::new(path);

        let entry = db
            .lookup("glacier")
            .expect("查询不应出错")
            .expect("应命中词条");

        assert_eq!(entry.phonetic.as_deref(), Some("ˈɡleɪʃər"), "应取音标");
        let noun = entry
            .definitions
            .iter()
            .find(|d| d.pos.as_deref() == Some("n."))
            .expect("应含名词词性分组");
        assert!(
            noun.meanings.iter().any(|m| m.contains("冰川")),
            "名词释义应含「冰川」，实际：{:?}",
            noun.meanings
        );
        let verb = entry
            .definitions
            .iter()
            .find(|d| d.pos.as_deref() == Some("vt."))
            .expect("应含及物动词词性分组");
        assert!(
            verb.meanings.iter().any(|m| m.contains("测试动词义")),
            "动词释义应含「测试动词义」，实际：{:?}",
            verb.meanings
        );
        assert!(
            entry.inflections.iter().any(|i| i == "glaciers"),
            "词形应含复数 glaciers，实际：{:?}",
            entry.inflections
        );
    }

    #[test]
    fn lookup_miss_returns_none() {
        let (_dir, path) = fixture_db(&[("glacier", "", "n. 冰川", "")]);
        let db = EcdictDb::new(path);

        let result = db.lookup("notarealword").expect("查询不应出错");
        assert!(result.is_none(), "未收录词应返回 None，实际：{result:?}");
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let (_dir, path) = fixture_db(&[("Glacier", "ˈɡleɪʃər", "n. 冰川", "")]);
        let db = EcdictDb::new(path);

        // 库内存大写首字母，用全小写查应命中（LOWER(word)=LOWER(?)）。
        let entry = db
            .lookup("glacier")
            .expect("查询不应出错")
            .expect("大小写不敏感应命中");
        assert!(
            entry
                .definitions
                .iter()
                .flat_map(|d| &d.meanings)
                .any(|m| m.contains("冰川")),
            "应命中并含释义「冰川」，实际：{:?}",
            entry.definitions
        );
    }

    #[test]
    fn lookup_missing_db_returns_network_error() {
        let db = EcdictDb::new("/nonexistent/path/ecdict.db");
        let err = db.lookup("glacier");
        assert!(
            matches!(err, Err(TranslateError::Network(_))),
            "库文件缺失应返回 Network 错误，实际：{err:?}"
        );
    }

    #[test]
    fn lookup_empty_translation_treated_as_miss() {
        // word 存在但 translation 为空白——视为未命中（避免返回空词条）。
        let (_dir, path) = fixture_db(&[("glacier", "ˈɡleɪʃər", "   ", "s:glaciers")]);
        let db = EcdictDb::new(path);

        let result = db.lookup("glacier").expect("查询不应出错");
        assert!(
            result.is_none(),
            "空 translation 应视为未命中返回 None，实际：{result:?}"
        );
    }

    #[test]
    fn lookup_parses_exchange_into_multiple_inflections() {
        let (_dir, path) = fixture_db(&[("go", "ɡoʊ", "v. 去", "p:went/d:gone/i:going/3:goes")]);
        let db = EcdictDb::new(path);

        let entry = db.lookup("go").expect("查询不应出错").expect("应命中");
        for expected in ["went", "gone", "going", "goes"] {
            assert!(
                entry.inflections.iter().any(|i| i == expected),
                "词形应含 {expected}，实际：{:?}",
                entry.inflections
            );
        }
    }
}
