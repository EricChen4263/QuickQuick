//! 集成测试：secret 存在标记表 + 设置页只读 DB 展示路径
//!
//! 背景：ad-hoc 签名下逐个读 keychain 触发反复弹密码。设置页判断"已配置"改为
//! 只读 secret_presence 标记表，绝不碰 keychain。本测试通过公开 API 验证：
//! - secret_presence 表由 ensure_schema 预埋
//! - 保存 secret 后标记存在、展示函数报 present 且不回明文
//! - 删除后标记消失、展示函数报 absent
//! - 展示路径完全不依赖 store（仅靠 DB）

use quickquick_lib::db;
use quickquick_lib::ipc::settings::{get_provider_credentials_impl, set_provider_credentials_impl};
use quickquick_lib::translate::credential::{
    delete_credentials, load_credentials_for_display, save_credentials, CredError, CredStore,
};
use std::collections::HashMap;
use std::sync::Mutex;
use tempfile::tempdir;

/// 固定 32 字节测试密钥，不依赖钥匙串
const KEY: [u8; 32] = [7u8; 32];

/// 集成测试用内存 CredStore，不触碰 OS keychain（headless）。
struct LocalMockCredStore {
    store: Mutex<HashMap<String, String>>,
}

impl LocalMockCredStore {
    fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl CredStore for LocalMockCredStore {
    fn set_secret(&self, provider_id: &str, field_key: &str, value: &str) -> Result<(), CredError> {
        let key = format!("{provider_id}.{field_key}");
        self.store.lock().unwrap().insert(key, value.to_string());
        Ok(())
    }

    fn get_secret(&self, provider_id: &str, field_key: &str) -> Result<Option<String>, CredError> {
        let key = format!("{provider_id}.{field_key}");
        Ok(self.store.lock().unwrap().get(&key).cloned())
    }

    fn delete_secret(&self, provider_id: &str, field_key: &str) -> Result<(), CredError> {
        let key = format!("{provider_id}.{field_key}");
        self.store.lock().unwrap().remove(&key);
        Ok(())
    }
}

/// 打开临时加密数据库，schema 已初始化（含 secret_presence 表）
fn open_tmp_db() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempdir().expect("tempdir 创建失败");
    let path = dir.path().join("test_secret_presence.db");
    let conn = db::open_or_create(&path, &KEY).expect("建库应成功");
    (dir, conn)
}

#[test]
fn ensure_schema_preembeds_secret_presence_table() {
    let (_dir, conn) = open_tmp_db();

    // 表存在则该查询不报 no such table
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM secret_presence", [], |row| row.get(0))
        .expect("secret_presence 表应由 ensure_schema 预埋");
    assert_eq!(count, 0, "新建库 secret_presence 应为空");
}

#[test]
fn save_then_display_reports_secret_present_without_store() {
    let (_dir, conn) = open_tmp_db();
    let store = LocalMockCredStore::new();
    save_credentials(
        "baidu",
        &[("app_id", "id1"), ("secret_key", "sk1")],
        &store,
        &conn,
    )
    .expect("保存应成功");

    // 展示路径不接受 store——完全靠 secret_presence 标记表
    let display = load_credentials_for_display("baidu", &conn).expect("展示读取应成功");

    let secret = display
        .iter()
        .find(|(k, _)| k == "secret_key")
        .expect("已配置 secret 应出现在展示结果中");
    assert_eq!(secret.1, "", "secret 展示值应为空串（不回明文）");
    let app_id = display.iter().find(|(k, _)| k == "app_id");
    assert_eq!(
        app_id.map(|(_, v)| v.as_str()),
        Some("id1"),
        "非密字段展示值应为实际值"
    );
}

#[test]
fn delete_then_display_reports_secret_absent() {
    let (_dir, conn) = open_tmp_db();
    let store = LocalMockCredStore::new();
    save_credentials("baidu", &[("secret_key", "sk1")], &store, &conn).expect("保存应成功");

    delete_credentials("baidu", &store, &conn).expect("删除应成功");

    let display = load_credentials_for_display("baidu", &conn).expect("展示读取应成功");
    assert!(
        display.iter().all(|(k, _)| k != "secret_key"),
        "删除后 secret 不应出现在展示结果中"
    );
}

#[test]
fn get_provider_credentials_impl_reports_set_from_db_only() {
    let (_dir, conn) = open_tmp_db();
    let store = LocalMockCredStore::new();
    let mut values = HashMap::new();
    values.insert("app_id".to_string(), "id1".to_string());
    values.insert("secret_key".to_string(), "sk1".to_string());
    set_provider_credentials_impl("baidu", values, &store, &conn).expect("保存应成功");

    // 命令接线后不再传 store：仅靠 DB 标记报 is_set
    let results = get_provider_credentials_impl("baidu", &conn).expect("读取应成功");

    let secret = results.iter().find(|r| r.key == "secret_key").unwrap();
    assert!(
        secret.is_set,
        "已配置 secret 应 is_set=true（仅靠 DB 标记）"
    );
    assert!(secret.value.is_none(), "secret 的 value 永远应为 None");

    let app_id = results.iter().find(|r| r.key == "app_id").unwrap();
    assert!(app_id.is_set, "非密字段已存应 is_set=true");
    assert_eq!(app_id.value, Some("id1".to_string()), "非密字段返回实际值");
}

/// keychain 删除恒失败的 store，用于验证 delete_credentials 的失败方向。
struct DeleteFailingCredStore;

impl CredStore for DeleteFailingCredStore {
    fn set_secret(&self, _: &str, _: &str, _: &str) -> Result<(), CredError> {
        Ok(())
    }
    fn get_secret(&self, _: &str, _: &str) -> Result<Option<String>, CredError> {
        Ok(None)
    }
    fn delete_secret(&self, _: &str, _: &str) -> Result<(), CredError> {
        Err(CredError::Keychain("模拟 keychain 删除失败".to_string()))
    }
}

#[test]
fn delete_clears_marker_before_keychain_so_failure_biases_unconfigured() {
    // 为什么：delete 必须先删标记再删 keychain。若顺序反了，keychain 删除失败时
    // 标记残留 → is_set=true 谎报"已配置"且无自愈。先删标记可保证任何半途失败
    // 都偏向保守的 is_set=false（与 save 失败方向语义一致）。
    let (_dir, conn) = open_tmp_db();
    save_credentials(
        "baidu",
        &[("secret_key", "sk1")],
        &LocalMockCredStore::new(),
        &conn,
    )
    .expect("保存应成功");

    // keychain 删除会失败，故 delete_credentials 整体返回 Err
    let result = delete_credentials("baidu", &DeleteFailingCredStore, &conn);
    assert!(result.is_err(), "keychain 删除失败时整体应返回 Err");

    // 关键断言：尽管整体失败，标记必须已先被删除，展示不再报"已配置"
    let display = load_credentials_for_display("baidu", &conn).expect("展示读取应成功");
    assert!(
        display.iter().all(|(k, _)| k != "secret_key"),
        "删除标记应先于 keychain 执行——失败时 is_set 应偏向 false（未配置）"
    );
}

#[test]
fn load_for_display_unknown_provider_returns_empty_by_design() {
    // 展示路径对未知 provider 刻意返回空 Vec（而非 UnknownProvider 错误）：
    // 它由 credential_schema 驱动，未知 provider 的 schema 为空 → 自然无字段可展示。
    // 与 save/delete 的写路径不对称是有意为之——展示是只读、纯渲染，无写副作用，
    // 对未知 provider 渲染成"无字段"比抛错更符合 UI 容错语义。
    let (_dir, conn) = open_tmp_db();

    let display = load_credentials_for_display("nonexistent_provider", &conn)
        .expect("展示读取不应对未知 provider 报错");

    assert!(display.is_empty(), "未知 provider 展示结果应为空 Vec");
}
