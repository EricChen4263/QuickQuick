//! 翻译 IPC 命令层
//!
//! 模式：每个命令 = 薄的 `#[tauri::command]` 包装 + 可单测的纯函数 impl。
//! 单测只测 impl 函数（传 `&Connection` + `&dyn HttpExecutor`），命令层把错误映射为 `String`。
//!
//! 命令清单（前端通过 invoke 对应的命令名调用）：
//! - `translate_text`            — 翻译文本，写入历史，返回译文与方向
//! - `list_translate_history`    — 按时间倒序返回最近一页翻译历史（供 A08 历史栏回填）

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use std::path::Path;
use std::time::Duration;

use crate::db::DbError;
use crate::ipc::settings::{get_selected_provider_impl, resolve_config_path};
use crate::ipc::{with_db, AppDb, EcdictDbState};
use crate::translate::cache::{cache_get, cache_key, cache_put};
use crate::translate::cancel::InflightTracker;
use crate::translate::credential::{load_credentials, CredStore, DbCredStore};
use crate::translate::ecdict_db::EcdictDb;
use crate::translate::error::{classify_timeout, map_provider_error};
use crate::translate::history::{
    add_translate_history, list_translate_history as db_list_translate_history, TranslateHistoryRow,
};
use crate::translate::lang::resolve_direction_with_source;
use crate::translate::providers::{build_provider, registry};
use crate::translate::retry::retry_with_backoff;
use crate::translate::{
    DictEntry, HttpExecutor, Lang, ProviderHttpRequest, TranslateError, TranslateRequest,
    TranslateResponse,
};

/// 默认翻译源 id（免 key，开箱可用）。
///
/// 设计文档§三.决策1：免 key 开箱可用的机翻源，无第三方实例可用性依赖。
pub const DEFAULT_PROVIDER_ID: &str = "google_free";

/// 翻译缓存容量上限（条目数），超出后按 `last_used_utc` 做 LRU 淘汰。
///
/// 取 500：单条缓存只存一段纯文本译文，500 条在加密库中的体量可忽略；
/// 同时足以覆盖「反复翻译同一批短文本」与「免费源被限流后复用既有结果」两类真实场景。
const TRANSLATE_CACHE_CAPACITY: usize = 500;

/// 单次翻译的最大尝试次数（含首次请求）。
///
/// 取 3：免费源偶发抖动一次重试即可恢复；再多会把可感知延迟推到不可接受
/// （退避序列 500ms、1000ms，两次重试合计最多 1.5s）。
const TRANSLATE_MAX_ATTEMPTS: u32 = 3;

/// 单次 HTTP 请求的整体超时上限。
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

/// 请求被更新的请求取代时返回给前端的错误文案。
const TRANSLATE_CANCELLED_MESSAGE: &str = "翻译请求已被更新的请求取代";

/// 生产环境的退避等待：真实线程休眠。
///
/// 这条休眠落在**既有阻塞缺陷之内**，不是新增的界面影响面：同步 `#[tauri::command]`
/// 由 tauri-macros 生成为 `body_blocking`，命令体在回调线程上直接执行（没有
/// `spawn_blocking`，也无运行时线程跳转）；而该回调是 wry 的
/// `WKScriptMessageHandler::userContentController:didReceiveScriptMessage:`，
/// 该协议在 `objc2-web-kit` 中声明为 `MainThreadOnly`。
/// 也就是说 `translate_text` 与既有的同步网络 I/O 一样运行在 macOS **主线程**上，
/// 同样会冻结界面；本函数是在这个既有缺陷之上再叠加最多 1.5s 的阻塞
/// （3 次尝试的退避 500ms + 1000ms），叠加在既有的 10s 网络超时之上。
///
/// 根治方向（超出本次改动范围，需动 `lib.rs` 的命令注册装配，已登记 follow-up）：
/// 把 `translate_text` 改为 `async` 命令，并把同步网络 I/O 与退避一并挪进
/// `spawn_blocking`，让主线程只负责派发。
fn sleep_blocking(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// 翻译运行期的横切配置：重试策略 + 在途请求追踪器。
///
/// 把两者聚为一组，使 `translate_text_with_runtime` 的参数数量收敛在 clippy 上限内，
/// 同时保留两项测试注入能力：只记录不退避的 `sleep`（断言退避序列）与独立的 `tracker`
/// （断言旧 generation 的结果被丢弃）。生产环境两者都取真实实现，退避会真实阻塞主线程。
pub struct TranslateRuntime<'a> {
    /// 单次翻译允许的最大尝试次数（含首次）。
    pub max_attempts: u32,
    /// 退避等待函数，入参为毫秒。
    pub sleep: &'a dyn Fn(u64),
    /// 在途请求追踪器，用于作废旧请求的结果。
    pub tracker: &'a InflightTracker,
}

impl TranslateRuntime<'static> {
    /// 生产运行期配置：3 次尝试 + 真实退避休眠 + 进程内在途追踪器单例。
    pub fn production() -> Self {
        Self {
            max_attempts: TRANSLATE_MAX_ATTEMPTS,
            sleep: &sleep_blocking,
            tracker: crate::translate::inflight_tracker(),
        }
    }
}

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

/// 把 ureq 错误归一为统一 `TranslateError`。
///
/// - 非 2xx 状态码：经 `map_provider_error` 归一为 Auth（401/403）/ RateLimit（429）/
///   ServerError（5xx）等具体变体，而不是笼统的 Network；
/// - 传输层失败：超时经 `classify_timeout` 归一为 Network，其余传输失败同为 Network。
///
/// 归一之后上层的 `is_transient` 判定才真正有意义：Auth / Quota 类永久错误不会被
/// 当成瞬时抖动反复重试，而 429 / 5xx / 超时才会进入退避重试。
fn normalize_ureq_error(err: ureq::Error) -> TranslateError {
    match err {
        ureq::Error::Status(status, _response) => map_provider_error(status, None),
        transport_error @ ureq::Error::Transport(_) => {
            if is_timeout(&transport_error) {
                classify_timeout()
            } else {
                TranslateError::Network(transport_error.to_string())
            }
        }
    }
}

/// 判断 ureq 错误是否由请求超时引起。
///
/// ureq 把超时统一表现为 `ErrorKind::Io`，真正原因藏在 `source()` 链上的
/// `io::Error`（`ErrorKind::TimedOut`），因此必须沿 source 链下探，
/// 而不能只看错误文本或 ureq 的 `kind()`。
///
/// 这条链路依赖 ureq 2.x 用 `impl From<io::Error> for Error` 把 `io::Error` 挂成
/// `Error::Transport` 的 `source`；升级 ureq 大版本时必须重新核对（对应单测会先失败）。
fn is_timeout(err: &ureq::Error) -> bool {
    let mut source = std::error::Error::source(err);
    while let Some(inner) = source {
        if let Some(io_err) = inner.downcast_ref::<std::io::Error>() {
            return io_err.kind() == std::io::ErrorKind::TimedOut;
        }
        source = inner.source();
    }
    false
}

impl HttpExecutor for UreqExecutor {
    fn execute(&self, req: &ProviderHttpRequest) -> Result<String, TranslateError> {
        let agent = ureq::AgentBuilder::new().timeout(HTTP_TIMEOUT).build();

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
            builder.send_string(body).map_err(normalize_ureq_error)?
        } else {
            builder.call().map_err(normalize_ureq_error)?
        };

        response
            .into_string()
            .map_err(|e| TranslateError::Network(format!("读取响应体失败: {e}")))
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
/// 3. 定方向（`resolve_direction_with_source`）——必须在缓存查询与 provider 构造之前，
///    因为缓存键要求**最终**的 source/target，而不是检测前的猜测语言
/// 4. 按 `(text, source_lang, target_lang, provider_id)` 四元组查缓存，命中直接返回（零网络）
/// 5. 未命中：`load_credentials` + `build_provider` 构造 provider（缺必填凭据 → Err）
/// 6. `provider.translate(req, exec)`（单步源默认 build/execute/parse；多步源自行编排），
///    瞬时错误按退避序列重试，永久错误立即返回
/// 7. Plain 结果写缓存（Dict 不写，理由见 `translate_text_with_runtime`）
/// 8. 写入翻译历史（命中缓存与未命中路径一致，provider_id 为真实选中的 id）
/// 9. 返回 TranslateResultDto
///
/// 取消：本函数只承担「一次翻译」的语义，使用本次调用独立新建的 tracker，
/// 不假设不同调用之间存在先后关系。跨请求的「旧结果作废」由命令层
/// `translate_text` 注入进程内单例 tracker 实现（见 `TranslateRuntime::production`）。
///
/// # Errors
/// - text 为空或全空白：返回 Err（不触发执行器）
/// - 必填凭据缺失：返回含"未配置"的中文 Err（不触发执行器）
/// - 执行器网络失败 / 瞬时可重试错误重试耗尽：`TranslateError` 转字符串
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
    let tracker = InflightTracker::new();
    let runtime = TranslateRuntime {
        max_attempts: TRANSLATE_MAX_ATTEMPTS,
        sleep: &sleep_blocking,
        tracker: &tracker,
    };
    translate_text_with_runtime(
        conn,
        exec,
        input,
        settings_path,
        cred_store,
        ecdict_db,
        &runtime,
    )
}

/// 带运行期横切配置（重试 / 取消）的翻译主链路实现。
///
/// 与 `translate_text_impl` 的差别只有两点：重试等待函数与在途追踪器由调用方注入。
/// 生产入口 `translate_text` 用 `TranslateRuntime::production()` 获得真实休眠 +
/// 进程内共享 tracker；单测注入记录退避序列的闭包与独立 tracker，从而不引入真实等待、
/// 也不受其它并发测试影响。
///
/// # Errors
/// 同 [`translate_text_impl`]，外加「请求被更新的请求取代」的取消错误。
fn translate_text_with_runtime(
    conn: &Connection,
    exec: &dyn HttpExecutor,
    input: TranslateInput<'_>,
    settings_path: &Path,
    cred_store: &dyn CredStore,
    ecdict_db: Arc<EcdictDb>,
    runtime: &TranslateRuntime<'_>,
) -> Result<TranslateResultDto, String> {
    let text = input.text;
    if text.trim().is_empty() {
        return Err("翻译文本不能为空或全空白".to_string());
    }

    let provider_id = get_selected_provider_impl(settings_path)?;

    // 方向先于缓存查询解析：缓存键必须由最终语言对构成。
    let target_lang = input.configured_target.map(Lang::new);
    let (source, target) =
        resolve_direction_with_source(text, input.configured_source, target_lang);
    let ctx = ResolvedTranslation {
        text,
        source: &source,
        target: &target,
        provider_id: &provider_id,
    };

    // 本请求的 generation：之后任何一次新请求都会让它失效。
    let generation = runtime.tracker.begin();

    // 命中缓存：零网络，也不必读凭据（凭据缺失时仍可复用既有译文）。
    if let Some(translated) = lookup_cache(conn, &ctx) {
        ensure_still_current(runtime.tracker, generation)?;
        record_history(conn, &ctx, &translated)?;
        return Ok(plain_dto(translated, &source, &target));
    }

    let response = translate_uncached(conn, exec, &ctx, cred_store, ecdict_db, runtime);
    // 任何副作用之前校验 generation：被取代的请求整条作废，不写缓存也不写历史。
    ensure_still_current(runtime.tracker, generation)?;
    let response = response?;

    // 只缓存 Plain。缓存表只存一段 `translated` 纯文本，`Dict` 的结构化 entry
    // （音标/词性分组释义/例句/词形）没有存放位置；若把 Dict 压成摘要写进缓存，
    // 命中时只能还原成纯文本译文，词典源会静默退化为机翻样式，属于用户可见的能力损失。
    // 收益也有限：现存 Dict 源（ecdict 本地词库、youdao_dict 回退分支）要么零网络、
    // 要么本就按「是否单词」分流，重复查询代价远小于一次错误降级。
    // 「只写 Plain」这条不变量同时让读取侧安全：表里任何记录都必然来自 Plain 响应。
    if let TranslateResponse::Plain { translated } = &response {
        store_cache(conn, &ctx, translated);
    }

    let (kind, translated, entry) = match response {
        TranslateResponse::Plain { translated } => ("plain", translated, None),
        TranslateResponse::Dict { entry } => ("dict", dict_entry_summary(&entry), Some(entry)),
    };
    record_history(conn, &ctx, &translated)?;

    Ok(TranslateResultDto {
        kind: kind.to_string(),
        translated,
        entry,
        source_lang: source.as_str().to_string(),
        target_lang: target.as_str().to_string(),
    })
}

/// 方向解析之后的翻译上下文：原文 + 最终语言对 + 选中的 provider。
///
/// 缓存键、历史写入与 DTO 构造都要用到这四项，聚为一组可让各段辅助函数的参数
/// 数量保持在 clippy 上限内（设计同 code-standards「参数 > 5 改结构体」）。
struct ResolvedTranslation<'a> {
    text: &'a str,
    source: &'a Lang,
    target: &'a Lang,
    provider_id: &'a str,
}

/// 走网络（含退避重试）完成一次翻译，返回 provider 的统一响应。
///
/// 重试范围覆盖整个 provider 编排（含多步源内部的多次 HTTP），保证「同源」语义。
///
/// # Errors
/// - 必填凭据缺失：返回含"未配置"的中文 Err（不触发执行器）
/// - 瞬时错误重试耗尽 / 永久错误：`TranslateError` 转字符串
fn translate_uncached(
    conn: &Connection,
    exec: &dyn HttpExecutor,
    ctx: &ResolvedTranslation<'_>,
    cred_store: &dyn CredStore,
    ecdict_db: Arc<EcdictDb>,
    runtime: &TranslateRuntime<'_>,
) -> Result<TranslateResponse, String> {
    let creds = load_credentials(ctx.provider_id, cred_store, conn).map_err(|e| e.to_string())?;
    let provider = build_provider(ctx.provider_id, &creds, Some(ecdict_db))?;

    let req = TranslateRequest {
        text: ctx.text.to_string(),
        source_lang: ctx.source.clone(),
        target_lang: ctx.target.clone(),
    };

    retry_with_backoff(
        runtime.max_attempts,
        || provider.translate(&req, exec),
        runtime.sleep,
    )
    .map_err(|e| e.to_string())
}

/// 按四元组查缓存（best-effort）。
///
/// 命中返回已缓存译文；未命中或查询失败返回 `None`。缓存是纯优化，DB 读失败不应让
/// 用户翻译直接失败，因此降级为「无缓存」并通过 stderr 留痕（不静默吞掉）。
fn lookup_cache(conn: &Connection, ctx: &ResolvedTranslation<'_>) -> Option<String> {
    let key = cache_key(
        ctx.text,
        ctx.source.as_str(),
        ctx.target.as_str(),
        ctx.provider_id,
    );
    match cache_get(conn, &key) {
        Ok(hit) => hit,
        Err(e) => {
            eprintln!("[QuickQuick] 读取翻译缓存失败，降级为直接请求翻译源: {e}");
            None
        }
    }
}

/// 写入缓存并触发 LRU 淘汰（best-effort）。
///
/// 写失败只影响后续命中率，不影响本次翻译结果，因此留痕后继续返回译文。
fn store_cache(conn: &Connection, ctx: &ResolvedTranslation<'_>, translated: &str) {
    let result = cache_put(
        conn,
        ctx.text,
        ctx.source.as_str(),
        ctx.target.as_str(),
        ctx.provider_id,
        translated,
        TRANSLATE_CACHE_CAPACITY,
    );
    if let Err(e) = result {
        eprintln!("[QuickQuick] 写入翻译缓存失败，本次译文仍正常返回: {e}");
    }
}

/// 返回结果前校验 generation：已被更新请求取代时返回取消错误。
fn ensure_still_current(tracker: &InflightTracker, generation: u64) -> Result<(), String> {
    if tracker.is_current(generation) {
        Ok(())
    } else {
        Err(TRANSLATE_CANCELLED_MESSAGE.to_string())
    }
}

/// 写入翻译历史；命中缓存与未命中路径共用，保证「命中缓存也算翻译过」。
fn record_history(
    conn: &Connection,
    ctx: &ResolvedTranslation<'_>,
    translated: &str,
) -> Result<(), String> {
    add_translate_history(
        conn,
        ctx.text,
        translated,
        ctx.source.as_str(),
        ctx.target.as_str(),
        ctx.provider_id,
    )
    .map(|_id| ())
    .map_err(|e| e.to_string())
}

/// 构造 Plain 结果 DTO（缓存命中路径使用：缓存里只有纯文本译文，没有结构化词条）。
fn plain_dto(translated: String, source: &Lang, target: &Lang) -> TranslateResultDto {
    TranslateResultDto {
        kind: "plain".to_string(),
        translated,
        entry: None,
        source_lang: source.as_str().to_string(),
        target_lang: target.as_str().to_string(),
    }
}

/// `list_translate_history` 的纯函数实现，可在测试中直接调用。
///
/// 按 created_utc 倒序返回**最近**一页翻译历史，条数上限由
/// [`crate::translate::history::TRANSLATE_HISTORY_PAGE_LIMIT`] 决定（不在本层重复定义，
/// 避免两处上限各自漂移）。
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
///
/// 走 `TranslateRuntime::production()`：真实退避休眠 + 进程内共享在途追踪器，
/// 因此用户连续发起的翻译只有最新一次的结果会被采纳，旧请求返回取消错误。
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
    let runtime = TranslateRuntime::production();
    // secret 存进同一加密 DB 的 provider_secret 表（去 Keychain），故在 with_db 闭包内构造 DbCredStore。
    let result = with_db(&state, |conn| {
        let cred_store = DbCredStore::new(conn);
        translate_text_with_runtime(
            conn,
            &UreqExecutor,
            TranslateInput {
                text: &text,
                configured_source: source.as_deref(),
                configured_target: target.as_deref(),
            },
            &settings_path,
            &cred_store,
            Arc::clone(&ecdict.0),
            &runtime,
        )
    });
    if result.is_ok() {
        if let Err(e) = app.emit(TRANSLATE_HISTORY_CHANGED_EVENT, ()) {
            eprintln!("[QuickQuick] 发送 {TRANSLATE_HISTORY_CHANGED_EVENT} 事件失败: {e}");
        }
    }
    result
}

/// Tauri 命令：按时间倒序返回最近一页翻译历史。
///
/// 返回条数上限由 `translate::history::TRANSLATE_HISTORY_PAGE_LIMIT` 决定，
/// 因此前端拿到的可能是「最近 N 条」而非全量（前端历史栏据此给出条数提示）。
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
    /// 包含 translate_history、provider_config 与 translation_cache，
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
            );
            CREATE TABLE translation_cache (
                cache_key     TEXT PRIMARY KEY NOT NULL,
                source_text   TEXT NOT NULL,
                source_lang   TEXT NOT NULL,
                target_lang   TEXT NOT NULL,
                provider_id   TEXT NOT NULL,
                translated    TEXT NOT NULL,
                created_utc   INTEGER NOT NULL,
                last_used_utc INTEGER NOT NULL
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

    // ---- 缓存接线（P1-6）----

    /// Google 免费源成功响应的最小合法 JSON 模板。
    fn google_free_ok_response(translated: &str) -> String {
        format!(r#"[[["{translated}","x",null,null,10]]]"#)
    }

    /// 统计缓存表当前行数（用于断言「是否写缓存」）。
    fn cache_row_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM translation_cache", [], |row| {
            row.get(0)
        })
        .expect("统计缓存行数应成功")
    }

    /// 构造运行期配置：注入只记录不退避的 sleep（不做真实休眠）与独立 tracker。
    fn runtime_with<'a>(
        tracker: &'a InflightTracker,
        sleep: &'a dyn Fn(u64),
    ) -> TranslateRuntime<'a> {
        TranslateRuntime {
            max_attempts: TRANSLATE_MAX_ATTEMPTS,
            sleep,
            tracker,
        }
    }

    /// 自动检测方向的输入（原文 + 默认方向）。
    fn auto_input(text: &str) -> TranslateInput<'_> {
        TranslateInput {
            text,
            configured_source: None,
            configured_target: None,
        }
    }

    #[test]
    fn translate_text_impl_second_identical_request_served_from_cache_without_network() {
        // Arrange：同文本、同语言对、同 provider
        let conn = setup_test_db();
        let fake = FakeExecutor::new(&lingva_ok_response("你好世界"));
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "lingva");

        // Act：连续翻译两次
        let first = translate_text_impl(
            &conn,
            &fake,
            auto_input("hello world"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("首次翻译应成功");
        let second = translate_text_impl(
            &conn,
            &fake,
            auto_input("hello world"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("第二次翻译应命中缓存并成功");

        // Assert：第二次零网络，译文与方向与首次一致，缓存表只有一条记录
        assert_eq!(
            fake.call_count(),
            1,
            "第二次翻译应命中缓存，执行器调用次数应保持 1"
        );
        assert_eq!(second.translated, "你好世界");
        assert_eq!(second.translated, first.translated);
        assert_eq!(second.kind, "plain");
        assert_eq!(second.source_lang, "en");
        assert_eq!(second.target_lang, "zh");
        assert_eq!(cache_row_count(&conn), 1, "缓存表应只有一条四元组记录");
    }

    #[test]
    fn translate_text_impl_cache_miss_when_target_language_changes() {
        // Arrange：同文本同 provider，但目标语言不同（en→zh 与 en→ja）
        let conn = setup_test_db();
        let fake = FakeExecutor::new(&lingva_ok_response("你好世界"));
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "lingva");

        // Act
        translate_text_impl(
            &conn,
            &fake,
            auto_input("hello world"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("首次 en→zh 翻译应成功");
        let second = translate_text_impl(
            &conn,
            &fake,
            TranslateInput {
                text: "hello world",
                configured_source: None,
                configured_target: Some("ja"),
            },
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("换目标语言的翻译应成功");

        // Assert：语言对进入缓存键，换语言对必 miss
        assert_eq!(second.target_lang, "ja");
        assert_eq!(
            fake.call_count(),
            2,
            "目标语言不同时缓存键不同，应再次走网络"
        );
        assert_eq!(cache_row_count(&conn), 2, "两个语言对应各留一条缓存");
    }

    #[test]
    fn translate_text_impl_cache_miss_when_provider_changes() {
        // Arrange：先 lingva，再切到 google_free（两者都免 key）
        let conn = setup_test_db();
        let lingva = FakeExecutor::new(&lingva_ok_response("你好世界"));
        let google = FakeExecutor::new(&google_free_ok_response("你好世界"));
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();

        // Act
        write_settings_with_provider(settings_file.path(), "lingva");
        translate_text_impl(
            &conn,
            &lingva,
            auto_input("hello world"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("lingva 翻译应成功");
        write_settings_with_provider(settings_file.path(), "google_free");
        translate_text_impl(
            &conn,
            &google,
            auto_input("hello world"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("google_free 翻译应成功");

        // Assert：provider_id 进入缓存键，换源必 miss
        assert_eq!(lingva.call_count(), 1);
        assert_eq!(
            google.call_count(),
            1,
            "provider 不同时缓存键不同，换源应再次走网络"
        );
        assert_eq!(cache_row_count(&conn), 2, "两个 provider 应各留一条缓存");
    }

    #[test]
    fn translate_text_impl_cache_hit_still_records_history() {
        // Arrange：同文本连续翻译两次
        let conn = setup_test_db();
        let fake = FakeExecutor::new(&lingva_ok_response("你好世界"));
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "lingva");

        // Act
        translate_text_impl(
            &conn,
            &fake,
            auto_input("hello world"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("首次翻译应成功");
        translate_text_impl(
            &conn,
            &fake,
            auto_input("hello world"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("命中缓存的翻译应成功");

        // Assert：命中缓存也算翻译过，历史条数与未命中路径保持一致（两次翻译两条历史）
        let count =
            crate::translate::history::translate_history_count(&conn).expect("查历史数应成功");
        assert_eq!(count, 2, "命中缓存也应写入翻译历史");
    }

    /// 有道词典成功响应的最小合法 JSON 模板（isWord=true 时解析为 Dict）。
    fn youdao_dict_ok_response() -> String {
        r#"{"isWord":true,"basic":{"phonetic":"ˈɡleɪʃər","explains":["n. 冰川"]},"translation":["冰川"]}"#
            .to_string()
    }

    #[test]
    fn translate_text_impl_dict_response_is_not_cached() {
        // Arrange：youdao_dict 是 Dict 产出源，缓存表只存纯文本，Dict 不写缓存
        let conn = setup_test_db();
        let fake = FakeExecutor::new(&youdao_dict_ok_response());
        let store = MockCredStore::new();
        // youdao_dict 的 app_key 是非密字段（落 provider_config 表），app_secret 才是 secret。
        conn.execute(
            "INSERT INTO provider_config (provider_id, field_key, value) VALUES (?1, ?2, ?3)",
            rusqlite::params!["youdao_dict", "app_key", "test-app-key"],
        )
        .expect("预置 youdao_dict app_key 应成功");
        store
            .set_secret("youdao_dict", "app_secret", "test-app-secret")
            .expect("预置 youdao_dict app_secret 应成功");
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "youdao_dict");

        // Act：同文本连续翻译两次
        let first = translate_text_impl(
            &conn,
            &fake,
            auto_input("glacier"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("词典翻译应成功");
        let second = translate_text_impl(
            &conn,
            &fake,
            auto_input("glacier"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
        )
        .expect("词典翻译应成功");

        // Assert：两次都是结构化词条，且缓存表为空（Dict 不写缓存）
        assert_eq!(first.kind, "dict");
        assert!(first.entry.is_some(), "词典结果应带结构化 entry");
        assert_eq!(
            second.kind, "dict",
            "第二次也应保留 dict 类型（未降级为纯文本）"
        );
        assert_eq!(fake.call_count(), 2, "Dict 不缓存，第二次仍走执行器");
        assert_eq!(cache_row_count(&conn), 0, "Dict 结果不应写入缓存");
    }

    // ---- 重试 / 取消接线（P1-7）----

    /// 假执行器注入的错误种类。
    #[derive(Clone, Copy)]
    enum InjectedError {
        /// 瞬时：服务端 5xx。
        ServerError,
        /// 永久：认证失败。
        Auth,
    }

    impl InjectedError {
        fn to_translate_error(self) -> TranslateError {
            match self {
                Self::ServerError => TranslateError::ServerError("HTTP 503".to_string()),
                Self::Auth => TranslateError::Auth("HTTP 401".to_string()),
            }
        }
    }

    /// 前 `failures_left` 次调用返回注入错误、之后返回成功响应的假执行器。
    struct FlakyExecutor {
        failures_left: AtomicU32,
        attempts: AtomicU32,
        injected: InjectedError,
        ok_response: String,
    }

    impl FlakyExecutor {
        fn new(injected: InjectedError, failures: u32, ok_response: String) -> Self {
            Self {
                failures_left: AtomicU32::new(failures),
                attempts: AtomicU32::new(0),
                injected,
                ok_response,
            }
        }

        fn attempt_count(&self) -> u32 {
            self.attempts.load(Ordering::SeqCst)
        }
    }

    impl HttpExecutor for FlakyExecutor {
        fn execute(&self, _req: &ProviderHttpRequest) -> Result<String, TranslateError> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            if self.failures_left.load(Ordering::SeqCst) > 0 {
                self.failures_left.fetch_sub(1, Ordering::SeqCst);
                return Err(self.injected.to_translate_error());
            }
            Ok(self.ok_response.clone())
        }
    }

    #[test]
    fn translate_text_with_runtime_retries_transient_error_then_succeeds() {
        // Arrange：第一次返回 5xx、第二次成功；sleep 注入记录器，零真实等待
        let conn = setup_test_db();
        let exec = FlakyExecutor::new(
            InjectedError::ServerError,
            1,
            lingva_ok_response("你好世界"),
        );
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "lingva");
        let recorded: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
        let sleep = |ms: u64| recorded.lock().unwrap().push(ms);
        let tracker = InflightTracker::new();
        let runtime = runtime_with(&tracker, &sleep);

        // Act
        let dto = translate_text_with_runtime(
            &conn,
            &exec,
            auto_input("hello world"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
            &runtime,
        )
        .expect("瞬时错误重试后应成功");

        // Assert：共尝试 2 次，退避序列为首次退避的 500ms
        assert_eq!(dto.translated, "你好世界");
        assert_eq!(exec.attempt_count(), 2, "瞬时错误应重试一次后成功");
        assert_eq!(
            *recorded.lock().unwrap(),
            vec![500],
            "退避序列应为 [500]，且未发生真实等待"
        );
    }

    #[test]
    fn translate_text_with_runtime_returns_permanent_error_without_retry() {
        // Arrange：认证失败属于永久错误，不得重试
        let conn = setup_test_db();
        let exec = FlakyExecutor::new(InjectedError::Auth, 99, lingva_ok_response("不应被使用"));
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "lingva");
        let recorded: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
        let sleep = |ms: u64| recorded.lock().unwrap().push(ms);
        let tracker = InflightTracker::new();
        let runtime = runtime_with(&tracker, &sleep);

        // Act
        let err = translate_text_with_runtime(
            &conn,
            &exec,
            auto_input("hello world"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
            &runtime,
        )
        .expect_err("认证失败应直接返回错误");

        // Assert：只调用 1 次，不进退避等待，错误归一为 Auth 文案
        assert_eq!(exec.attempt_count(), 1, "永久错误不应重试");
        assert!(
            recorded.lock().unwrap().is_empty(),
            "永久错误不应进入退避等待"
        );
        assert!(err.contains("认证错误"), "错误应归一为 Auth 变体: {err}");
        assert_eq!(
            crate::translate::history::translate_history_count(&conn).expect("查历史数应成功"),
            0,
            "失败的翻译不应写历史"
        );
    }

    /// 在 `execute` 中发起一次新请求的假执行器，模拟「翻译进行中用户又提交了一次」。
    struct SupersedingExecutor<'a> {
        tracker: &'a InflightTracker,
        response: String,
    }

    impl HttpExecutor for SupersedingExecutor<'_> {
        fn execute(&self, _req: &ProviderHttpRequest) -> Result<String, TranslateError> {
            // 故意丢弃新 generation：这里只关心「让本次请求的 generation 过期」。
            let _ = self.tracker.begin();
            Ok(self.response.clone())
        }
    }

    #[test]
    fn translate_text_with_runtime_discards_result_of_superseded_generation() {
        // Arrange：执行器在「网络往返」中发起新请求，使本次 generation 立即过期
        let conn = setup_test_db();
        let tracker = InflightTracker::new();
        let exec = SupersedingExecutor {
            tracker: &tracker,
            response: lingva_ok_response("你好世界"),
        };
        let store = MockCredStore::new();
        let settings_file = tempfile::NamedTempFile::new().unwrap();
        write_settings_with_provider(settings_file.path(), "lingva");
        let noop_sleep = |_ms: u64| {};
        let runtime = runtime_with(&tracker, &noop_sleep);

        // Act
        let result = translate_text_with_runtime(
            &conn,
            &exec,
            auto_input("hello world"),
            settings_file.path(),
            &store,
            placeholder_ecdict_db(),
            &runtime,
        );

        // Assert：旧 generation 的结果被丢弃，且不留下缓存/历史副作用
        let err = result.expect_err("旧 generation 的结果应被丢弃");
        assert_eq!(err, TRANSLATE_CANCELLED_MESSAGE);
        assert_eq!(cache_row_count(&conn), 0, "被作废的请求不应写缓存");
        assert_eq!(
            crate::translate::history::translate_history_count(&conn).expect("查历史数应成功"),
            0,
            "被作废的请求不应写历史"
        );
    }

    // ---- 错误归一（P1-7）----

    /// 在本地回环起一个只回一行状态行的 HTTP 服务，返回监听端口。
    ///
    /// 用于真实走通 `UreqExecutor` → ureq → 非 2xx 状态码的归一链路，全程不访问外网。
    fn spawn_http_status_server(status_line: &'static str) -> u16 {
        use std::io::{BufRead, BufReader, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("绑定回环端口应成功");
        let port = listener.local_addr().expect("读取本地地址应成功").port();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut reader = BufReader::new(stream.try_clone().expect("克隆连接应成功"));
                let mut line = String::new();
                // 读完请求头再回响应，避免客户端仍在写请求时被提前关闭连接
                while reader.read_line(&mut line).unwrap_or(0) > 0 {
                    let is_header_end = line == "\r\n" || line == "\n";
                    line.clear();
                    if is_header_end {
                        break;
                    }
                }
                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        port
    }

    #[test]
    fn ureq_executor_normalizes_http_status_errors() {
        // Arrange：覆盖认证失败、限流、服务端错误三类状态码
        let cases = [
            ("401 Unauthorized", "auth"),
            ("403 Forbidden", "auth"),
            ("429 Too Many Requests", "rate_limit"),
            ("503 Service Unavailable", "server_error"),
        ];

        for (status_line, expected) in cases {
            let port = spawn_http_status_server(status_line);
            let req = ProviderHttpRequest {
                method: "GET",
                url: format!("http://127.0.0.1:{port}/"),
                body: None,
                headers: vec![],
            };

            // Act
            let err = UreqExecutor
                .execute(&req)
                .expect_err("非 2xx 状态码应返回错误");

            // Assert：状态码被归一为具体变体，而非笼统的 Network
            let actual = match &err {
                TranslateError::Auth(_) => "auth",
                TranslateError::RateLimit(_) => "rate_limit",
                TranslateError::ServerError(_) => "server_error",
                TranslateError::Network(_) => "network",
                _ => "other",
            };
            assert_eq!(actual, expected, "HTTP {status_line} 的归一结果不符: {err}");
        }
    }

    #[test]
    fn is_timeout_reads_timed_out_io_source_on_ureq_transport_error() {
        // Arrange：ureq 用 `impl From<io::Error> for Error` 把 io::Error 挂成
        // `Error::Transport` 的 source；真实超时正是 io::ErrorKind::TimedOut 走这条链，
        // 因此这样构造与线上超时错误同形，且零等待、零网络。
        let timed_out = ureq::Error::from(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "timed out reading response",
        ));
        let refused = ureq::Error::from(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "connection refused",
        ));

        // Assert：只有 TimedOut 判为超时
        assert!(
            is_timeout(&timed_out),
            "io::ErrorKind::TimedOut 应识别为超时"
        );
        assert!(!is_timeout(&refused), "其它 IO 错误不应被当作超时");

        // Assert：超时归一为统一文案；非超时传输失败也归 Network，但文案不同
        assert!(
            matches!(normalize_ureq_error(timed_out), TranslateError::Network(msg) if msg == "请求超时"),
            "超时应归一为 Network(\"请求超时\")"
        );
        assert!(
            matches!(normalize_ureq_error(refused), TranslateError::Network(msg) if msg != "请求超时"),
            "非超时传输失败同样归 Network，但不使用超时文案"
        );
    }
}
