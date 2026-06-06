//! 翻译框架核心模块
//!
//! 本模块定义薄 provider 契约（三件职责）与静态注册表。
//! 缓存、限流、凭据、重试、超时、取消等横切关注点不在 trait 上，
//! 由核心框架层（s03–s05）实现。

pub mod cache;
pub mod cancel;
pub mod credential;
pub mod ecdict_db;
pub mod error;
pub mod history;
pub mod lang;
pub mod providers;
pub mod retry;

pub use providers::registry;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// BCP-47 语言标签的薄包装。
///
/// S02 细化前作为简单字符串持有；Provider 映射表（s02）负责抹平 zh/zh-CN/zh-Hans 等变体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lang(String);

impl Lang {
    /// 构造新语言标签。接受任意 BCP-47 串，s02 再做归一验证。
    pub fn new(tag: impl Into<String>) -> Self {
        Self(tag.into())
    }

    /// 返回内部标签字符串引用。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 统一翻译请求。
///
/// 缓存键由 `(text, source_lang, target_lang, provider_id)` 共同决定（s05 实现缓存时使用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateRequest {
    pub text: String,
    pub source_lang: Lang,
    pub target_lang: Lang,
}

/// 统一翻译响应：单段译文（机翻/LLM）或结构化词条（词典源）。
///
/// 用 serde internally tagged 枚举携带 `kind` 判别标签，前端据此分别渲染：
/// - `Plain` → `{"kind":"plain","translated":"..."}`，原译文文本渲染。
/// - `Dict`  → `{"kind":"dict","entry":{...}}`，按 `DictEntry` 结构化渲染。
///
/// 既有 19 源（机翻 + LLM）全部产出 `Plain`；词典 4 源（TV4-F2/F3）产出 `Dict`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TranslateResponse {
    /// 单段译文，携带翻译后的文本。
    Plain {
        /// 翻译后的文本。
        translated: String,
    },
    /// 结构化词典词条。
    Dict {
        /// 词条结构化内容（音标/释义/例句/发音/变形）。
        entry: DictEntry,
    },
}

impl TranslateResponse {
    /// 构造 Plain 变体的便捷方法（既有源高频使用）。
    pub fn plain(translated: impl Into<String>) -> Self {
        Self::Plain {
            translated: translated.into(),
        }
    }
}

/// 词典词条结构化内容。
///
/// 字段全部用 `Option`/`Vec` 容空——不同词典源能提供的字段不一，缺失即空，
/// 序列化后前端按字段有无渲染（设计文档§二.2.4）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DictEntry {
    /// 音标（如 "ˈɡleɪʃər"），无则 None。
    pub phonetic: Option<String>,
    /// 按词性分组的释义列表。
    pub definitions: Vec<PosDefinition>,
    /// 例句列表。
    pub examples: Vec<String>,
    /// 发音音频 URL，无则 None。
    pub audio: Option<String>,
    /// 词形变化（复数/时态等）列表。
    pub inflections: Vec<String>,
}

/// 按词性分组的释义。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PosDefinition {
    /// 词性（如 "noun"/"verb"），无则 None。
    pub pos: Option<String>,
    /// 该词性下的释义文本列表。
    pub meanings: Vec<String>,
}

/// Provider 能力声明。
///
/// 编译期静态注册表用此结构体列举全部 provider 的元数据，
/// 前端凭此动态渲染凭据表单（s04 扩展字段）。
#[derive(Debug, Clone)]
pub struct ProviderCapability {
    /// 唯一 provider 标识，用于缓存键、配置存储、日志。
    pub id: &'static str,
    /// 显示名称（UI 展示用）。
    pub name: &'static str,
    /// 是否需要用户提供 API Key。免 key 源（如 lingva）false、需 key 源（如 baidu）true。
    pub needs_key: bool,
    /// 是否为非官方/自建接口（免 key 源均为非官方，随对方改版即可能失效）。
    ///
    /// 前端据此渲染「⚠ 非官方」标注并在失败时给降级提示（设计文档§三.决策3）。
    /// lingva/google_free/yandex/transmart/bing 为 true；官方 keyed 源 baidu/deepl_free/google 为 false。
    pub is_unofficial: bool,
}

/// Provider 的 HTTP 请求描述符。
///
/// 只描述请求意图，不真发网络——真实 HTTP 调用由核心框架（s03/s07）统一执行，
/// 这样超时/取消/重试逻辑可集中管理。
#[derive(Debug, Clone)]
pub struct ProviderHttpRequest {
    /// HTTP 方法（"GET" / "POST"）。
    pub method: &'static str,
    /// 目标 URL（含查询参数）。
    pub url: String,
    /// POST body（GET 请求为 None）。
    pub body: Option<String>,
    /// 额外请求头（key → value 键值对列表）。
    ///
    /// 大多数 provider 不需要自定义头部，默认空列表即可。
    /// DeepL 等需要 Authorization 头的 provider 在此声明。
    pub headers: Vec<(String, String)>,
}

/// 统一翻译错误枚举（s03 归一映射）。
///
/// 每个变体携带人类可读的上下文字符串。
/// `ParseError` 保留 s01/s02 既有用法；其余变体为 s03 新增。
#[derive(Debug, Error)]
pub enum TranslateError {
    /// 响应解析失败（JSON 格式错误或缺少字段）。s01/s02 既有。
    #[error("解析错误: {0}")]
    ParseError(String),

    /// 网络层错误（超时、连接拒绝、DNS 失败等）。
    /// 超时也归入此变体——超时本质是网络层未在期限内响应。
    #[error("网络错误: {0}")]
    Network(String),

    /// 认证失败（HTTP 401/403，或 API Key 无效）。
    #[error("认证错误: {0}")]
    Auth(String),

    /// 请求频率超限（HTTP 429）。瞬时可重试。
    #[error("频率超限: {0}")]
    RateLimit(String),

    /// 配额耗尽（免费额度用完等）。永久，需用户干预。
    #[error("配额超限: {0}")]
    Quota(String),

    /// 不支持的语言对。永久错误。
    #[error("不支持的语言: {0}")]
    Unsupported(String),

    /// 原文过长，超过 provider 单次限制。永久错误。
    #[error("文本过长: {0}")]
    TooLong(String),

    /// Provider 服务端内部错误（HTTP 5xx）。瞬时可重试。
    #[error("服务端错误: {0}")]
    ServerError(String),
}

/// 可注入 HTTP 执行器抽象（翻译核心框架层）。
///
/// 把真实网络调用抽象为 trait，使多步源（如 Bing）可在 `translate` 内自行编排
/// 多次 HTTP，而单测可注入假执行器完全隔离网络。生产实现（基于 ureq）见
/// `ipc::translate::UreqExecutor`。
pub trait HttpExecutor: Send + Sync {
    /// 按 provider 描述符发起 HTTP 请求，返回响应体原始字符串。
    ///
    /// # Errors
    /// 网络层失败（超时、连接拒绝、DNS 等）映射为 `TranslateError::Network`。
    fn execute(&self, req: &ProviderHttpRequest) -> Result<String, TranslateError>;
}

/// 薄 provider 契约——单步源恰好三件职责：
/// 1. 声明能力（capability）
/// 2. 把统一请求转为自家 HTTP 调用描述（build_request）
/// 3. 把原始响应/错误解析回统一结果（parse_response）
///
/// 多步源（如 Bing 需先抓 token 再翻译）override 默认 `translate`，自行用注入的
/// `HttpExecutor` 编排多次 HTTP。单步源沿用默认 `translate`（build→execute→parse），
/// 无需改动任何代码即自动适配执行流，这是架构扩展不回归既有源的保证。
///
/// 以下关注点**不在本 trait 上**，由核心框架横切实现：
/// - 缓存（s05）
/// - 限流（s03）
/// - 凭据读取（s04）
/// - 重试（s03）
/// - 超时与取消（s03）
pub trait TranslateProvider: Send + Sync {
    /// 声明 provider 的静态元数据（id、名称、是否需要 key 等）。
    fn capability(&self) -> ProviderCapability;

    /// 将统一请求转为该 provider 的 HTTP 调用描述。
    ///
    /// 不实际发起网络请求——框架负责执行并传入原始响应体供 `parse_response` 处理。
    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest;

    /// 将 provider 原始响应体解析为统一结果或统一错误。
    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError>;

    /// 执行完整翻译流程（默认实现 = 单步 build_request→execute→parse_response）。
    ///
    /// 多步源 override 此方法，在内部按需多次调用 `executor.execute`。
    /// 默认实现与重构前 `ipc::translate` 的手动三步**逐字等价**，使既有单步源零改动适配。
    ///
    /// # Errors
    /// - 执行器网络失败 → `TranslateError::Network`
    /// - 响应解析失败 → `TranslateError::ParseError` 等
    fn translate(
        &self,
        req: &TranslateRequest,
        executor: &dyn HttpExecutor,
    ) -> Result<TranslateResponse, TranslateError> {
        let http_req = self.build_request(req);
        let raw = executor.execute(&http_req)?;
        self.parse_response(&raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 对齐 acceptance TV4-F1-A01：Plain 变体 JSON 往返带 kind 标签且语义不丢。
    #[test]
    fn translate_response_plain_variant_roundtrip() {
        let resp = TranslateResponse::plain("glacier");
        let json = serde_json::to_string(&resp).expect("Plain 序列化应成功");
        assert_eq!(json, r#"{"kind":"plain","translated":"glacier"}"#);

        let back: TranslateResponse = serde_json::from_str(&json).expect("Plain 反序列化应成功");
        assert_eq!(back, resp, "往返后应与原值相等");
        assert!(
            matches!(&back, TranslateResponse::Plain { translated } if translated == "glacier"),
            "往返后应仍为 Plain 且译文不变"
        );
    }

    // 对齐 acceptance TV4-F1-A01：既有源 parse_response 仍返回 Plain，不回归为 Dict。
    #[test]
    fn existing_providers_return_plain_no_regression() {
        let provider = providers::build_provider("lingva", &[], None).expect("lingva 应构造成功");
        let resp = provider
            .parse_response(r#"{"translation":"glacier"}"#)
            .expect("含 translation 字段应解析成功");
        assert!(
            matches!(&resp, TranslateResponse::Plain { translated } if translated == "glacier"),
            "既有机翻源应返回 Plain 变体，实际：{resp:?}"
        );
    }

    // 对齐 acceptance TV4-F1-A01：Dict 变体序列化带 kind 类型标签，前端可判别。
    #[test]
    fn dict_entry_serializes_with_type_tag() {
        let entry = DictEntry {
            phonetic: Some("ˈɡleɪʃər".to_string()),
            definitions: vec![PosDefinition {
                pos: Some("noun".to_string()),
                meanings: vec!["冰川".to_string()],
            }],
            examples: vec!["The glacier melts.".to_string()],
            audio: None,
            inflections: vec!["glaciers".to_string()],
        };
        let resp = TranslateResponse::Dict { entry };
        let value: serde_json::Value = serde_json::to_value(&resp).expect("Dict 序列化应成功");

        assert_eq!(value["kind"], "dict", "应携带 kind=dict 类型标签");
        assert_eq!(value["entry"]["phonetic"], "ˈɡleɪʃər");
        assert_eq!(value["entry"]["definitions"][0]["pos"], "noun");
        assert_eq!(value["entry"]["definitions"][0]["meanings"][0], "冰川");
        assert_eq!(value["entry"]["examples"][0], "The glacier melts.");
        assert_eq!(value["entry"]["inflections"][0], "glaciers");
    }
}
