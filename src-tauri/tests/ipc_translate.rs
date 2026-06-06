//! 集成测试：翻译 IPC 命令实现层（impl 函数）
//!
//! 覆盖验收项 V4-F1-A02：翻译 IPC 命令
//! - 中文输入 → 方向 zh→en，译文来自 FakeExecutor 预置值
//! - 英文/非中文输入 → 方向 →zh
//! - 翻译后历史被写入（translate_history_count +1）
//! - 空文本 → Err，且执行器调用次数为 0
//!
//! 测试约定：函数名含子串 `ipc_translate` 确保 verify 命中。

use quickquick_lib::db;
use quickquick_lib::ipc::translate::{
    list_translate_history_impl, translate_text_impl, FakeExecutor, TranslateInput,
};
use quickquick_lib::translate::credential::{CredError, CredStore};
use quickquick_lib::translate::ecdict_db::EcdictDb;
use rusqlite::Connection;
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

/// 占位 ECDICT DAO：路径不存在，供非 ecdict 源的调用传参（永不被查询）。
fn placeholder_ecdict_db() -> Arc<EcdictDb> {
    Arc::new(EcdictDb::new(std::path::PathBuf::from(
        "/nonexistent/ecdict.db",
    )))
}

/// 构造仅含原文、源/目标语言走默认的 `TranslateInput`（自动检测方向）。
fn auto_input(text: &str) -> TranslateInput<'_> {
    TranslateInput {
        text,
        configured_source: None,
        configured_target: None,
    }
}

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

const KEY: [u8; 32] = [7u8; 32];

/// 打开临时加密数据库，返回连接（schema 已初始化）
fn open_tmp_db() -> (tempfile::TempDir, Connection) {
    let dir = tempdir().expect("tempdir 创建失败");
    let path = dir.path().join("test_translate.db");
    let conn = db::open_or_create(&path, &KEY).expect("建库应成功");
    (dir, conn)
}

/// 在 dir 下写入 settings.json，包含指定的 selected_provider。
fn write_settings(dir: &tempfile::TempDir, provider_id: &str) -> std::path::PathBuf {
    let path = dir.path().join("settings.json");
    let mut f = std::fs::File::create(&path).expect("创建 settings.json 失败");
    write!(f, r#"{{"selected_provider":"{provider_id}"}}"#).expect("写入失败");
    path
}

/// V4-F1-A02：中文输入时方向应为 zh→en，译文应等于执行器预置值
#[test]
fn ipc_translate_chinese_text_produces_zh_to_en_direction() {
    let (_dir, conn) = open_tmp_db();
    let settings_path = write_settings(&_dir, "lingva");
    let store = LocalMockCredStore::new();
    let fake = FakeExecutor::new(r#"{"translation":"Hello"}"#);

    let result = translate_text_impl(
        &conn,
        &fake,
        auto_input("你好"),
        &settings_path,
        &store,
        placeholder_ecdict_db(),
    )
    .expect("翻译应成功");

    assert_eq!(result.source_lang, "zh", "中文输入 sourceLang 应为 zh");
    assert_eq!(result.target_lang, "en", "中文输入 targetLang 应为 en");
    assert_eq!(result.translated, "Hello", "译文应等于 FakeExecutor 预置值");
}

/// V4-F1-A02：非中文（英文）输入时方向应为 en→zh
#[test]
fn ipc_translate_english_text_produces_en_to_zh_direction() {
    let (_dir, conn) = open_tmp_db();
    let settings_path = write_settings(&_dir, "lingva");
    let store = LocalMockCredStore::new();
    let fake = FakeExecutor::new(r#"{"translation":"你好"}"#);

    let result = translate_text_impl(
        &conn,
        &fake,
        auto_input("Hello"),
        &settings_path,
        &store,
        placeholder_ecdict_db(),
    )
    .expect("翻译应成功");

    assert_eq!(result.source_lang, "en", "英文输入 sourceLang 应为 en");
    assert_eq!(result.target_lang, "zh", "英文输入 targetLang 应为 zh");
    assert_eq!(result.translated, "你好", "译文应等于 FakeExecutor 预置值");
}

/// V4-F1-A02：翻译成功后历史应被写入（translate_history_count +1）
#[test]
fn ipc_translate_writes_to_history_after_success() {
    let (_dir, conn) = open_tmp_db();
    let settings_path = write_settings(&_dir, "lingva");
    let store = LocalMockCredStore::new();
    let fake = FakeExecutor::new(r#"{"translation":"World"}"#);

    let count_before =
        quickquick_lib::translate::history::translate_history_count(&conn).expect("count 应成功");

    translate_text_impl(
        &conn,
        &fake,
        auto_input("世界"),
        &settings_path,
        &store,
        placeholder_ecdict_db(),
    )
    .expect("翻译应成功");

    let count_after =
        quickquick_lib::translate::history::translate_history_count(&conn).expect("count 应成功");

    assert_eq!(count_after, count_before + 1, "翻译后历史条目数应 +1");
}

/// V4-F1-A02：空文本输入应返回 Err，且不触发执行器
#[test]
fn ipc_translate_empty_text_returns_error_without_calling_executor() {
    let (_dir, conn) = open_tmp_db();
    let settings_path = write_settings(&_dir, "lingva");
    let store = LocalMockCredStore::new();
    let fake = FakeExecutor::new("should not be called");

    let result = translate_text_impl(
        &conn,
        &fake,
        auto_input(""),
        &settings_path,
        &store,
        placeholder_ecdict_db(),
    );

    assert!(result.is_err(), "空文本应返回 Err");
    assert_eq!(fake.call_count(), 0, "空文本不应触发执行器");
}

/// V4-F1-A02：全空白文本输入应返回 Err，且不触发执行器
#[test]
fn ipc_translate_whitespace_text_returns_error_without_calling_executor() {
    let (_dir, conn) = open_tmp_db();
    let settings_path = write_settings(&_dir, "lingva");
    let store = LocalMockCredStore::new();
    let fake = FakeExecutor::new("should not be called");

    let result = translate_text_impl(
        &conn,
        &fake,
        auto_input("   \t\n"),
        &settings_path,
        &store,
        placeholder_ecdict_db(),
    );

    assert!(result.is_err(), "全空白文本应返回 Err");
    assert_eq!(fake.call_count(), 0, "全空白文本不应触发执行器");
}

/// V4-F1-A02：list_translate_history_impl 返回含已翻译条目（按时间倒序）
#[test]
fn ipc_translate_list_history_returns_entries_in_desc_order() {
    let (_dir, conn) = open_tmp_db();
    let settings_path = write_settings(&_dir, "lingva");
    let store = LocalMockCredStore::new();
    let fake1 = FakeExecutor::new(r#"{"translation":"Hello"}"#);
    let fake2 = FakeExecutor::new(r#"{"translation":"World"}"#);

    translate_text_impl(
        &conn,
        &fake1,
        auto_input("你好"),
        &settings_path,
        &store,
        placeholder_ecdict_db(),
    )
    .expect("第一次翻译应成功");
    translate_text_impl(
        &conn,
        &fake2,
        auto_input("世界"),
        &settings_path,
        &store,
        placeholder_ecdict_db(),
    )
    .expect("第二次翻译应成功");

    let history = list_translate_history_impl(&conn).expect("list 应成功");

    assert_eq!(history.len(), 2, "应返回 2 条历史");
    assert_eq!(history[0].source_text, "世界", "倒序：最后翻译的应排第一");
    assert_eq!(history[1].source_text, "你好", "倒序：最先翻译的应排第二");
}
