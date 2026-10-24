//! 错误归一映射（V2-F1-S03 A03）。
//!
//! 把 provider 原始 HTTP 状态码与业务错误码归一到 `TranslateError` 具体变体，
//! 使上层逻辑只需面对统一枚举，无需感知各 provider 的差异。
//!
//! 归一结果是重试决策的唯一依据（`retry::is_transient`）：只有网络抖动、限流、
//! 5xx 这类瞬时失败可以退避重试；配额耗尽、鉴权失败等持续性失败必须立即返回，
//! 否则重试只会成倍放大对 provider 的请求。

use super::{providers, TranslateError};

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
/// - DeepL 私有状态码 `456`（配额耗尽）→ `Quota`
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
    const QUOTA_CODES: &[&str] = &[
        "quota_exceeded",
        "insufficient_quota",
        "quota_limit_exceeded",
    ];
    const TOO_LONG_CODES: &[&str] = &["text_too_long", "too_long", "content_too_large"];
    const UNSUPPORTED_CODES: &[&str] = &[
        "unsupported_lang",
        "unsupported_language",
        "language_not_supported",
    ];

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
///
/// 除标准状态码段外，还要处理 **provider 私有状态码**：DeepL 用 `456` 表达
/// 「配额耗尽」（非标准 HTTP 语义），这类失败只有状态码能分辨——响应体文案不保证
/// 稳定，不能作为判据。语义与文案属于该 provider，故通用表只做路由，由 DeepL
/// 自己的映射表产出具体变体，避免 provider 专属词汇混进标准状态码段的映射。
fn map_by_http_status(status: u16) -> TranslateError {
    match status {
        401 | 403 => TranslateError::Auth(format!("认证失败: HTTP {status}")),
        429 => TranslateError::RateLimit(format!("频率超限: HTTP {status}")),
        providers::DEEPL_QUOTA_EXCEEDED_STATUS => providers::map_deepl_http_status(status, ""),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::translate::UreqExecutor;
    use crate::translate::retry::retry_with_backoff;
    use crate::translate::{HttpExecutor, ProviderHttpRequest};

    /// 把错误变体压成标签，便于逐状态码表格式断言（`TranslateError` 未实现 `PartialEq`）。
    fn variant_label(err: &TranslateError) -> &'static str {
        match err {
            TranslateError::ParseError(_) => "parse_error",
            TranslateError::Network(_) => "network",
            TranslateError::Auth(_) => "auth",
            TranslateError::RateLimit(_) => "rate_limit",
            TranslateError::Quota(_) => "quota",
            TranslateError::Unsupported(_) => "unsupported",
            TranslateError::TooLong(_) => "too_long",
            TranslateError::ServerError(_) => "server_error",
        }
    }

    /// 在本地回环起一个只回一行状态行的 HTTP 服务，返回监听端口。
    ///
    /// 与既有回环测试同风格：真实走通「执行器 → ureq → 非 2xx 状态码 → 归一」
    /// 整条链路，全程不访问外网。
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

    /// 让生产执行器真实请求一个只回指定状态行的回环服务，返回归一后的错误。
    fn normalize_via_production_executor(status_line: &'static str) -> TranslateError {
        let port = spawn_http_status_server(status_line);
        UreqExecutor
            .execute(&ProviderHttpRequest {
                method: "GET",
                url: format!("http://127.0.0.1:{port}/"),
                body: None,
                headers: vec![],
            })
            .expect_err("非 2xx 状态码应返回错误")
    }

    // DeepL 配额耗尽只以状态码 456 表达，必须走真实执行器链路验证其归一结果，
    // 而不是只测映射函数本身（否则「映射写对了但没人调用」的孤儿形态会再次出现）。
    #[test]
    fn http_status_normalization_through_production_executor() {
        // Arrange：DeepL 私有 456 + 三类标准状态码作为对照
        let cases = [
            ("456 Quota Exceeded", "quota"),
            ("403 Forbidden", "auth"),
            ("429 Too Many Requests", "rate_limit"),
            ("503 Service Unavailable", "server_error"),
        ];

        for (status_line, expected) in cases {
            // Act
            let err = normalize_via_production_executor(status_line);

            // Assert
            assert_eq!(
                variant_label(&err),
                expected,
                "HTTP {status_line} 的归一结果不符: {err}"
            );
        }
    }

    // 配额是持续性失败：重试只会把一次失败放大成多次请求，必须立即返回。
    #[test]
    fn deepl_quota_456_is_not_retried() {
        // Arrange
        let mut attempts = 0u32;
        let sleeps: std::cell::RefCell<Vec<u64>> = std::cell::RefCell::new(Vec::new());

        // Act
        let result = retry_with_backoff(
            3,
            || {
                attempts += 1;
                Err::<(), _>(map_provider_error(456, None))
            },
            |ms| sleeps.borrow_mut().push(ms),
        );

        // Assert
        assert!(
            matches!(&result, Err(TranslateError::Quota(_))),
            "456 应归一为 Quota，实际：{result:?}"
        );
        assert_eq!(attempts, 1, "配额错误是永久失败，重试只会放大请求");
        assert!(
            sleeps.borrow().is_empty(),
            "永久错误不应产生退避等待，实际：{:?}",
            sleeps.borrow()
        );
    }

    // 正向对照：同一判据下瞬时错误必须仍能重试，否则上面「只试 1 次」的断言
    // 可能只是因为重试机制整体失效而恒真。
    #[test]
    fn transient_server_error_is_retried_with_backoff() {
        // Arrange
        let mut attempts = 0u32;
        let sleeps: std::cell::RefCell<Vec<u64>> = std::cell::RefCell::new(Vec::new());

        // Act
        let result = retry_with_backoff(
            3,
            || {
                attempts += 1;
                Err::<(), _>(map_provider_error(503, None))
            },
            |ms| sleeps.borrow_mut().push(ms),
        );

        // Assert
        assert!(
            matches!(&result, Err(TranslateError::ServerError(_))),
            "503 应归一为 ServerError，实际：{result:?}"
        );
        assert_eq!(attempts, 3, "瞬时错误应用满重试次数");
        // 既有实现对每次瞬时失败都退避（含最后一次尝试之后），此处顺带锁定该行为
        assert_eq!(
            *sleeps.borrow(),
            vec![500, 1000, 2000],
            "退避序列应为指数退避 500/1000/2000ms"
        );
    }
}
