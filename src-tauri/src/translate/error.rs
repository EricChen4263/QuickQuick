//! 错误归一映射（V2-F1-S03 A03）。
//!
//! 把 provider 原始 HTTP 状态码与业务错误码归一到 `TranslateError` 具体变体，
//! 使上层逻辑只需面对统一枚举，无需感知各 provider 的差异。

use super::TranslateError;

/// 将 provider 原始错误归一为 `TranslateError`。
///
/// 优先级：`provider_code` 精确匹配 > HTTP 状态码段。
///
/// - `http_status == 0`：网络层失败（未收到 HTTP 响应）→ `Network`
/// - `provider_code` 在已知 quota code 集合中 → `Quota`
/// - `provider_code` 在已知 too_long code 集合中 → `TooLong`
/// - `provider_code` 在已知 unsupported code 集合中 → `Unsupported`
/// - `401 / 403` → `Auth`
/// - `429` → `RateLimit`
/// - `5xx` → `ServerError`
/// - 其余 → `ServerError`（保守兜底）
pub fn map_provider_error(http_status: u16, provider_code: Option<&str>) -> TranslateError {
    if http_status == 0 {
        return TranslateError::Network("网络层失败，未收到 HTTP 响应".to_string());
    }

    if let Some(code) = provider_code {
        if let Some(err) = map_by_provider_code(code) {
            return err;
        }
    }

    map_by_http_status(http_status)
}

/// 根据 provider 业务错误码精确归一，返回 None 表示未命中任何已知 code。
///
/// 使用精确集合匹配而非子串 contains，避免 `quota_remaining`/`unsupported_format` 等
/// 含相同子串但语义不同的 code 被误命中。
fn map_by_provider_code(code: &str) -> Option<TranslateError> {
    const QUOTA_CODES: &[&str] = &["quota_exceeded", "insufficient_quota", "quota_limit_exceeded"];
    const TOO_LONG_CODES: &[&str] = &["text_too_long", "too_long", "content_too_large"];
    const UNSUPPORTED_CODES: &[&str] = &["unsupported_lang", "unsupported_language", "language_not_supported"];

    if QUOTA_CODES.contains(&code) {
        return Some(TranslateError::Quota(format!("配额超限: {code}")));
    }
    if TOO_LONG_CODES.contains(&code) {
        return Some(TranslateError::TooLong(format!("文本过长: {code}")));
    }
    if UNSUPPORTED_CODES.contains(&code) {
        return Some(TranslateError::Unsupported(format!("不支持的语言: {code}")));
    }
    None
}

/// 根据 HTTP 状态码段归一。
fn map_by_http_status(status: u16) -> TranslateError {
    match status {
        401 | 403 => TranslateError::Auth(format!("认证失败: HTTP {status}")),
        429 => TranslateError::RateLimit(format!("频率超限: HTTP {status}")),
        500..=599 => TranslateError::ServerError(format!("服务端错误: HTTP {status}")),
        _ => TranslateError::ServerError(format!("未知错误: HTTP {status}")),
    }
}

/// 将调用超时归一为 `Network` 变体。
///
/// 超时本质是网络层未在期限内响应，归入 `Network` 枚举语义，
/// 与连接拒绝、DNS 失败等同等对待，上层统一按 Network 处理。
pub fn classify_timeout() -> TranslateError {
    TranslateError::Network("请求超时".to_string())
}
