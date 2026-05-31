//! 翻译框架核心模块
//!
//! 本模块定义薄 provider 契约（三件职责）与静态注册表。
//! 缓存、限流、凭据、重试、超时、取消等横切关注点不在 trait 上，
//! 由核心框架层（后续小功能 s03–s05）实现。

pub mod lang;
mod providers;

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

/// 统一翻译响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateResponse {
    pub translated: String,
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
    /// 是否需要用户提供 API Key。MyMemory 为 false（默认源）。
    pub needs_key: bool,
}

/// Provider 的 HTTP 请求描述符。
///
/// 只描述请求意图，不真发网络——真实 HTTP 调用由核心框架（s03/s07）统一执行，
/// 这样超时/取消/重试逻辑可集中管理。
#[derive(Debug, Clone)]
pub struct ProviderHttpRequest {
    /// 目标 URL（含查询参数）。
    pub url: String,
    /// POST body（GET 请求为 None）。
    pub body: Option<String>,
}

/// 翻译错误枚举。
///
/// 本次为占位变体（s03 细化归一映射：quota/auth/network/ratelimit/unsupported/tooLong/serverError）。
#[derive(Debug, Error)]
pub enum TranslateError {
    /// 响应解析失败（JSON 格式错误或缺少字段）。
    #[error("解析错误: {0}")]
    ParseError(String),

    /// 网络层错误（超时、连接拒绝等）；s03 细化。
    #[error("网络错误: {0}")]
    NetworkError(String),

    /// Provider 返回的通用业务错误；s03 细化为具体变体。
    #[error("Provider 错误: {0}")]
    ProviderError(String),
}

/// 薄 provider 契约——恰好三件职责：
/// 1. 声明能力（capability）
/// 2. 把统一请求转为自家 HTTP 调用描述（build_request）
/// 3. 把原始响应/错误解析回统一结果（parse_response）
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
}
