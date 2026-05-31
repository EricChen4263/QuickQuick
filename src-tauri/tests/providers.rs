//! Provider 集成测试
//!
//! 覆盖：
//! - V2-F2-A09 provider_mymemory：MyMemory 适配（默认源，无 key）
//!   capability + build_request（URL 编码、email 可选）+ parse_response（成功/quota/rate-limit/非法 JSON）

use quickquick_lib::translate::{Lang, TranslateError, TranslateProvider, TranslateRequest};

// 构造测试用 TranslateRequest 的辅助函数
fn make_request(text: &str, src: &str, tgt: &str) -> TranslateRequest {
    TranslateRequest {
        text: text.to_string(),
        source_lang: Lang::new(src),
        target_lang: Lang::new(tgt),
    }
}

// V2-F2-A09 provider_mymemory

/// A09-1: capability 声明 id=mymemory、needs_key=false
#[test]
fn provider_mymemory_capability_id_and_no_key() {
    // Arrange
    let provider = quickquick_lib::translate::providers::MyMemoryProvider::new(None);

    // Act
    let cap = provider.capability();

    // Assert
    assert_eq!(cap.id, "mymemory");
    assert!(!cap.needs_key, "MyMemory 默认源无需 API Key");
}

/// A09-2: build_request 生成的 URL 含正确编码的 q= 参数和 langpair=
#[test]
fn provider_mymemory_build_request_url_encoding() {
    // Arrange
    let provider = quickquick_lib::translate::providers::MyMemoryProvider::new(None);
    let req = make_request("hello world", "en", "zh");

    // Act
    let http_req = provider.build_request(&req);

    // Assert: 空格必须编码（%20 或 +），langpair 须含 en|zh-CN（mymemory 中文映射）
    assert!(
        http_req.url.contains("q=hello%20world") || http_req.url.contains("q=hello+world"),
        "URL 中原文须做 percent-encoding，实际 URL: {}",
        http_req.url
    );
    assert!(
        http_req.url.contains("langpair=en%7Czh-CN") || http_req.url.contains("langpair=en|zh-CN"),
        "langpair 须含 en|zh-CN（mymemory 的中文代码），实际 URL: {}",
        http_req.url
    );
    assert!(http_req.body.is_none(), "MyMemory 使用 GET，body 应为 None");
}

/// A09-3: 提供 email 时 URL 含 de= 参数
#[test]
fn provider_mymemory_build_request_includes_email_when_provided() {
    // Arrange
    let provider = quickquick_lib::translate::providers::MyMemoryProvider::new(
        Some("user@example.com".to_string()),
    );
    let req = make_request("hi", "en", "zh");

    // Act
    let http_req = provider.build_request(&req);

    // Assert
    assert!(
        http_req.url.contains("de=user%40example.com") || http_req.url.contains("de=user@example.com"),
        "提供 email 时 URL 须含 de= 参数，实际 URL: {}",
        http_req.url
    );
}

/// A09-4: 不提供 email 时 URL 不含 de= 参数
#[test]
fn provider_mymemory_build_request_no_email_param_when_none() {
    // Arrange
    let provider = quickquick_lib::translate::providers::MyMemoryProvider::new(None);
    let req = make_request("hi", "en", "zh");

    // Act
    let http_req = provider.build_request(&req);

    // Assert
    assert!(
        !http_req.url.contains("de="),
        "不提供 email 时 URL 不应含 de= 参数，实际 URL: {}",
        http_req.url
    );
}

/// A09-5: parse_response 成功路径——解析正确 JSON 返回 translated
#[test]
fn provider_mymemory_parse_response_success() {
    // Arrange
    let provider = quickquick_lib::translate::providers::MyMemoryProvider::new(None);
    let raw = r#"{"responseData":{"translatedText":"你好，世界"},"responseStatus":200}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let resp = result.expect("成功 JSON 应解析成功");
    assert_eq!(resp.translated, "你好，世界");
}

/// A09-6: parse_response 配额耗尽 responseStatus=403（Number）+ quota 文案 → 精确归 Quota
#[test]
fn provider_mymemory_parse_response_quota_exceeded() {
    // Arrange
    let provider = quickquick_lib::translate::providers::MyMemoryProvider::new(None);
    let raw = r#"{"responseData":{"translatedText":""},"responseStatus":403,"responseDetails":"MYMEMORY WARNING: YOU USED ALL AVAILABLE FREE TRANSLATIONS FOR TODAY. NEXT AVAILABLE: ..."}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert: 403 + quota 文案必须精确归入 Quota，不接受 Auth
    let err = result.expect_err("配额耗尽 JSON 应返回错误");
    assert!(
        matches!(err, TranslateError::Quota(_)),
        "responseStatus=403+quota文案应精确归入 Quota，实际: {:?}",
        err
    );
}

/// A09-9: parse_response responseStatus 为字符串 "403" + quota 文案 → 归 Quota，不误判为成功
#[test]
fn provider_mymemory_parse_response_quota_status_as_string() {
    // Arrange
    let provider = quickquick_lib::translate::providers::MyMemoryProvider::new(None);
    // 部分 MyMemory 响应 responseStatus 以字符串形式返回
    let raw = r#"{"responseData":{"translatedText":""},"responseStatus":"403","responseDetails":"MYMEMORY WARNING: YOU USED ALL AVAILABLE FREE TRANSLATIONS FOR TODAY. NEXT AVAILABLE: ..."}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert: 字符串 "403" 须被正确解析为 403，归入 Quota
    let err = result.expect_err("字符串 responseStatus=403 + quota 文案应返回错误，不得误判为成功");
    assert!(
        matches!(err, TranslateError::Quota(_)),
        "responseStatus 字符串 \"403\"+quota 文案应归入 Quota，实际: {:?}",
        err
    );
}

/// A09-7: parse_response 速率限制 responseStatus=429 → TranslateError::RateLimit
#[test]
fn provider_mymemory_parse_response_rate_limit() {
    // Arrange
    let provider = quickquick_lib::translate::providers::MyMemoryProvider::new(None);
    let raw = r#"{"responseData":{"translatedText":""},"responseStatus":429,"responseDetails":"TOO MANY REQUESTS"}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let err = result.expect_err("429 响应应返回错误");
    assert!(
        matches!(err, TranslateError::RateLimit(_)),
        "responseStatus=429 应归入 RateLimit，实际: {:?}",
        err
    );
}

/// A09-8: parse_response 非法 JSON → TranslateError::ParseError
#[test]
fn provider_mymemory_parse_response_invalid_json() {
    // Arrange
    let provider = quickquick_lib::translate::providers::MyMemoryProvider::new(None);
    let raw = "this is not json at all {{";

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let err = result.expect_err("非法 JSON 应返回 ParseError");
    assert!(
        matches!(err, TranslateError::ParseError(_)),
        "非法 JSON 应归入 ParseError，实际: {:?}",
        err
    );
}
