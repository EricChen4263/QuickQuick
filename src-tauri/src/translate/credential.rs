//! 凭据配置模块：provider 结构化字段 schema + secret→keychain / 非密→加密 DB 路由
//!
//! 设计对齐设计文档§4.1#4：
//! - secret 字段（is_secret=true）写 keychain，绝不写 DB
//! - 非密字段（is_secret=false）写加密 DB，绝不占 keychain
//! - 路由正确性是 A05 的核心约束
//!
//! 测试隔离策略：
//! - 生产路径：`KeyringCredStore`（调用真实 keyring）
//! - 测试路径：`MockCredStore`（内存 HashMap，不触碰 OS keychain）
//!
//! 安全约定：
//! - 任何地方均不打印 secret 字段的值（日志安全）
//! - 生产 keyring account 格式：`cred.<provider_id>.<field_key>`

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
    /// 是否为 secret——true 时写 keychain，false 时写加密 DB
    pub is_secret: bool,
    /// 是否必填
    pub required: bool,
}

/// 凭据操作错误
#[derive(Debug, Error)]
pub enum CredError {
    /// keyring/CredStore 后端操作失败
    #[error("keychain 操作失败：{0}")]
    Keychain(String),

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
/// 生产实现：`KeyringCredStore`（调用 keyring crate）
/// 测试实现：`MockCredStore`（内存 HashMap，headless 不弹窗）
pub trait CredStore {
    /// 写入 secret（field_value 不得出现在任何日志/错误消息中）
    fn set_secret(&self, provider_id: &str, field_key: &str, value: &str) -> Result<(), CredError>;

    /// 读取 secret；未找到时返回 None（不算错误）
    fn get_secret(&self, provider_id: &str, field_key: &str) -> Result<Option<String>, CredError>;
}

/// 生产用 CredStore：调用 keyring crate 写 OS keychain。
///
/// Keychain service 固定为 "io.quickquick.app"，account 格式：`cred.<pid>.<key>`。
pub struct KeyringCredStore;

impl CredStore for KeyringCredStore {
    fn set_secret(&self, provider_id: &str, field_key: &str, value: &str) -> Result<(), CredError> {
        let account = keychain_account(provider_id, field_key);
        let entry = keyring::Entry::new("io.quickquick.app", &account)
            .map_err(|e| CredError::Keychain(e.to_string()))?;
        entry
            .set_secret(value.as_bytes())
            .map_err(|e| CredError::Keychain(e.to_string()))
    }

    fn get_secret(&self, provider_id: &str, field_key: &str) -> Result<Option<String>, CredError> {
        let account = keychain_account(provider_id, field_key);
        let entry = keyring::Entry::new("io.quickquick.app", &account)
            .map_err(|e| CredError::Keychain(e.to_string()))?;
        match entry.get_secret() {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(CredError::Keychain(e.to_string())),
        }
    }
}

/// 返回指定 provider 的结构化凭据字段 schema。
///
/// 框架通过此函数动态渲染表单，并在 save/load 时按 is_secret 路由存取。
/// 未知 provider_id 返回空 Vec（不 panic）。
pub fn credential_schema(provider_id: &str) -> Vec<CredentialField> {
    match provider_id {
        "mymemory" => vec![CredentialField {
            key: "email",
            label: "Email（可选，提高配额）",
            is_secret: false,
            required: false,
        }],
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
        _ => vec![],
    }
}

/// 保存 provider 凭据：按 schema 路由——secret→CredStore（keychain），非密→加密 DB。
///
/// # 路由规则
/// - `is_secret=true` 的字段写 `store`（生产=keyring，测试=MockCredStore）
/// - `is_secret=false` 的字段写 `provider_config` 表（由 `db::ensure_schema` 预埋）
///
/// # 安全
/// - secret 值绝不写 DB，不在任何日志/错误消息中出现
/// - 非密值绝不写 keychain/store
/// - 路由必须由 schema 判定（设计§4.1#4）——未知 field_key 拒绝写入而非静默降级
///
/// # Errors
/// - `CredError::UnknownProvider`：provider_id 在任何 schema 中均不存在
/// - `CredError::UnknownField`：field_key 不在该 provider 的 schema 中
/// - `CredError::Keychain`：store 写入失败
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
/// - `CredError::Keychain`：store 读取失败
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

/// 将非密字段写入加密 DB（UPSERT 语义）。
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

/// 从加密 DB 读取非密字段。未找到时返回 None。
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

/// 构造 keychain account 名：`cred.<provider_id>.<field_key>`
fn keychain_account(provider_id: &str, field_key: &str) -> String {
    format!("cred.{provider_id}.{field_key}")
}

/// 测试用内存 CredStore，不触碰 OS keychain（headless CI/单元测试专用）。
///
/// 仅在 `#[cfg(test)]` 下编译，不进入生产二进制。
/// 集成测试和单元测试均可 import 使用。
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
}

#[cfg(test)]
impl CredStore for MockCredStore {
    fn set_secret(&self, provider_id: &str, field_key: &str, value: &str) -> Result<(), CredError> {
        let key = format!("{provider_id}.{field_key}");
        self.store.lock().unwrap().insert(key, value.to_string());
        Ok(())
    }

    fn get_secret(&self, provider_id: &str, field_key: &str) -> Result<Option<String>, CredError> {
        let key = format!("{provider_id}.{field_key}");
        Ok(self.store.lock().unwrap().get(&key).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn keychain_account_format_is_correct() {
        let account = keychain_account("baidu", "secret_key");
        assert_eq!(account, "cred.baidu.secret_key");
    }

    #[test]
    fn save_and_load_routes_correctly_with_mock_store() {
        // Arrange
        let store = MockCredStore::new();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // provider_config 表由 db::ensure_schema 预埋；单元测试绕开 open_or_create，
        // 此处手动建表保持测试自包含。
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS provider_config (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
                value        TEXT NOT NULL,
                PRIMARY KEY (provider_id, field_key)
            );",
        )
        .unwrap();

        let values = vec![("app_id", "test_app"), ("secret_key", "test_secret")];

        // Act
        save_credentials("baidu", &values, &store, &conn).unwrap();

        // Assert：secret 在 store，不在 DB
        let kc_val = store.get_secret("baidu", "secret_key").unwrap();
        assert_eq!(kc_val, Some("test_secret".to_string()));

        let kc_app_id = store.get_secret("baidu", "app_id").unwrap();
        assert!(kc_app_id.is_none(), "非密字段不应在 store 中");

        // Assert：非密在 DB，不在 store
        let db_val = read_from_db(&conn, "baidu", "app_id").unwrap();
        assert_eq!(db_val, Some("test_app".to_string()));

        let db_secret = read_from_db(&conn, "baidu", "secret_key").unwrap();
        assert!(db_secret.is_none(), "secret 字段不应在 DB 中");
    }
}
