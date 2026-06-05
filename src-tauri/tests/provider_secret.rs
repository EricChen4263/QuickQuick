//! 集成测试：provider_secret 表（secret 进加密 DB）+ 设置页只读展示路径
//!
//! 背景：本地密钥库改造去 Keychain 后，secret 存进同一张 SQLCipher 整库加密的 DB
//! （`provider_secret` 表），不再碰钥匙串、连明文 JSON 都不留。本测试通过公开 API 验证：
//! - provider_secret 表由 ensure_schema 预埋、退役后的 secret_presence 表不复存在
//! - 保存 secret 后展示函数报 present 且不回明文
//! - 删除后展示函数报 absent
//! - 展示路径只读 DB（不回 secret 明文）

use quickquick_lib::db;
use quickquick_lib::ipc::settings::{get_provider_credentials_impl, set_provider_credentials_impl};
use quickquick_lib::translate::credential::{
    delete_credentials, load_credentials_for_display, save_credentials, DbCredStore,
};
use std::collections::HashMap;
use tempfile::tempdir;

/// 固定 32 字节测试密钥，不依赖任何机器绑定/钥匙串
const KEY: [u8; 32] = [7u8; 32];

/// 打开临时加密数据库，schema 已初始化（含 provider_secret 表）
fn open_tmp_db() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempdir().expect("tempdir 创建失败");
    let path = dir.path().join("test_provider_secret.db");
    let conn = db::open_or_create(&path, &KEY).expect("建库应成功");
    (dir, conn)
}

#[test]
fn ensure_schema_preembeds_provider_secret_table() {
    let (_dir, conn) = open_tmp_db();

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM provider_secret", [], |row| row.get(0))
        .expect("provider_secret 表应由 ensure_schema 预埋");
    assert_eq!(count, 0, "新建库 provider_secret 应为空");
}

#[test]
fn ensure_schema_drops_retired_secret_presence_table() {
    let (_dir, conn) = open_tmp_db();

    // 退役后的 secret_presence 表应不存在：查它应报 no such table
    let result = conn.query_row("SELECT COUNT(*) FROM secret_presence", [], |row| {
        row.get::<_, i64>(0)
    });
    assert!(
        result.is_err(),
        "secret_presence 表应已退役（DROP TABLE IF EXISTS），查询应报错"
    );
}

#[test]
fn save_then_display_reports_secret_present_without_plaintext() {
    let (_dir, conn) = open_tmp_db();
    let store = DbCredStore::new(&conn);
    save_credentials(
        "baidu",
        &[("app_id", "id1"), ("secret_key", "sk1")],
        &store,
        &conn,
    )
    .expect("保存应成功");

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
fn secret_value_is_stored_encrypted_and_retrievable() {
    use quickquick_lib::translate::credential::CredStore;
    let (_dir, conn) = open_tmp_db();
    let store = DbCredStore::new(&conn);

    store
        .set_secret("deepl_free", "auth_key", "ak-secret")
        .expect("写 secret 应成功");

    // secret 真值进了加密库，可经 store 取回（区别于展示路径只回空串）
    assert_eq!(
        store.get_secret("deepl_free", "auth_key").unwrap(),
        Some("ak-secret".to_string()),
        "store 应能取回写入的 secret 明文"
    );
}

#[test]
fn delete_then_display_reports_secret_absent() {
    let (_dir, conn) = open_tmp_db();
    let store = DbCredStore::new(&conn);
    save_credentials("baidu", &[("secret_key", "sk1")], &store, &conn).expect("保存应成功");

    delete_credentials("baidu", &store, &conn).expect("删除应成功");

    let display = load_credentials_for_display("baidu", &conn).expect("展示读取应成功");
    assert!(
        display.iter().all(|(k, _)| k != "secret_key"),
        "删除后 secret 不应出现在展示结果中"
    );
}

#[test]
fn get_provider_credentials_impl_reports_set_from_db() {
    let (_dir, conn) = open_tmp_db();
    let store = DbCredStore::new(&conn);
    let mut values = HashMap::new();
    values.insert("app_id".to_string(), "id1".to_string());
    values.insert("secret_key".to_string(), "sk1".to_string());
    set_provider_credentials_impl("baidu", values, &store, &conn).expect("保存应成功");

    let results = get_provider_credentials_impl("baidu", &conn).expect("读取应成功");

    let secret = results.iter().find(|r| r.key == "secret_key").unwrap();
    assert!(secret.is_set, "已配置 secret 应 is_set=true");
    assert!(secret.value.is_none(), "secret 的 value 永远应为 None");

    let app_id = results.iter().find(|r| r.key == "app_id").unwrap();
    assert!(app_id.is_set, "非密字段已存应 is_set=true");
    assert_eq!(app_id.value, Some("id1".to_string()), "非密字段返回实际值");
}

#[test]
fn load_for_display_unknown_provider_returns_empty_by_design() {
    // 展示路径对未知 provider 刻意返回空 Vec（而非 UnknownProvider 错误）：
    // 它由 credential_schema 驱动，未知 provider 的 schema 为空 → 自然无字段可展示。
    let (_dir, conn) = open_tmp_db();

    let display = load_credentials_for_display("nonexistent_provider", &conn)
        .expect("展示读取不应对未知 provider 报错");

    assert!(display.is_empty(), "未知 provider 展示结果应为空 Vec");
}
