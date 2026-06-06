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
use tauri::{AppHandle, Emitter, State};

use std::path::Path;

use crate::db::DbError;
use crate::ipc::settings::{get_selected_provider_impl, resolve_config_path};
use crate::ipc::{with_db, AppDb, EcdictDbState};
use crate::translate::credential::{load_credentials, CredStore, DbCredStore};
use crate::translate::ecdict_db::EcdictDb;
use crate::translate::history::{
    add_translate_history, list_translate_history as db_list_translate_history, TranslateHistoryRow,
};
use crate::translate::lang::resolve_direction_with_source;
use crate::translate::providers::{build_provider, registry};
use crate::translate::{
    DictEntry, HttpExecutor, Lang, ProviderHttpRequest, TranslateError, TranslateRequest,
    TranslateResponse,
};

/// 默认翻译源 id（免 key，开箱可用）。
///
/// 设计文档§三.决策1：免 key 开箱可用的机翻源，无第三方实例可用性依赖。
pub const DEFAULT_PROVIDER_ID: &str = "google_free";

/// 把持久化的 selected_provider 解析为当前注册表内的有效 id。
///
/// 以「是否在注册表」为判定（而非硬编码判 mymemory），将来删任何源都安全：
/// 若 `stored` 仍在 `registry()` 的 id 集合内则原样返回；否则（含旧值 mymemory、
/// 任意未知 id）回退到 `DEFAULT_PROVIDER_ID`。抽为纯函数以便单测，
/// 调用方负责在写路径持久化修正后的值。
pub fn resolve_provider_or_fallback(stored: &str) -> String {
    let is_known = registry().iter().any(|cap| cap.id == stored);
    if is_known {
        stored.to_string()
    } else {
        DEFAULT_PROVIDER_ID.to_string()
    }
}

/// 翻译历史变化事件名。与前端 src/ipc/events.ts 的 TRANSLATE_HISTORY_CHANGED_EVENT 必须一致。
/// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
const TRANSLATE_HISTORY_CHANGED_EVENT: &str = "translate-history-changed";

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
/// `kind` 为类型判别标签（`"plain"`/`"dict"`），与后端 `TranslateResponse`
/// 的 serde tag 取值一致——前端据此分别渲染普通译文 / 结构化词条。
/// Plain 结果 `translated` 为译文、`entry` 为 None；Dict 结果 `entry` 为词条、
/// `translated` 为词条的纯文本摘要（供历史栏与不支持词典渲染处回退展示）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateResultDto {
    pub kind: String,
    pub translated: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<DictEntry>,
    pub source_lang: String,
    pub target_lang: String,
}

/// 把词典词条压成单行纯文本摘要，供历史记录与回退展示。
///
/// 取首个有释义的词性的释义拼接；无释义则回退音标或空串。
fn dict_entry_summary(entry: &DictEntry) -> String {
    let meanings: Vec<&str> = entry
        .definitions
        .iter()
        .flat_map(|d| d.meanings.iter())
        .map(String::as_str)
        .collect();
    if !meanings.is_empty() {
        return meanings.join("; ");
    }
    entry.phonetic.clone().unwrap_or_default()
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

/// 翻译请求的用户输入三元组：原文 + 用户配置的源/目标语言。
///
/// 把同源同生命周期的三个请求参数聚为一组，使 `translate_text_impl` 参数数量
/// 收敛在 clippy 上限内（设计同 code-standards「参数 > 5 改结构体」）。
/// `configured_source`/`configured_target` 为 `None` 时分别走自动检测 / 回退默认目标。
pub struct TranslateInput<'a> {
    /// 待翻译原文。
    pub text: &'a str,
    /// 用户显式指定的源语言码（如 "ja"）；`None`/"auto" 走自动检测。
    pub configured_source: Option<&'a str>,
    /// 用户配置的目标语言码；`None` 走方向解析的默认目标。
    pub configured_target: Option<&'a str>,
}

/// `translate_text` 的纯函数实现，可在测试中直接调用。
///
/// 编排流程：
/// 1. 校验输入（空/全空白 text → Err，不发网络）
/// 2. 读 settings_path 取 selected_provider_id
/// 3. load_credentials(provider_id, cred_store, conn) 加载凭据
/// 4. build_provider(provider_id, &creds, ecdict_db) 动态构造 provider（缺必填凭据 → Err）
/// 5. 定方向（resolve_direction_with_source）
/// 6. provider.translate(req, exec)（单步源默认 build/execute/parse；多步源自行编排）
/// 7. 写入翻译历史（provider_id 为真实选中的 id）
/// 8. 返回 TranslateResultDto
///
/// # Errors
/// - text 为空或全空白：返回 Err（不触发执行器）
/// - 必填凭据缺失：返回含"未配置"的中文 Err（不触发执行器）
/// - 执行器网络失败：`TranslateError` 转字符串
/// - 响应解析失败：`TranslateError` 转字符串
/// - 历史写入失败：DbError 转字符串
pub fn translate_text_impl(
    conn: &Connection,
    exec: &dyn HttpExecutor,
    input: TranslateInput<'_>,
    settings_path: &Path,
    cred_store: &dyn CredStore,
    ecdict_db: Arc<EcdictDb>,
) -> Result<TranslateResultDto, String> {
    let text = input.text;
    if text.trim().is_empty() {
        return Err("翻译文本不能为空或全空白".to_string());
    }

    let provider_id = get_selected_provider_impl(settings_path)?;
    let creds = load_credentials(&provider_id, cred_store, conn).map_err(|e| e.to_string())?;
    let provider = build_provider(&provider_id, &creds, Some(ecdict_db))?;

    let target_lang = input.configured_target.map(Lang::new);
    let (source, target) =
        resolve_direction_with_source(text, input.configured_source, target_lang);

    let req = TranslateRequest {
        text: text.to_string(),
        source_lang: source.clone(),
        target_lang: target.clone(),
    };

    // 改调 provider.translate（默认实现 = 单步 build/execute/parse；多步源如 Bing override）。
    let resp = provider.translate(&req, exec).map_err(|e| e.to_string())?;

    // 拆出类型判别标签 + 历史用纯文本：Plain 直接取译文，Dict 压成摘要回退。
    let (kind, translated, entry) = match resp {
        TranslateResponse::Plain { translated } => ("plain", translated, None),
        TranslateResponse::Dict { entry } => ("dict", dict_entry_summary(&entry), Some(entry)),
    };

    add_translate_history(
        conn,
        text,
        &translated,
        source.as_str(),
        target.as_str(),
        &provider_id,
    )
    .map_err(|e| e.to_string())?;

    Ok(TranslateResultDto {
        kind: kind.to_string(),
        translated,
        entry,
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
/// 翻译成功后 emit `TRANSLATE_HISTORY_CHANGED_EVENT`，通知前端历史栏刷新；
/// emit 失败仅 eprintln 记录，不影响翻译结果返回。
#[tauri::command]
pub fn translate_text(
    app: AppHandle,
    state: State<'_, AppDb>,
    ecdict: State<'_, EcdictDbState>,
    text: String,
    source: Option<String>,
    target: Option<String>,
) -> Result<TranslateResultDto, String> {
    let settings_path = resolve_config_path(&app, "settings.json")?;
    let ecdict_db = Arc::clone(&ecdict.0);
    // secret 存进同一加密 DB 的 provider_secret 表（去 Keychain），故在 with_db 闭包内构造 DbCredStore。
    let result = with_db(&state, |conn| {
        let cred_store = DbCredStore::new(conn);
        translate_text_impl(
            conn,
            &UreqExecutor,
            TranslateInput {
                text: &text,
                configured_source: source.as_deref(),
                configured_target: target.as_deref(),
            },
            &settings_path,
            &cred_store,
            Arc::clone(&ecdict_db),
        )
    });
    if result.is_ok() {
        if let Err(e) = app.emit(TRANSLATE_HISTORY_CHANGED_EVENT, ()) {
            eprintln!("[QuickQuick] 发送 {TRANSLATE_HISTORY_CHANGED_EVENT} 事件失败: {e}");
        }
    }
    result
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

    use crate::translate::credential::MockCredStore;

    /// 测试用占位本地词典 DAO：路径不存在，仅供非 ecdict 源的调用传参（永不被查询）。
    fn placeholder_ecdict_db() -> Arc<EcdictDb> {
        Arc::new(EcdictDb::new(std::path::PathBuf::from(
            "/nonexistent/ecdict.db",
        )))
    }

    // 对齐 acceptance TV1-F1-A03：持久化的 selected_provider 不在注册表内
    // （含旧值 mymemory、已下架 id、任意未知 id）时，解析回退为默认源 google_free；
    // 注册表内的合法 id 原样保留。
    #[test]
    fn selected_provider_migrates_unknown_to_default() {
        // 旧值 mymemory（已删除）→ 回退默认源 google_free
        assert_eq!(resolve_provider_or_fallback("mymemory"), "google_free");
        // 任意未知 id → 回退 google_free
        assert_eq!(
            resolve_provider_or_fallback("totally_unknown"),
            "google_free"
        );
        // 已下架的 bing_dict / cambridge（Commit 1）→ 回退默认源
        assert_eq!(resolve_provider_or_fallback("bing_dict"), "google_free");
        assert_eq!(resolve_provider_or_fallback("cambridge"), "google_free");
        // 空串 → 回退 google_free
        assert_eq!(resolve_provider_or_fallback(""), "google_free");
        // 注册表内合法 id 原样保留（不被误改）
        assert_eq!(resolve_provider_or_fallback("lingva"), "lingva");
        assert_eq!(resolve_provider_or_fallback("baidu"), "baidu");
        assert_eq!(resolve_provider_or_fallback("google"), "google");
    }

    /// 创建内存 SQLite 并初始化所需表，供测试隔离使用。
    ///
    /// 包含 translate_history 和 provider_config，
    /// schema 必须与各自对应模块中实际 SQL 一致。
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
            );
            CREATE TABLE provider_config (
                provider_id  TEXT NOT NULL,
                field_key    TEXT NOT NULL,
                value        TEXT NOT NULL,
                PRIMARY KEY (provider_id, field_key)
            );",
        )
        .expect("建表失败");
        conn
    }

    /// 写入临时 settings.json，包含指定的 selected_provider。
    fn write_settings_with_provider(path: &std::path::Path, provider_id: &str) {
        let content = format!(r#"{{"selected_provider":"{provider_id}"}}"#);
        std::fs::write(path, content).expect("写 settings 失败");
    }

    /// Lingva 成功响应的最小合法 JSON 模板（provider 能解析的格式）。
    fn lingva_ok_response(translated: &str) -> String {
        format!(r#"{{"translation":"{translated}"}}"#)
    }

    #[test]
    fn translate_text_impl_with_none_source_uses_auto_detection() {
        // Arrange
        let conn = setup_test_db();
        let fake = FakeExecutor::new(&lingva_ok_response("你好世界"));
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "lingva");
        // Act
        let result = translate_text_impl(
            &conn,
            &fake,
            TranslateInput {
                text: "hello world",
                configured_source: None,
                configured_target: None,
            },
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        );
        // Assert: 无 source 显式值，检测 en，翻到 zh
        let dto = result.expect("应成功");
        assert_eq!(dto.source_lang, "en");
        assert_eq!(dto.target_lang, "zh");
        assert_eq!(fake.call_count(), 1);
    }

    #[test]
    fn translate_text_impl_explicit_source_and_target_reach_dto() {
        // Arrange
        let conn = setup_test_db();
        let fake = FakeExecutor::new(&lingva_ok_response("안녕하세요"));
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "lingva");
        // Act
        let result = translate_text_impl(
            &conn,
            &fake,
            TranslateInput {
                text: "こんにちは",
                configured_source: Some("ja"),
                configured_target: Some("ko"),
            },
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        );
        // Assert
        let dto = result.expect("应成功");
        assert_eq!(dto.source_lang, "ja");
        assert_eq!(dto.target_lang, "ko");
    }

    #[test]
    fn translate_text_impl_empty_text_returns_error_without_calling_executor() {
        // Arrange
        let conn = setup_test_db();
        let fake = FakeExecutor::new("{}");
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "lingva");
        // Act
        let result = translate_text_impl(
            &conn,
            &fake,
            TranslateInput {
                text: "   ",
                configured_source: None,
                configured_target: None,
            },
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        );
        // Assert
        assert!(result.is_err());
        assert_eq!(fake.call_count(), 0, "空文本不应调用执行器");
    }

    #[test]
    fn translate_text_impl_selected_lingva_writes_provider_id_in_history() {
        // Arrange
        let conn = setup_test_db();
        let fake = FakeExecutor::new(&lingva_ok_response("世界你好"));
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "lingva");
        // Act
        translate_text_impl(
            &conn,
            &fake,
            TranslateInput {
                text: "hello world",
                configured_source: None,
                configured_target: None,
            },
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("应成功");
        // Assert：历史记录中 provider_id 应为 "lingva"，不应是硬编码字符串
        let rows = crate::translate::history::list_translate_history(&conn).expect("查历史失败");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].provider_id, "lingva");
    }

    /// DeepL Free 成功响应的最小合法 JSON（provider 能解析的格式）。
    fn deepl_ok_response(translated: &str) -> String {
        format!(r#"{{"translations":[{{"text":"{translated}","detected_source_language":"ZH"}}]}}"#)
    }

    /// 动态路由不变量（非 mymemory 场景）：
    /// selected_provider="deepl_free" 时，写入历史的 provider_id 应为 "deepl_free"，
    /// 而非硬编码的 "mymemory"——守护动态路由写历史的核心不变量。
    #[test]
    fn translate_text_impl_selected_deepl_free_writes_deepl_provider_id_in_history() {
        // Arrange
        let conn = setup_test_db();
        let fake = FakeExecutor::new(&deepl_ok_response("Good"));
        let store = MockCredStore::new();
        store
            .set_secret("deepl_free", "auth_key", "test-deepl-auth-key-xxx")
            .expect("预置 deepl_free auth_key 应成功");
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "deepl_free");

        // Act
        translate_text_impl(
            &conn,
            &fake,
            TranslateInput {
                text: "你好",
                configured_source: None,
                configured_target: None,
            },
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("deepl_free 翻译应成功");

        // Assert：历史 provider_id 应为 "deepl_free"，不能是硬编码 "mymemory"
        let rows: Vec<String> = conn
            .prepare("SELECT provider_id FROM translate_history")
            .expect("prepare 应成功")
            .query_map([], |row| row.get(0))
            .expect("query_map 应成功")
            .map(|r| r.expect("row 应可读"))
            .collect();
        assert_eq!(rows.len(), 1, "应有 1 条历史记录");
        assert_eq!(
            rows[0], "deepl_free",
            "历史 provider_id 应为 \"deepl_free\"，实际: {}",
            rows[0]
        );
    }

    #[test]
    fn translate_text_impl_selected_baidu_without_creds_returns_err_with_hint() {
        // Arrange：settings 选 baidu，但 store 里没有凭据
        let conn = setup_test_db();
        let fake = FakeExecutor::new("{}");
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "baidu");
        // Act
        let result = translate_text_impl(
            &conn,
            &fake,
            TranslateInput {
                text: "hello world",
                configured_source: None,
                configured_target: None,
            },
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        );
        // Assert：应返回含"未配置"的错误，执行器不应被调用
        assert!(result.is_err(), "百度无凭据应返回 Err");
        let err = result.unwrap_err();
        assert!(err.contains("未配置"), "错误消息应提示未配置凭据：{err}");
        assert_eq!(fake.call_count(), 0, "凭据缺失时执行器不应被调用");
    }
}
