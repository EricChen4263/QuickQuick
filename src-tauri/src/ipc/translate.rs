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
use crate::translate::lang::resolve_direction;
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
/// 2. 本地检测方向（resolve_direction）
/// 3. 构造 TranslateRequest → 选默认 provider（mymemory）→ build_request → exec.execute
/// 4. parse_response → 得译文
/// 5. 写入翻译历史
/// 6. 返回 TranslateResultDto
///
/// # Errors
/// - text 为空或全空白：返回 Err（不触发执行器）
/// - 执行器网络失败：`TranslateError`
/// - 响应解析失败：`TranslateError`
/// - 历史写入失败：DbError 转为 String（历史写入失败不中断译文返回的语义待后续评估；
///   此处选择失败即报错，保持一致性）
pub fn translate_text_impl(
    conn: &Connection,
    exec: &dyn HttpExecutor,
    text: &str,
    configured_target: Option<&str>,
) -> Result<TranslateResultDto, String> {
    if text.trim().is_empty() {
        return Err("翻译文本不能为空或全空白".to_string());
    }

    let target_lang = configured_target.map(Lang::new);
    let (source, target) = resolve_direction(text, target_lang);

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
#[tauri::command]
pub fn translate_text(
    state: State<'_, AppDb>,
    text: String,
    target: Option<String>,
) -> Result<TranslateResultDto, String> {
    with_db(&state, |conn| {
        translate_text_impl(conn, &UreqExecutor, &text, target.as_deref())
    })
}

/// Tauri 命令：按时间倒序列出翻译历史。
#[tauri::command]
pub fn list_translate_history(state: State<'_, AppDb>) -> Result<Vec<TranslateHistoryDto>, String> {
    with_db(&state, |conn| {
        list_translate_history_impl(conn).map_err(|e| e.to_string())
    })
}
