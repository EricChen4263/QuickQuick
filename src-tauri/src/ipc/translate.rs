//! 翻译 IPC 命令层
//!
//! 模式：每个命令 = 薄的 `#[tauri::command]` 包装 + 可单测的纯函数 impl。
//! 单测只测 impl 函数（传 `&Connection` + `&dyn HttpExecutor`），命令层把错误映射为 `String`。
//!
//! 命令清单（前端通过 invoke 对应的命令名调用）：
//! - `translate_text`            — 翻译文本，写入历史，返回译文与方向
//! - `list_translate_history`    — 按时间倒序列出翻译历史（供 A08 历史栏回填）

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::db::DbError;
use crate::ipc::{with_db, AppDb};
use crate::translate::history::{
    add_translate_history, list_translate_history as db_list_translate_history, TranslateHistoryRow,
};
use crate::translate::lang::resolve_direction_with_source;
use crate::translate::providers::MyMemoryProvider;
use crate::translate::{
    Lang, ProviderHttpRequest, TranslateError, TranslateProvider, TranslateRequest,
};

/// 可注入 HTTP 执行器抽象。
///
/// 把真实网络调用抽象为 trait，使 impl 函数可在测试中注入 FakeExecutor，
/// 完全隔离网络，无需启动真实 HTTP 服务器。
pub trait HttpExecutor: Send + Sync {
    /// 按 provider 描述符发起 HTTP 请求，返回响应体原始字符串。
    ///
    /// # Errors
    /// 网络层失败（超时、连接拒绝、DNS 等）映射为 `TranslateError::Network`。
    fn execute(&self, req: &ProviderHttpRequest) -> Result<String, TranslateError>;
}

/// 基于 `ureq` 的同步 HTTP 执行器（生产用）。
///
/// 选用同步 ureq 而非 async reqwest，避免引入 Tokio 运行时与 `Mutex<Connection>`
/// 跨 await 点持有的复杂度（SQLCipher Connection 不是 Send，无法跨 await）。
pub struct UreqExecutor;

impl HttpExecutor for UreqExecutor {
    fn execute(&self, req: &ProviderHttpRequest) -> Result<String, TranslateError> {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(10))
            .build();

        let mut builder = match req.method {
            "GET" => agent.get(&req.url),
            "POST" => agent.post(&req.url),
            other => {
                return Err(TranslateError::Network(format!(
                    "不支持的 HTTP 方法: {other}"
                )))
            }
        };

        for (key, val) in &req.headers {
            builder = builder.set(key, val);
        }

        let response = if let Some(body) = &req.body {
            builder
                .send_string(body)
                .map_err(|e| TranslateError::Network(e.to_string()))?
        } else {
            builder
                .call()
                .map_err(|e| TranslateError::Network(e.to_string()))?
        };

        response
            .into_string()
            .map_err(|e| TranslateError::Network(e.to_string()))
    }
}

/// 测试用假执行器：返回构造时注入的预置响应串，并记录调用次数。
///
/// 使用 `Arc<AtomicU32>` 使 `call_count()` 可在测试断言中读取，
/// 即使 FakeExecutor 以引用传入也可共享计数。
pub struct FakeExecutor {
    raw_response: String,
    call_count: Arc<AtomicU32>,
}

impl FakeExecutor {
    /// 构造假执行器，`raw_response` 为预置的原始响应体字符串。
    pub fn new(raw_response: &str) -> Self {
        Self {
            raw_response: raw_response.to_string(),
            call_count: Arc::new(AtomicU32::new(0)),
        }
    }

    /// 返回执行器被调用的次数。
    pub fn call_count(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl HttpExecutor for FakeExecutor {
    fn execute(&self, _req: &ProviderHttpRequest) -> Result<String, TranslateError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.raw_response.clone())
    }
}

/// 翻译结果 DTO（返回给前端）。
///
/// 字段用 camelCase 序列化，与前端 TypeScript 接口对齐。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateResultDto {
    pub translated: String,
    pub source_lang: String,
    pub target_lang: String,
}

/// 翻译历史条目 DTO（供 A08 历史栏回填）。
///
/// 字段用 camelCase 序列化，与前端 TypeScript 接口对齐。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateHistoryDto {
    pub id: String,
    pub source_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub provider_id: String,
    pub created_utc: i64,
}

impl From<TranslateHistoryRow> for TranslateHistoryDto {
    fn from(row: TranslateHistoryRow) -> Self {
        Self {
            id: row.id,
            source_text: row.source_text,
            translated_text: row.translated_text,
            source_lang: row.source_lang,
            target_lang: row.target_lang,
            provider_id: row.provider_id,
            created_utc: row.created_utc,
        }
    }
}

/// `translate_text` 的纯函数实现，可在测试中直接调用。
///
/// 编排流程：
/// 1. 校验输入（空/全空白 text → Err，不发网络）
/// 2. 定方向（resolve_direction_with_source）：显式源语优先，否则自动检测
/// 3. 构造 TranslateRequest → 选默认 provider（mymemory）→ build_request → exec.execute
/// 4. parse_response → 得译文
/// 5. 写入翻译历史
/// 6. 返回 TranslateResultDto
///
/// # Errors
/// - text 为空或全空白：返回 Err（不触发执行器）
/// - 执行器网络失败：`TranslateError`
/// - 响应解析失败：`TranslateError`
/// - 历史写入失败：DbError 转为 String
pub fn translate_text_impl(
    conn: &Connection,
    exec: &dyn HttpExecutor,
    text: &str,
    configured_source: Option<&str>,
    configured_target: Option<&str>,
) -> Result<TranslateResultDto, String> {
    if text.trim().is_empty() {
        return Err("翻译文本不能为空或全空白".to_string());
    }

    let target_lang = configured_target.map(Lang::new);
    let (source, target) = resolve_direction_with_source(text, configured_source, target_lang);

    let req = TranslateRequest {
        text: text.to_string(),
        source_lang: source.clone(),
        target_lang: target.clone(),
    };

    let provider = MyMemoryProvider::new(None);
    let http_req = provider.build_request(&req);
    let raw = exec.execute(&http_req).map_err(|e| e.to_string())?;
    let resp = provider.parse_response(&raw).map_err(|e| e.to_string())?;

    add_translate_history(
        conn,
        text,
        &resp.translated,
        source.as_str(),
        target.as_str(),
        "mymemory",
    )
    .map_err(|e| e.to_string())?;

    Ok(TranslateResultDto {
        translated: resp.translated,
        source_lang: source.as_str().to_string(),
        target_lang: target.as_str().to_string(),
    })
}

/// `list_translate_history` 的纯函数实现，可在测试中直接调用。
///
/// 按 created_utc 倒序返回全部翻译历史记录。
///
/// # Errors
/// `DbError::Sqlite`：数据库查询失败
pub fn list_translate_history_impl(conn: &Connection) -> Result<Vec<TranslateHistoryDto>, DbError> {
    let rows = db_list_translate_history(conn)?;
    Ok(rows.into_iter().map(TranslateHistoryDto::from).collect())
}

/// Tauri 命令：翻译文本，返回译文与方向信息。
///
/// 前端通过 `invoke("translate_text", { text, source, target })` 调用。
/// `source` 为具体语言码（如 "ja"）时跳过检测；为 "auto"/null/省略 时回退自动检测。
#[tauri::command]
pub fn translate_text(
    state: State<'_, AppDb>,
    text: String,
    source: Option<String>,
    target: Option<String>,
) -> Result<TranslateResultDto, String> {
    with_db(&state, |conn| {
        translate_text_impl(
            conn,
            &UreqExecutor,
            &text,
            source.as_deref(),
            target.as_deref(),
        )
    })
}

/// Tauri 命令：按时间倒序列出翻译历史。
#[tauri::command]
pub fn list_translate_history(state: State<'_, AppDb>) -> Result<Vec<TranslateHistoryDto>, String> {
    with_db(&state, |conn| {
        list_translate_history_impl(conn).map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// 创建内存 SQLite 并初始化 translate_history 表，供测试隔离使用。
    ///
    /// schema 必须与 history.rs 中 add_translate_history 使用的列名完全一致。
    fn setup_test_db() -> Connection {
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

    /// MyMemory 成功响应的最小合法 JSON 模板（provider 能解析的格式）。
    fn mymemory_ok_response(translated: &str) -> String {
        format!(r#"{{"responseStatus":200,"responseData":{{"translatedText":"{translated}"}}}}"#)
    }

    #[test]
    fn translate_text_impl_with_none_source_uses_auto_detection() {
        // Arrange: source=None，让检测路径走；文本为英文，默认翻到 zh
        let conn = setup_test_db();
        let fake = FakeExecutor::new(&mymemory_ok_response("你好世界"));
        // Act
        let result = translate_text_impl(&conn, &fake, "hello world", None, None);
        // Assert: 无 source 显式值，检测 en，翻到 zh
        let dto = result.expect("应成功");
        assert_eq!(dto.source_lang, "en");
        assert_eq!(dto.target_lang, "zh");
        assert_eq!(fake.call_count(), 1);
    }

    #[test]
    fn translate_text_impl_explicit_source_and_target_reach_dto() {
        // Arrange: 注入 FakeExecutor，显式 source="ja" target="ko"
        let conn = setup_test_db();
        let fake = FakeExecutor::new(&mymemory_ok_response("안녕하세요"));
        // Act
        let result = translate_text_impl(&conn, &fake, "こんにちは", Some("ja"), Some("ko"));
        // Assert: source_lang/target_lang 在 DTO 中正确反映显式值
        let dto = result.expect("应成功");
        assert_eq!(dto.source_lang, "ja");
        assert_eq!(dto.target_lang, "ko");
    }

    #[test]
    fn translate_text_impl_empty_text_returns_error_without_calling_executor() {
        // Arrange: 空文本应提前 Err，不触发执行器
        let conn = setup_test_db();
        let fake = FakeExecutor::new("{}");
        // Act
        let result = translate_text_impl(&conn, &fake, "   ", None, None);
        // Assert
        assert!(result.is_err());
        assert_eq!(fake.call_count(), 0, "空文本不应调用执行器");
    }
}
