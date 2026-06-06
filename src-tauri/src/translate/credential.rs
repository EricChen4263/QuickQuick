//! 凭据配置模块：provider 结构化字段 schema + secret/非密字段分表存进加密 DB
//!
//! 设计对齐 docs/design/local-keystore-no-keychain.md §四#2/#3：
//! - secret 字段（is_secret=true）存 `provider_secret` 表（SQLCipher 整库加密，去 Keychain）
//! - 非密字段（is_secret=false）存 `provider_config` 表
//! - 路由由 `credential_schema` 的 `is_secret` 判定，secret 绝不与非密字段混表
//!
//! 安全约定：
//! - secret 值进的是整库 AES-256 加密的 DB，不再碰 OS 钥匙串、连明文 JSON 都不留
//! - 任何地方均不打印 secret 字段的值（日志安全）
//! - 展示路径用 `SELECT 1 FROM provider_secret` 判断「已配置」，绝不回明文
//!
//! 测试隔离策略：
//! - 生产路径：`DbCredStore`（读写加密 DB 的 provider_secret 表）
//! - 单元测试：`MockCredStore`（内存 HashMap，证明 trait 抽象）+ `DbCredStore`（in-memory 加密库往返）

use rusqlite::Connection;
use thiserror::Error;

/// 凭据字段描述符。
///
/// 每个 provider 声明一组字段，框架据此动态渲染表单并路由存取。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialField {
    /// 字段标识符（唯一键，用于存取路由）
    pub key: &'static str,
    /// UI 显示标签
    pub label: &'static str,
    /// 是否为 secret——true 时存 provider_secret，false 时存 provider_config
    pub is_secret: bool,
    /// 是否必填
    pub required: bool,
}

/// 凭据操作错误
#[derive(Debug, Error)]
pub enum CredError {
    /// secret 存储后端操作失败（CredStore 实现返回的非 DB 错误）
    #[error("secret 存储失败：{0}")]
    Store(String),

    /// DB 操作失败
    #[error("数据库操作失败：{0}")]
    Db(#[from] rusqlite::Error),

    /// 未知 provider_id（不在任何已知 provider 的 schema 中）
    #[error("未知 provider：{0}")]
    UnknownProvider(String),

    /// 未知 field_key（该字段不在对应 provider 的 schema 中）
    ///
    /// 携带 provider_id 和 field_key，便于调用方定位问题；不携带字段值（安全约定）。
    #[error("provider '{provider}' 不存在字段 '{field}'")]
    UnknownField {
        /// provider 标识
        provider: String,
        /// 非法字段 key
        field: String,
    },
}

/// Secret 存储抽象 trait——便于生产和测试使用不同后端。
///
/// 生产实现：`DbCredStore`（读写加密 DB 的 provider_secret 表）
/// 测试实现：`MockCredStore`（内存 HashMap）
pub trait CredStore {
    /// 写入 secret（field_value 不得出现在任何日志/错误消息中）
    fn set_secret(&self, provider_id: &str, field_key: &str, value: &str) -> Result<(), CredError>;

    /// 读取 secret；未找到时返回 None（不算错误）
    fn get_secret(&self, provider_id: &str, field_key: &str) -> Result<Option<String>, CredError>;

    /// 删除 secret；未找到时视为成功（幂等）
    fn delete_secret(&self, provider_id: &str, field_key: &str) -> Result<(), CredError>;
}

/// 生产用 CredStore：把 secret 读写进加密 DB 的 `provider_secret` 表。
///
/// 持有 `&Connection`（在命令层 `with_db` 闭包内构造，与 `save_credentials(...conn)` 共用同一连接）。
/// secret 进整库加密的 DB，不碰 OS 钥匙串，故展示/读写均不触发任何密码弹窗。
pub struct DbCredStore<'a> {
    /// 加密 DB 连接（provider_secret 表所在）
    conn: &'a Connection,
}

impl<'a> DbCredStore<'a> {
    /// 用已开的加密 DB 连接构造 DbCredStore。
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl CredStore for DbCredStore<'_> {
    fn set_secret(&self, provider_id: &str, field_key: &str, value: &str) -> Result<(), CredError> {
        self.conn.execute(
            "INSERT INTO provider_secret (provider_id, field_key, value)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(provider_id, field_key) DO UPDATE SET value = excluded.value",
            rusqlite::params![provider_id, field_key, value],
        )?;
        Ok(())
    }

    fn get_secret(&self, provider_id: &str, field_key: &str) -> Result<Option<String>, CredError> {
        use rusqlite::OptionalExtension;
        let value = self
            .conn
            .query_row(
                "SELECT value FROM provider_secret WHERE provider_id = ?1 AND field_key = ?2",
                rusqlite::params![provider_id, field_key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    fn delete_secret(&self, provider_id: &str, field_key: &str) -> Result<(), CredError> {
        self.conn.execute(
            "DELETE FROM provider_secret WHERE provider_id = ?1 AND field_key = ?2",
            rusqlite::params![provider_id, field_key],
        )?;
        Ok(())
    }
}

/// 返回指定 provider 的结构化凭据字段 schema。
///
/// 框架通过此函数动态渲染表单，并在 save/load 时按 is_secret 路由存取。
/// 未知 provider_id 返回空 Vec（不 panic）。
pub fn credential_schema(provider_id: &str) -> Vec<CredentialField> {
    match provider_id {
        // lingva 免 key，无凭据字段，落入 `_ => vec![]` 分支。
        "baidu" => vec![
            CredentialField {
                key: "app_id",
                label: "AppID",
                is_secret: false,
                required: true,
            },
            CredentialField {
                key: "secret_key",
                label: "Secret Key",
                is_secret: true,
                required: true,
            },
        ],
        "baidu_field" => vec![
            CredentialField {
                key: "app_id",
                label: "AppID",
                is_secret: false,
                required: true,
            },
            CredentialField {
                key: "secret_key",
                label: "Secret Key",
                is_secret: true,
                required: true,
            },
            CredentialField {
                key: "field",
                label: "领域（如 it/finance）",
                is_secret: false,
                required: true,
            },
        ],
        "youdao" => vec![
            CredentialField {
                key: "app_key",
                label: "应用 ID",
                is_secret: false,
                required: true,
            },
            CredentialField {
                key: "app_secret",
                label: "应用密钥",
                is_secret: true,
                required: true,
            },
        ],
        "caiyun" => vec![CredentialField {
            key: "token",
            label: "Token",
            is_secret: true,
            required: true,
        }],
        "niutrans" => vec![CredentialField {
            key: "apikey",
            label: "API Key",
            is_secret: true,
            required: true,
        }],
        "tencent" => vec![
            CredentialField {
                key: "secret_id",
                label: "SecretId",
                is_secret: false,
                required: true,
            },
            CredentialField {
                key: "secret_key",
                label: "SecretKey",
                is_secret: true,
                required: true,
            },
        ],
        "alibaba" => vec![
            CredentialField {
                key: "accesskey_id",
                label: "AccessKey ID",
                is_secret: false,
                required: true,
            },
            CredentialField {
                key: "accesskey_secret",
                label: "AccessKey Secret",
                is_secret: true,
                required: true,
            },
        ],
        "volcengine" => vec![
            CredentialField {
                key: "access_key_id",
                label: "AccessKeyId",
                is_secret: false,
                required: true,
            },
            CredentialField {
                key: "secret_access_key",
                label: "SecretAccessKey",
                is_secret: true,
                required: true,
            },
            CredentialField {
                key: "region",
                label: "地域（默认 cn-north-1）",
                is_secret: false,
                required: false,
            },
        ],
        "deepl_free" => vec![CredentialField {
            key: "auth_key",
            label: "Auth Key",
            is_secret: true,
            required: true,
        }],
        "google" => vec![CredentialField {
            key: "api_key",
            label: "API Key",
            is_secret: true,
            required: true,
        }],
        "openai" => vec![
            CredentialField {
                key: "apiKey",
                label: "API Key",
                is_secret: true,
                required: true,
            },
            CredentialField {
                key: "model",
                label: "模型（如 gpt-4o-mini）",
                is_secret: false,
                required: true,
            },
            CredentialField {
                key: "base_url",
                label: "Base URL（留空用官方端点）",
                is_secret: false,
                required: false,
            },
            CredentialField {
                key: "prompt",
                label: "自定义 Prompt（留空用内置默认）",
                is_secret: false,
                required: false,
            },
        ],
        // Ollama 本地自部署无鉴权，无 apiKey 字段（needs_key=false）。
        "ollama" => vec![
            CredentialField {
                key: "model",
                label: "模型（如 llama3）",
                is_secret: false,
                required: true,
            },
            CredentialField {
                key: "base_url",
                label: "Base URL（留空用 localhost:11434）",
                is_secret: false,
                required: false,
            },
            CredentialField {
                key: "prompt",
                label: "自定义 Prompt（留空用内置默认）",
                is_secret: false,
                required: false,
            },
        ],
        _ => vec![],
    }
}

/// 保存 provider 凭据：按 schema 路由——secret→provider_secret，非密→provider_config。
///
/// # 路由规则
/// - `is_secret=true` 的字段写 `store`（生产=DbCredStore→provider_secret 表）
/// - `is_secret=false` 的字段写 `provider_config` 表
///
/// # 安全
/// - secret 值绝不混入 provider_config，不在任何日志/错误消息中出现
/// - 非密值绝不写 provider_secret
/// - 路由必须由 schema 判定——未知 field_key 拒绝写入而非静默降级
///
/// # Errors
/// - `CredError::UnknownProvider`：provider_id 在任何 schema 中均不存在
/// - `CredError::UnknownField`：field_key 不在该 provider 的 schema 中
/// - `CredError::Store`：secret 写入失败
/// - `CredError::Db`：SQL 执行失败
pub fn save_credentials(
    provider_id: &str,
    values: &[(&str, &str)],
    store: &dyn CredStore,
    conn: &Connection,
) -> Result<(), CredError> {
    let schema = credential_schema(provider_id);

    if schema.is_empty() {
        return Err(CredError::UnknownProvider(provider_id.to_string()));
    }

    for (field_key, field_value) in values {
        let field =
            schema
                .iter()
                .find(|f| f.key == *field_key)
                .ok_or_else(|| CredError::UnknownField {
                    provider: provider_id.to_string(),
                    field: field_key.to_string(),
                })?;

        if field.is_secret {
            store.set_secret(provider_id, field_key, field_value)?;
        } else {
            write_to_db(conn, provider_id, field_key, field_value)?;
        }
    }

    Ok(())
}

/// 读取 provider 全部已保存凭据（store + DB 均读，合并返回）。
///
/// 返回 `Vec<(field_key, value)>`，调用方按 key 查找对应字段值。
/// 未保存的字段不出现在结果中。
///
/// # Errors
/// - `CredError::Store`：secret 读取失败
/// - `CredError::Db`：SQL 执行失败
pub fn load_credentials(
    provider_id: &str,
    store: &dyn CredStore,
    conn: &Connection,
) -> Result<Vec<(String, String)>, CredError> {
    let schema = credential_schema(provider_id);

    let mut result = Vec::new();

    for field in &schema {
        if field.is_secret {
            if let Some(val) = store.get_secret(provider_id, field.key)? {
                result.push((field.key.to_string(), val));
            }
        } else if let Some(val) = read_from_db(conn, provider_id, field.key)? {
            result.push((field.key.to_string(), val));
        }
    }

    Ok(result)
}

/// 删除 provider 全部已保存凭据（store + DB 均清）。
///
/// 按 schema 遍历：is_secret 字段调 `store.delete_secret`；非密字段 DELETE FROM provider_config。
/// 幂等——字段不存在也不报错。
///
/// # Errors
/// - `CredError::UnknownProvider`：provider_id 不在任何已知 schema 中
/// - `CredError::Store`：secret 删除失败
/// - `CredError::Db`：SQL 执行失败
pub fn delete_credentials(
    provider_id: &str,
    store: &dyn CredStore,
    conn: &Connection,
) -> Result<(), CredError> {
    let schema = credential_schema(provider_id);

    if schema.is_empty() {
        return Err(CredError::UnknownProvider(provider_id.to_string()));
    }

    for field in &schema {
        if field.is_secret {
            store.delete_secret(provider_id, field.key)?;
        } else {
            delete_from_db(conn, provider_id, field.key)?;
        }
    }

    Ok(())
}

/// 读取 provider 凭据用于设置页展示——只读加密 DB，绝不回 secret 明文。
///
/// secret 字段用 `SELECT 1 FROM provider_secret` 判断是否已配置：已配置则返回空串值
/// （仅表示「已配置」，不回明文）；非密字段读 provider_config 取实际值。
/// secret 进的是整库加密 DB（不碰钥匙串），故展示路径不触发任何密码弹窗。
///
/// # 返回
/// `Vec<(field_key, value)>`：secret 字段 value 为空串，非密字段为实际值。
/// 未配置的字段不出现在结果中。
///
/// # Errors
/// - `CredError::Db`：SQL 执行失败
pub fn load_credentials_for_display(
    provider_id: &str,
    conn: &Connection,
) -> Result<Vec<(String, String)>, CredError> {
    let schema = credential_schema(provider_id);

    let mut result = Vec::new();

    for field in &schema {
        if field.is_secret {
            if secret_exists(conn, provider_id, field.key)? {
                result.push((field.key.to_string(), String::new()));
            }
        } else if let Some(val) = read_from_db(conn, provider_id, field.key)? {
            result.push((field.key.to_string(), val));
        }
    }

    Ok(result)
}

/// 查询某 secret 是否已存在于 provider_secret 表（已配置），只取存在性、不回值。
fn secret_exists(
    conn: &Connection,
    provider_id: &str,
    field_key: &str,
) -> Result<bool, rusqlite::Error> {
    use rusqlite::OptionalExtension;
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM provider_secret WHERE provider_id = ?1 AND field_key = ?2",
            rusqlite::params![provider_id, field_key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// 将非密字段写入加密 DB 的 provider_config 表（UPSERT 语义）。
fn write_to_db(
    conn: &Connection,
    provider_id: &str,
    field_key: &str,
    value: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO provider_config (provider_id, field_key, value)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(provider_id, field_key) DO UPDATE SET value = excluded.value",
        rusqlite::params![provider_id, field_key, value],
    )?;
    Ok(())
}

/// 从加密 DB 的 provider_config 表读取非密字段。未找到时返回 None。
fn read_from_db(
    conn: &Connection,
    provider_id: &str,
    field_key: &str,
) -> Result<Option<String>, rusqlite::Error> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT value FROM provider_config WHERE provider_id = ?1 AND field_key = ?2",
        rusqlite::params![provider_id, field_key],
        |row| row.get(0),
    )
    .optional()
}

/// 从加密 DB 的 provider_config 表删除非密字段（幂等，不存在时不报错）。
fn delete_from_db(
    conn: &Connection,
    provider_id: &str,
    field_key: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM provider_config WHERE provider_id = ?1 AND field_key = ?2",
        rusqlite::params![provider_id, field_key],
    )?;
    Ok(())
}

/// 测试用内存 CredStore，不触碰任何持久化（headless CI/单元测试专用）。
///
/// 仅在 `#[cfg(test)]` 下编译，不进入生产二进制。
/// 集成测试和单元测试均可 import 使用，用于证明 `CredStore` trait 抽象。
#[cfg(test)]
pub struct MockCredStore {
    store: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

#[cfg(test)]
impl MockCredStore {
    pub fn new() -> Self {
        Self {
            store: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn account(provider_id: &str, field_key: &str) -> String {
        format!("{provider_id}.{field_key}")
    }
}

#[cfg(test)]
impl Default for MockCredStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl CredStore for MockCredStore {
    fn set_secret(&self, provider_id: &str, field_key: &str, value: &str) -> Result<(), CredError> {
        let key = Self::account(provider_id, field_key);
        self.store.lock().unwrap().insert(key, value.to_string());
        Ok(())
    }

    fn get_secret(&self, provider_id: &str, field_key: &str) -> Result<Option<String>, CredError> {
        let key = Self::account(provider_id, field_key);
        Ok(self.store.lock().unwrap().get(&key).cloned())
    }

    fn delete_secret(&self, provider_id: &str, field_key: &str) -> Result<(), CredError> {
        let key = Self::account(provider_id, field_key);
        self.store.lock().unwrap().remove(&key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 建内存库并建 provider_config + provider_secret 两表（与 ensure_schema 一致）。
    fn make_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS provider_config (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
                value        TEXT NOT NULL,
                PRIMARY KEY (provider_id, field_key)
            );
            CREATE TABLE IF NOT EXISTS provider_secret (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
                value        TEXT NOT NULL,
                PRIMARY KEY (provider_id, field_key)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn db_cred_store_set_get_roundtrip() {
        let conn = make_test_db();
        let store = DbCredStore::new(&conn);

        store.set_secret("baidu", "secret_key", "sk1").unwrap();

        assert_eq!(
            store.get_secret("baidu", "secret_key").unwrap(),
            Some("sk1".to_string()),
            "set 后 get 应取回同一值"
        );
    }

    #[test]
    fn db_cred_store_get_missing_returns_none() {
        let conn = make_test_db();
        let store = DbCredStore::new(&conn);

        assert_eq!(
            store.get_secret("baidu", "secret_key").unwrap(),
            None,
            "未命中应返回 None"
        );
    }

    #[test]
    fn db_cred_store_overwrites_existing_value() {
        let conn = make_test_db();
        let store = DbCredStore::new(&conn);
        store.set_secret("google", "api_key", "old").unwrap();

        store.set_secret("google", "api_key", "new").unwrap();

        assert_eq!(
            store.get_secret("google", "api_key").unwrap(),
            Some("new".to_string()),
            "二次 set 应覆盖为新值"
        );
    }

    #[test]
    fn db_cred_store_delete_removes_value() {
        let conn = make_test_db();
        let store = DbCredStore::new(&conn);
        store.set_secret("baidu", "secret_key", "sk1").unwrap();

        store.delete_secret("baidu", "secret_key").unwrap();

        assert_eq!(
            store.get_secret("baidu", "secret_key").unwrap(),
            None,
            "delete 后应取不到值"
        );
    }

    #[test]
    fn db_cred_store_delete_missing_is_idempotent() {
        let conn = make_test_db();
        let store = DbCredStore::new(&conn);

        let result = store.delete_secret("baidu", "nonexistent");

        assert!(result.is_ok(), "删除不存在的键应成功（幂等）: {result:?}");
    }

    #[test]
    fn save_credentials_routes_secret_to_provider_secret_table() {
        let store_conn = make_test_db();
        let store = DbCredStore::new(&store_conn);
        let values = vec![("app_id", "id1"), ("secret_key", "sk1")];

        save_credentials("baidu", &values, &store, &store_conn).unwrap();

        // secret 进 provider_secret，不进 provider_config
        let secret: String = store_conn
            .query_row(
                "SELECT value FROM provider_secret WHERE provider_id='baidu' AND field_key='secret_key'",
                [],
                |r| r.get(0),
            )
            .expect("secret 应写入 provider_secret 表");
        assert_eq!(secret, "sk1", "provider_secret 应存 secret 实际值");

        let secret_in_config: Option<String> = read_from_db(&store_conn, "baidu", "secret_key").unwrap();
        assert!(secret_in_config.is_none(), "secret 不应写入 provider_config");

        // 非密字段进 provider_config，不进 provider_secret
        let app_id = read_from_db(&store_conn, "baidu", "app_id").unwrap();
        assert_eq!(app_id, Some("id1".to_string()), "非密字段应写入 provider_config");
        assert!(
            store.get_secret("baidu", "app_id").unwrap().is_none(),
            "非密字段不应写入 provider_secret"
        );
    }

    #[test]
    fn load_for_display_reports_secret_present_without_returning_plaintext() {
        let conn = make_test_db();
        let store = DbCredStore::new(&conn);
        save_credentials("baidu", &[("app_id", "id1"), ("secret_key", "sk1")], &store, &conn)
            .unwrap();

        let display = load_credentials_for_display("baidu", &conn).unwrap();

        let secret = display.iter().find(|(k, _)| k == "secret_key");
        assert!(secret.is_some(), "已配置的 secret 应出现在展示结果中");
        assert_eq!(secret.unwrap().1, "", "secret 展示值应为空串（不回明文）");
        let app_id = display.iter().find(|(k, _)| k == "app_id");
        assert_eq!(
            app_id.map(|(_, v)| v.as_str()),
            Some("id1"),
            "非密字段展示值应为实际值"
        );
    }

    #[test]
    fn load_for_display_reports_secret_absent_when_unset() {
        let conn = make_test_db();
        write_to_db(&conn, "baidu", "app_id", "id1").unwrap();

        let display = load_credentials_for_display("baidu", &conn).unwrap();

        assert!(
            display.iter().all(|(k, _)| k != "secret_key"),
            "未配置的 secret 不应出现在展示结果中"
        );
    }

    #[test]
    fn delete_credentials_removes_secret_from_provider_secret() {
        let conn = make_test_db();
        let store = DbCredStore::new(&conn);
        save_credentials("baidu", &[("secret_key", "sk1")], &store, &conn).unwrap();
        assert!(secret_exists(&conn, "baidu", "secret_key").unwrap());

        delete_credentials("baidu", &store, &conn).unwrap();

        assert!(
            !secret_exists(&conn, "baidu", "secret_key").unwrap(),
            "删除后 provider_secret 中 secret 应消失"
        );
    }

    #[test]
    fn delete_credentials_removes_non_secret_from_db() {
        let conn = make_test_db();
        let store = DbCredStore::new(&conn);
        write_to_db(&conn, "baidu", "app_id", "my_app_id").unwrap();
        assert!(read_from_db(&conn, "baidu", "app_id").unwrap().is_some());

        delete_credentials("baidu", &store, &conn).unwrap();

        assert!(
            read_from_db(&conn, "baidu", "app_id").unwrap().is_none(),
            "删除后非密字段应从 provider_config 移除"
        );
    }

    #[test]
    fn delete_credentials_is_idempotent_when_nothing_stored() {
        let conn = make_test_db();
        let store = DbCredStore::new(&conn);

        let result = delete_credentials("baidu", &store, &conn);
        assert!(result.is_ok(), "删除不存在的凭据应成功（幂等）: {result:?}");
    }

    #[test]
    fn delete_credentials_unknown_provider_returns_err() {
        let conn = make_test_db();
        let store = DbCredStore::new(&conn);

        let result = delete_credentials("unknown_provider", &store, &conn);
        assert!(result.is_err(), "未知 provider 应返回 Err");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("未知 provider"),
            "错误信息应含未知 provider：{err_msg}"
        );
    }

    #[test]
    fn credential_schema_baidu_fields_count_is_two() {
        let fields = credential_schema("baidu");
        assert_eq!(fields.len(), 2, "百度 schema 应有 2 个字段");
    }

    #[test]
    fn credential_schema_unknown_provider_returns_empty() {
        let fields = credential_schema("nonexistent_provider");
        assert!(fields.is_empty(), "未知 provider 应返回空 schema");
    }

    #[test]
    fn mock_store_set_get_delete_roundtrip() {
        let store = MockCredStore::new();
        store.set_secret("baidu", "secret_key", "val").unwrap();
        assert_eq!(
            store.get_secret("baidu", "secret_key").unwrap(),
            Some("val".to_string())
        );
        store.delete_secret("baidu", "secret_key").unwrap();
        assert!(
            store.get_secret("baidu", "secret_key").unwrap().is_none(),
            "delete 后应取不到值"
        );
    }

    #[test]
    fn save_and_load_routes_correctly_with_mock_store() {
        let store = MockCredStore::new();
        let conn = make_test_db();
        let values = vec![("app_id", "test_app"), ("secret_key", "test_secret")];

        save_credentials("baidu", &values, &store, &conn).unwrap();

        // secret 在 store，不在 DB
        assert_eq!(
            store.get_secret("baidu", "secret_key").unwrap(),
            Some("test_secret".to_string())
        );
        assert!(read_from_db(&conn, "baidu", "secret_key").unwrap().is_none());

        // 非密在 DB，不在 store
        assert_eq!(
            read_from_db(&conn, "baidu", "app_id").unwrap(),
            Some("test_app".to_string())
        );
        assert!(store.get_secret("baidu", "app_id").unwrap().is_none());
    }

    /// 取指定 key 的字段，便于断言其 is_secret/required。
    fn field<'a>(fields: &'a [CredentialField], key: &str) -> &'a CredentialField {
        fields
            .iter()
            .find(|f| f.key == key)
            .unwrap_or_else(|| panic!("schema 应含字段 {key}"))
    }

    // 对齐 acceptance TV2-F5-A01（凭据 schema 部分）：
    // baidu_field/youdao 两源 schema 字段与 is_secret 标记正确。
    #[test]
    fn credential_schema_for_v2_keyed_sources() {
        // 百度专业：app_id（非密）、secret_key（密）、field（非密）。
        let bf = credential_schema("baidu_field");
        assert_eq!(bf.len(), 3, "baidu_field schema 应有 3 个字段");
        assert!(
            !field(&bf, "app_id").is_secret,
            "app_id 应为非密（appid 非密）"
        );
        assert!(field(&bf, "app_id").required, "app_id 应为必填");
        assert!(
            field(&bf, "secret_key").is_secret,
            "secret_key 应为 secret"
        );
        assert!(field(&bf, "secret_key").required, "secret_key 应为必填");
        assert!(
            !field(&bf, "field").is_secret,
            "field（领域）应为非密"
        );
        assert!(field(&bf, "field").required, "field 应为必填");

        // 有道：app_key（非密）、app_secret（密）。
        let yd = credential_schema("youdao");
        assert_eq!(yd.len(), 2, "youdao schema 应有 2 个字段");
        assert!(
            !field(&yd, "app_key").is_secret,
            "app_key 应为非密"
        );
        assert!(field(&yd, "app_key").required, "app_key 应为必填");
        assert!(
            field(&yd, "app_secret").is_secret,
            "app_secret 应为 secret"
        );
        assert!(field(&yd, "app_secret").required, "app_secret 应为必填");

        // 彩云：token（密、必填，唯一字段）。
        let cy = credential_schema("caiyun");
        assert_eq!(cy.len(), 1, "caiyun schema 应有 1 个字段");
        assert!(field(&cy, "token").is_secret, "token 应为 secret");
        assert!(field(&cy, "token").required, "token 应为必填");

        // 小牛：apikey（密、必填，唯一字段）。
        let nt = credential_schema("niutrans");
        assert_eq!(nt.len(), 1, "niutrans schema 应有 1 个字段");
        assert!(field(&nt, "apikey").is_secret, "apikey 应为 secret");
        assert!(field(&nt, "apikey").required, "apikey 应为必填");

        // 腾讯云：secret_id（非密）、secret_key（密），均必填。
        let tc = credential_schema("tencent");
        assert_eq!(tc.len(), 2, "tencent schema 应有 2 个字段");
        assert!(
            !field(&tc, "secret_id").is_secret,
            "secret_id 应为非密"
        );
        assert!(field(&tc, "secret_id").required, "secret_id 应为必填");
        assert!(
            field(&tc, "secret_key").is_secret,
            "secret_key 应为 secret"
        );
        assert!(field(&tc, "secret_key").required, "secret_key 应为必填");

        // 阿里：accesskey_id（非密）、accesskey_secret（密），均必填。
        let ab = credential_schema("alibaba");
        assert_eq!(ab.len(), 2, "alibaba schema 应有 2 个字段");
        assert!(
            !field(&ab, "accesskey_id").is_secret,
            "accesskey_id 应为非密"
        );
        assert!(field(&ab, "accesskey_id").required, "accesskey_id 应为必填");
        assert!(
            field(&ab, "accesskey_secret").is_secret,
            "accesskey_secret 应为 secret"
        );
        assert!(
            field(&ab, "accesskey_secret").required,
            "accesskey_secret 应为必填"
        );

        // 火山：access_key_id（非密、必填）、secret_access_key（密、必填）、region（非密、选填）。
        let vc = credential_schema("volcengine");
        assert_eq!(vc.len(), 3, "volcengine schema 应有 3 个字段");
        assert!(
            !field(&vc, "access_key_id").is_secret,
            "access_key_id 应为非密"
        );
        assert!(
            field(&vc, "access_key_id").required,
            "access_key_id 应为必填"
        );
        assert!(
            field(&vc, "secret_access_key").is_secret,
            "secret_access_key 应为 secret"
        );
        assert!(
            field(&vc, "secret_access_key").required,
            "secret_access_key 应为必填"
        );
        assert!(!field(&vc, "region").is_secret, "region 应为非密");
        assert!(
            !field(&vc, "region").required,
            "region 应为选填（有默认 cn-north-1）"
        );
    }
}
