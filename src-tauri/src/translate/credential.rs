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

    /// 删除 secret；未找到时视为成功（幂等）
    fn delete_secret(&self, provider_id: &str, field_key: &str) -> Result<(), CredError>;
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

    /// keyring 3.6.3 删除方法：`entry.delete_credential()`；
    /// `NoEntry` 错误视为成功（凭据已不存在，幂等）。
    fn delete_secret(&self, provider_id: &str, field_key: &str) -> Result<(), CredError> {
        let account = keychain_account(provider_id, field_key);
        let entry = keyring::Entry::new("io.quickquick.app", &account)
            .map_err(|e| CredError::Keychain(e.to_string()))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(CredError::Keychain(e.to_string())),
        }
    }
}

/// 开发期文件凭据库（debug-only，release 构建用 KeyringCredStore）。
///
/// # 为什么需要它
/// 与 FileKeyProvider 同因：dev 反复重编导致 macOS 钥匙串「始终允许」失效、反复弹密码。
/// debug 构建改把翻译 secret 存本地 JSON 文件、完全绕开 OS 钥匙串。
///
/// # 安全约束
/// - 仅 `#[cfg(debug_assertions)]` 编译，绝不进入 release 分发二进制。
/// - JSON 文件权限设为 `0600`（仅属主可读写）。
/// - 文件就是 dev 密钥库：secret 仍是 secret，只是后端从钥匙串换成受限文件。
///
/// # 存储格式
/// `HashMap<account, value>` 序列化为 JSON，account 复用 `keychain_account`
/// （`cred.<pid>.<key>`），与 KeyringCredStore 的 account 命名一致。
#[cfg(debug_assertions)]
pub struct FileCredStore {
    /// 凭据 JSON 文件完整路径（config_dir/dev-credentials.json）
    file_path: std::path::PathBuf,
}

#[cfg(debug_assertions)]
impl FileCredStore {
    /// dev 凭据文件名（落在 app_config_dir 下）
    const CRED_FILE_NAME: &'static str = "dev-credentials.json";

    /// 创建 FileCredStore，凭据文件落在 `config_dir/dev-credentials.json`。
    pub fn new(config_dir: &std::path::Path) -> Self {
        Self {
            file_path: config_dir.join(Self::CRED_FILE_NAME),
        }
    }

    /// 读取全部凭据表；文件不存在时返回空表（视为尚无凭据，非错误）。
    fn read_all(&self) -> Result<std::collections::HashMap<String, String>, CredError> {
        match std::fs::read(&self.file_path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| CredError::Keychain(format!("dev 凭据文件解析失败：{e}"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(std::collections::HashMap::new())
            }
            Err(e) => Err(CredError::Keychain(e.to_string())),
        }
    }

    /// 写入全部凭据表并设权限 0600（unix）。非 unix 平台跳过权限设置。
    fn write_all(&self, map: &std::collections::HashMap<String, String>) -> Result<(), CredError> {
        let json = serde_json::to_vec_pretty(map)
            .map_err(|e| CredError::Keychain(format!("dev 凭据序列化失败：{e}")))?;
        std::fs::write(&self.file_path, json).map_err(|e| CredError::Keychain(e.to_string()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.file_path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| CredError::Keychain(e.to_string()))?;
        }

        Ok(())
    }
}

#[cfg(debug_assertions)]
impl CredStore for FileCredStore {
    fn set_secret(&self, provider_id: &str, field_key: &str, value: &str) -> Result<(), CredError> {
        let mut map = self.read_all()?;
        map.insert(keychain_account(provider_id, field_key), value.to_string());
        self.write_all(&map)
    }

    fn get_secret(&self, provider_id: &str, field_key: &str) -> Result<Option<String>, CredError> {
        let map = self.read_all()?;
        Ok(map.get(&keychain_account(provider_id, field_key)).cloned())
    }

    fn delete_secret(&self, provider_id: &str, field_key: &str) -> Result<(), CredError> {
        let mut map = self.read_all()?;
        // 仅当确实删掉了键才重写文件：否则删不存在的键会凭空建出空 dev-credentials.json，
        // 与「幂等 no-op」语义矛盾（也避免在文件本不存在时无谓落盘）。
        let removed = map.remove(&keychain_account(provider_id, field_key));
        if removed.is_some() {
            self.write_all(&map)?;
        }
        Ok(())
    }
}

/// 构造默认 CredStore：debug 用文件库、release 用钥匙串。
///
/// 把 cfg 选择逻辑收敛到单点，避免在 3 个命令调用点各写一遍 cfg。
/// `config_dir` 仅 debug 分支需要（FileCredStore 的文件位置）；release 分支忽略。
#[cfg(debug_assertions)]
pub fn default_cred_store(config_dir: &std::path::Path) -> impl CredStore {
    FileCredStore::new(config_dir)
}

/// 构造默认 CredStore（release：钥匙串）。`config_dir` 在此分支不使用。
#[cfg(not(debug_assertions))]
pub fn default_cred_store(_config_dir: &std::path::Path) -> impl CredStore {
    KeyringCredStore
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
            // 写存在标记，使设置页能只读 DB 判断"已配置"而不碰 keychain
            write_secret_marker(conn, provider_id, field_key)?;
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

/// 删除 provider 全部已保存凭据（store + DB 均清）。
///
/// 按 schema 遍历：is_secret 字段调 `store.delete_secret`；非密字段 DELETE FROM DB。
/// 幂等——字段不存在也不报错。
///
/// # Errors
/// - `CredError::UnknownProvider`：provider_id 不在任何已知 schema 中
/// - `CredError::Keychain`：store 删除失败（NoEntry 已在 store 层视为成功）
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
            // 先删标记、再删 keychain：若 keychain 删除半途失败，标记已先清，
            // is_set 偏向保守的 false（未配置），而非谎报"已配置"且无自愈。
            delete_secret_marker(conn, provider_id, field.key)?;
            store.delete_secret(provider_id, field.key)?;
        } else {
            delete_from_db(conn, provider_id, field.key)?;
        }
    }

    Ok(())
}

/// 读取 provider 凭据用于设置页展示——只读 DB，绝不碰 keychain。
///
/// 与 `load_credentials` 的区别：本函数**不接受也不使用 store 参数**，从而
/// 不会触发 keychain 弹密码。secret 字段靠 `secret_presence` 标记表判断是否已配置，
/// 已配置则返回空串值（仅表示"已配置"，不回明文）；非密字段读加密 DB 取实际值。
///
/// # 迁移策略（不回填）
/// 展示路径永不读 keychain。本改动落地前已存入 keychain、但无标记行的 secret，
/// 会被显示为"待配置"，需重存一次以注册标记。预发布期、单用户，刻意为之。
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
            if secret_marker_exists(conn, provider_id, field.key)? {
                result.push((field.key.to_string(), String::new()));
            }
        } else if let Some(val) = read_from_db(conn, provider_id, field.key)? {
            result.push((field.key.to_string(), val));
        }
    }

    Ok(result)
}

/// 写入 secret 存在标记（INSERT OR REPLACE，幂等）。仅记录键，绝不存值。
fn write_secret_marker(
    conn: &Connection,
    provider_id: &str,
    field_key: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO secret_presence (provider_id, field_key) VALUES (?1, ?2)",
        rusqlite::params![provider_id, field_key],
    )?;
    Ok(())
}

/// 删除 secret 存在标记（幂等，不存在时不报错）。
fn delete_secret_marker(
    conn: &Connection,
    provider_id: &str,
    field_key: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM secret_presence WHERE provider_id = ?1 AND field_key = ?2",
        rusqlite::params![provider_id, field_key],
    )?;
    Ok(())
}

/// 查询某 secret 是否有存在标记（已配置）。
fn secret_marker_exists(
    conn: &Connection,
    provider_id: &str,
    field_key: &str,
) -> Result<bool, rusqlite::Error> {
    use rusqlite::OptionalExtension;
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM secret_presence WHERE provider_id = ?1 AND field_key = ?2",
            rusqlite::params![provider_id, field_key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
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

/// 从加密 DB 删除非密字段（幂等，不存在时不报错）。
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
impl Default for MockCredStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl CredStore for MockCredStore {
    fn set_secret(&self, provider_id: &str, field_key: &str, value: &str) -> Result<(), CredError> {
        let key = keychain_account(provider_id, field_key);
        self.store.lock().unwrap().insert(key, value.to_string());
        Ok(())
    }

    fn get_secret(&self, provider_id: &str, field_key: &str) -> Result<Option<String>, CredError> {
        let key = keychain_account(provider_id, field_key);
        Ok(self.store.lock().unwrap().get(&key).cloned())
    }

    fn delete_secret(&self, provider_id: &str, field_key: &str) -> Result<(), CredError> {
        let key = keychain_account(provider_id, field_key);
        self.store.lock().unwrap().remove(&key);
        Ok(())
    }
}

// 测试模块本身也需 debug_assertions 门控：FileCredStore 是 debug-only 类型，
// release 测试构建下不存在，若仅 #[cfg(test)] 会导致 cargo test --release 编译失败（E0433）。
#[cfg(all(test, debug_assertions))]
mod file_cred_store_tests {
    use super::*;
    use tempfile::tempdir;

    /// set 后 get 应取回同一值（往返）
    #[test]
    fn file_store_set_get_roundtrip() {
        let dir = tempdir().unwrap();
        let store = FileCredStore::new(dir.path());

        store.set_secret("baidu", "secret_key", "sk1").unwrap();

        assert_eq!(
            store.get_secret("baidu", "secret_key").unwrap(),
            Some("sk1".to_string()),
            "set 后 get 应取回同一值"
        );
    }

    /// get 未命中返回 None（不算错误）
    #[test]
    fn file_store_get_missing_returns_none() {
        let dir = tempdir().unwrap();
        let store = FileCredStore::new(dir.path());

        assert_eq!(
            store.get_secret("baidu", "secret_key").unwrap(),
            None,
            "未命中应返回 None"
        );
    }

    /// delete 后 get 返回 None
    #[test]
    fn file_store_delete_removes_value() {
        let dir = tempdir().unwrap();
        let store = FileCredStore::new(dir.path());
        store.set_secret("baidu", "secret_key", "sk1").unwrap();

        store.delete_secret("baidu", "secret_key").unwrap();

        assert_eq!(
            store.get_secret("baidu", "secret_key").unwrap(),
            None,
            "delete 后应取不到值"
        );
    }

    /// delete 不存在的键视为成功（幂等），且不应建立空的凭据文件
    #[test]
    fn file_store_delete_missing_is_ok_and_creates_no_file() {
        let dir = tempdir().unwrap();
        let store = FileCredStore::new(dir.path());

        let result = store.delete_secret("baidu", "nonexistent");

        assert!(result.is_ok(), "删除不存在的键应成功（幂等）: {result:?}");
        assert!(
            !dir.path().join("dev-credentials.json").exists(),
            "删除不存在的键不应建立空凭据文件（真 no-op）"
        );
    }

    /// 凭据文件内容损坏（非法 JSON）时 get_secret 返回 Err 而非 panic
    #[test]
    fn file_store_corrupt_json_returns_err() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("dev-credentials.json"), b"{\"bad").unwrap();
        let store = FileCredStore::new(dir.path());

        let result = store.get_secret("baidu", "secret_key");

        assert!(
            result.is_err(),
            "损坏 JSON 应优雅返回 Err，不 panic: {result:?}"
        );
    }

    /// 0 字节空文件时 get_secret 返回 Err 而非 panic
    #[test]
    fn file_store_empty_file_returns_err() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("dev-credentials.json"), b"").unwrap();
        let store = FileCredStore::new(dir.path());

        let result = store.get_secret("baidu", "secret_key");

        assert!(
            result.is_err(),
            "0 字节空文件应优雅返回 Err，不 panic: {result:?}"
        );
    }

    /// 新实例从文件读出此前持久化的值（跨实例持久化）
    #[test]
    fn file_store_persists_across_instances() {
        let dir = tempdir().unwrap();
        FileCredStore::new(dir.path())
            .set_secret("deepl_free", "auth_key", "ak1")
            .unwrap();

        let reloaded = FileCredStore::new(dir.path())
            .get_secret("deepl_free", "auth_key")
            .unwrap();

        assert_eq!(reloaded, Some("ak1".to_string()), "新实例应从文件读出旧值");
    }

    /// 同键二次 set 覆盖旧值
    #[test]
    fn file_store_overwrites_existing_value() {
        let dir = tempdir().unwrap();
        let store = FileCredStore::new(dir.path());
        store.set_secret("google", "api_key", "old").unwrap();

        store.set_secret("google", "api_key", "new").unwrap();

        assert_eq!(
            store.get_secret("google", "api_key").unwrap(),
            Some("new".to_string()),
            "二次 set 应覆盖为新值"
        );
    }

    /// unix 下凭据文件权限应为 0600
    #[cfg(unix)]
    #[test]
    fn file_store_sets_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let store = FileCredStore::new(dir.path());

        store.set_secret("baidu", "secret_key", "sk1").unwrap();

        let mode = std::fs::metadata(dir.path().join("dev-credentials.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "凭据文件权限应为 0600");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS provider_config (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
                value        TEXT NOT NULL,
                PRIMARY KEY (provider_id, field_key)
            );
            CREATE TABLE IF NOT EXISTS secret_presence (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
                PRIMARY KEY (provider_id, field_key)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn save_credentials_writes_secret_marker() {
        let store = MockCredStore::new();
        let conn = make_test_db();
        let values = vec![("app_id", "id1"), ("secret_key", "sk1")];

        save_credentials("baidu", &values, &store, &conn).unwrap();

        assert!(
            secret_marker_exists(&conn, "baidu", "secret_key").unwrap(),
            "保存 secret 后应写入存在标记"
        );
        assert!(
            !secret_marker_exists(&conn, "baidu", "app_id").unwrap(),
            "非密字段不应写入 secret 标记"
        );
    }

    #[test]
    fn delete_credentials_removes_secret_marker() {
        let store = MockCredStore::new();
        let conn = make_test_db();
        save_credentials("baidu", &[("secret_key", "sk1")], &store, &conn).unwrap();
        assert!(secret_marker_exists(&conn, "baidu", "secret_key").unwrap());

        delete_credentials("baidu", &store, &conn).unwrap();

        assert!(
            !secret_marker_exists(&conn, "baidu", "secret_key").unwrap(),
            "删除后 secret 标记应消失"
        );
    }

    #[test]
    fn load_for_display_reports_secret_present() {
        let store = MockCredStore::new();
        let conn = make_test_db();
        save_credentials(
            "baidu",
            &[("app_id", "id1"), ("secret_key", "sk1")],
            &store,
            &conn,
        )
        .unwrap();

        // 展示路径不接受 store 参数，完全靠 secret_presence 标记表
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
    fn load_for_display_reports_secret_absent_when_no_marker() {
        let conn = make_test_db();
        // 仅写非密字段，secret 无标记（模拟迁移前已存 keychain 但无标记的状态）
        write_to_db(&conn, "baidu", "app_id", "id1").unwrap();

        let display = load_credentials_for_display("baidu", &conn).unwrap();

        assert!(
            display.iter().all(|(k, _)| k != "secret_key"),
            "无标记的 secret 不应出现在展示结果中"
        );
    }

    #[test]
    fn marker_helpers_roundtrip() {
        let conn = make_test_db();
        assert!(!secret_marker_exists(&conn, "google", "api_key").unwrap());

        write_secret_marker(&conn, "google", "api_key").unwrap();
        assert!(secret_marker_exists(&conn, "google", "api_key").unwrap());

        // INSERT OR REPLACE：重复写不报错
        write_secret_marker(&conn, "google", "api_key").unwrap();
        assert!(secret_marker_exists(&conn, "google", "api_key").unwrap());

        delete_secret_marker(&conn, "google", "api_key").unwrap();
        assert!(!secret_marker_exists(&conn, "google", "api_key").unwrap());
    }

    #[test]
    fn delete_secret_marker_is_idempotent() {
        let conn = make_test_db();
        let result = delete_secret_marker(&conn, "google", "api_key");
        assert!(result.is_ok(), "删除不存在的标记应成功（幂等）: {result:?}");
    }

    #[test]
    fn delete_credentials_removes_secret_from_store() {
        let store = MockCredStore::new();
        let conn = make_test_db();
        store
            .set_secret("baidu", "secret_key", "supersecret")
            .unwrap();
        assert!(store.get_secret("baidu", "secret_key").unwrap().is_some());

        delete_credentials("baidu", &store, &conn).unwrap();

        assert!(
            store.get_secret("baidu", "secret_key").unwrap().is_none(),
            "删除后 secret 应从 store 移除"
        );
    }

    #[test]
    fn delete_credentials_removes_non_secret_from_db() {
        let store = MockCredStore::new();
        let conn = make_test_db();
        write_to_db(&conn, "baidu", "app_id", "my_app_id").unwrap();
        assert!(read_from_db(&conn, "baidu", "app_id").unwrap().is_some());

        delete_credentials("baidu", &store, &conn).unwrap();

        assert!(
            read_from_db(&conn, "baidu", "app_id").unwrap().is_none(),
            "删除后非密字段应从 DB 移除"
        );
    }

    #[test]
    fn delete_credentials_is_idempotent_when_nothing_stored() {
        let store = MockCredStore::new();
        let conn = make_test_db();

        let result = delete_credentials("baidu", &store, &conn);
        assert!(result.is_ok(), "删除不存在的凭据应成功（幂等）: {result:?}");
    }

    #[test]
    fn delete_credentials_unknown_provider_returns_err() {
        let store = MockCredStore::new();
        let conn = make_test_db();

        let result = delete_credentials("unknown_provider", &store, &conn);
        assert!(result.is_err(), "未知 provider 应返回 Err");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("未知 provider"),
            "错误信息应含未知 provider：{err_msg}"
        );
    }

    #[test]
    fn delete_secret_mock_removes_key() {
        let store = MockCredStore::new();
        store.set_secret("baidu", "secret_key", "val").unwrap();
        store.delete_secret("baidu", "secret_key").unwrap();
        assert!(
            store.get_secret("baidu", "secret_key").unwrap().is_none(),
            "delete_secret 后应取不到值"
        );
    }

    #[test]
    fn delete_secret_mock_nonexistent_is_ok() {
        let store = MockCredStore::new();
        let result = store.delete_secret("baidu", "nonexistent_key");
        assert!(
            result.is_ok(),
            "删除不存在的 key 应视为成功（幂等）: {result:?}"
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
            );
            CREATE TABLE IF NOT EXISTS secret_presence (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
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
