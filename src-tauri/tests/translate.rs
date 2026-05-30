//! 翻译框架集成测试
//!
//! 覆盖：
//! - V2-F1-A01 provider_trait_contract：薄 provider 契约三件职责可用，缓存/限流等不在 trait 上
//! - V2-F1-A08 static_registry_lists_four：静态注册表枚举 4 家 provider

use quickquick_lib::translate::{
    Lang, ProviderCapability, ProviderHttpRequest, TranslateError, TranslateProvider,
    TranslateRequest, TranslateResponse, registry,
};

// ──────────── 测试用 stub provider ────────────

/// 最小 stub provider，仅实现 trait 规定的三件职责。
/// 缓存、限流、凭据、重试、超时、取消均不在此 trait 上——由核心框架横切（后续小功能实现）。
struct StubProvider;

impl TranslateProvider for StubProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "stub",
            name: "Stub Provider",
            needs_key: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        ProviderHttpRequest {
            url: format!(
                "https://stub.example.com/translate?q={}&source={}&target={}",
                req.text, req.source_lang.as_str(), req.target_lang.as_str()
            ),
            body: None,
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;
        let translated = v["translated"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing 'translated' field".to_string()))?
            .to_string();
        Ok(TranslateResponse { translated })
    }
}

// ────────── V2-F1-A01 provider_trait_contract ──────────

/// A01：薄 provider 契约——三件职责均可调用，且不含缓存/限流/凭据等横切关注点。
#[test]
fn provider_contract_capability_returns_correct_fields() {
    // Arrange
    let provider = StubProvider;

    // Act
    let cap = provider.capability();

    // Assert
    assert_eq!(cap.id, "stub");
    assert_eq!(cap.name, "Stub Provider");
    assert!(!cap.needs_key);
}

#[test]
fn provider_contract_build_request_produces_assertable_descriptor() {
    // Arrange
    let provider = StubProvider;
    let req = TranslateRequest {
        text: "hello".to_string(),
        source_lang: Lang::new("en"),
        target_lang: Lang::new("zh"),
    };

    // Act
    let http_req = provider.build_request(&req);

    // Assert：URL 包含所有必要字段，不真发网络
    assert!(http_req.url.contains("hello"), "URL 应含原文");
    assert!(http_req.url.contains("en"), "URL 应含源语言");
    assert!(http_req.url.contains("zh"), "URL 应含目标语言");
}

#[test]
fn provider_contract_parse_response_extracts_translated_text() {
    // Arrange
    let provider = StubProvider;
    let mock_json = r#"{"translated": "你好"}"#;

    // Act
    let result = provider.parse_response(mock_json);

    // Assert
    let resp = result.expect("合法 JSON 应解析成功");
    assert_eq!(resp.translated, "你好");
}

#[test]
fn provider_contract_parse_response_returns_error_on_invalid_json() {
    // Arrange
    let provider = StubProvider;
    let bad_input = "not valid json {{";

    // Act
    let result = provider.parse_response(bad_input);

    // Assert：非法输入应返回 ParseError，不 panic
    assert!(result.is_err(), "非法 JSON 应返回 Err");
    assert!(matches!(result.unwrap_err(), TranslateError::ParseError(_)));
}

#[test]
fn provider_contract_parse_response_returns_error_on_missing_field() {
    // Arrange
    let provider = StubProvider;
    let json_without_field = r#"{"other_field": "value"}"#;

    // Act
    let result = provider.parse_response(json_without_field);

    // Assert：缺少 translated 字段应返回 ParseError
    assert!(result.is_err(), "缺少字段应返回 Err");
    assert!(matches!(result.unwrap_err(), TranslateError::ParseError(_)));
}

// ────────── V2-F1-A08 static_registry_lists_four ──────────

/// A08：静态注册表枚举 4 家 provider。
#[test]
fn static_registry_lists_four_providers() {
    // Arrange + Act
    let providers = registry();

    // Assert
    assert_eq!(providers.len(), 4, "注册表应恰好包含 4 家 provider");
}

#[test]
fn static_registry_contains_mymemory() {
    // Arrange + Act
    let providers = registry();
    let ids: Vec<&str> = providers.iter().map(|p| p.id).collect();

    // Assert
    assert!(ids.contains(&"mymemory"), "注册表应包含 MyMemory");
}

#[test]
fn static_registry_contains_baidu() {
    // Arrange + Act
    let providers = registry();
    let ids: Vec<&str> = providers.iter().map(|p| p.id).collect();

    // Assert
    assert!(ids.contains(&"baidu"), "注册表应包含百度");
}

#[test]
fn static_registry_contains_deepl() {
    // Arrange + Act
    let providers = registry();
    let ids: Vec<&str> = providers.iter().map(|p| p.id).collect();

    // Assert
    assert!(ids.contains(&"deepl_free"), "注册表应包含 DeepL Free");
}

#[test]
fn static_registry_contains_google() {
    // Arrange + Act
    let providers = registry();
    let ids: Vec<&str> = providers.iter().map(|p| p.id).collect();

    // Assert
    assert!(ids.contains(&"google"), "注册表应包含 Google");
}

#[test]
fn static_registry_mymemory_does_not_need_key() {
    // Arrange + Act
    let providers = registry();
    let mymemory = providers.iter().find(|p| p.id == "mymemory");

    // Assert：MyMemory 是默认无需 key 的源
    let cap = mymemory.expect("注册表中应有 MyMemory");
    assert!(!cap.needs_key, "MyMemory 应为 needs_key=false（默认源）");
}

#[test]
fn static_registry_keyed_providers_need_key() {
    // Arrange + Act
    let providers = registry();

    // Assert：百度/DeepL/Google 均需要 key
    for cap in providers.iter().filter(|p| p.id != "mymemory") {
        assert!(
            cap.needs_key,
            "provider '{}' 应为 needs_key=true",
            cap.id
        );
    }
}
