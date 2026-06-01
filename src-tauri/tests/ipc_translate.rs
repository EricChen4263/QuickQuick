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
    list_translate_history_impl, translate_text_impl, FakeExecutor,
};
use rusqlite::Connection;
use tempfile::tempdir;

const KEY: [u8; 32] = [7u8; 32];

/// 打开临时加密数据库，返回连接（schema 已初始化）
fn open_tmp_db() -> (tempfile::TempDir, Connection) {
    let dir = tempdir().expect("tempdir 创建失败");
    let path = dir.path().join("test_translate.db");
    let conn = db::open_or_create(&path, &KEY).expect("建库应成功");
    (dir, conn)
}

/// V4-F1-A02：中文输入时方向应为 zh→en，译文应等于执行器预置值
#[test]
fn ipc_translate_chinese_text_produces_zh_to_en_direction() {
    let (_dir, conn) = open_tmp_db();
    let fake =
        FakeExecutor::new(r#"{"responseData":{"translatedText":"Hello"},"responseStatus":200}"#);

    let result = translate_text_impl(&conn, &fake, "你好", None).expect("翻译应成功");

    assert_eq!(result.source_lang, "zh", "中文输入 sourceLang 应为 zh");
    assert_eq!(result.target_lang, "en", "中文输入 targetLang 应为 en");
    assert_eq!(result.translated, "Hello", "译文应等于 FakeExecutor 预置值");
}

/// V4-F1-A02：非中文（英文）输入时方向应为 en→zh
#[test]
fn ipc_translate_english_text_produces_en_to_zh_direction() {
    let (_dir, conn) = open_tmp_db();
    let fake =
        FakeExecutor::new(r#"{"responseData":{"translatedText":"你好"},"responseStatus":200}"#);

    let result = translate_text_impl(&conn, &fake, "Hello", None).expect("翻译应成功");

    assert_eq!(result.source_lang, "en", "英文输入 sourceLang 应为 en");
    assert_eq!(result.target_lang, "zh", "英文输入 targetLang 应为 zh");
    assert_eq!(result.translated, "你好", "译文应等于 FakeExecutor 预置值");
}

/// V4-F1-A02：翻译成功后历史应被写入（translate_history_count +1）
#[test]
fn ipc_translate_writes_to_history_after_success() {
    let (_dir, conn) = open_tmp_db();
    let fake =
        FakeExecutor::new(r#"{"responseData":{"translatedText":"World"},"responseStatus":200}"#);

    let count_before =
        quickquick_lib::translate::history::translate_history_count(&conn).expect("count 应成功");

    translate_text_impl(&conn, &fake, "世界", None).expect("翻译应成功");

    let count_after =
        quickquick_lib::translate::history::translate_history_count(&conn).expect("count 应成功");

    assert_eq!(count_after, count_before + 1, "翻译后历史条目数应 +1");
}

/// V4-F1-A02：空文本输入应返回 Err，且不触发执行器
#[test]
fn ipc_translate_empty_text_returns_error_without_calling_executor() {
    let (_dir, conn) = open_tmp_db();
    let fake = FakeExecutor::new("should not be called");

    let result = translate_text_impl(&conn, &fake, "", None);

    assert!(result.is_err(), "空文本应返回 Err");
    assert_eq!(fake.call_count(), 0, "空文本不应触发执行器");
}

/// V4-F1-A02：全空白文本输入应返回 Err，且不触发执行器
#[test]
fn ipc_translate_whitespace_text_returns_error_without_calling_executor() {
    let (_dir, conn) = open_tmp_db();
    let fake = FakeExecutor::new("should not be called");

    let result = translate_text_impl(&conn, &fake, "   \t\n", None);

    assert!(result.is_err(), "全空白文本应返回 Err");
    assert_eq!(fake.call_count(), 0, "全空白文本不应触发执行器");
}

/// V4-F1-A02：list_translate_history_impl 返回含已翻译条目（按时间倒序）
#[test]
fn ipc_translate_list_history_returns_entries_in_desc_order() {
    let (_dir, conn) = open_tmp_db();
    let fake1 =
        FakeExecutor::new(r#"{"responseData":{"translatedText":"Hello"},"responseStatus":200}"#);
    let fake2 =
        FakeExecutor::new(r#"{"responseData":{"translatedText":"World"},"responseStatus":200}"#);

    translate_text_impl(&conn, &fake1, "你好", None).expect("第一次翻译应成功");
    translate_text_impl(&conn, &fake2, "世界", None).expect("第二次翻译应成功");

    let history = list_translate_history_impl(&conn).expect("list 应成功");

    assert_eq!(history.len(), 2, "应返回 2 条历史");
    assert_eq!(history[0].source_text, "世界", "倒序：最后翻译的应排第一");
    assert_eq!(history[1].source_text, "你好", "倒序：最先翻译的应排第二");
}
