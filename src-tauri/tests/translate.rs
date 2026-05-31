//! 翻译框架集成测试
//!
//! 覆盖：
//! - V2-F1-A01 provider_trait_contract：薄 provider 契约三件职责可用，缓存/限流等不在 trait 上
//! - V2-F1-A02 lang_normalize_and_direction：语言归一，本地检测定方向，BCP-47，provider 映射表
//! - V2-F1-A08 static_registry_lists_four：静态注册表枚举 4 家 provider

use quickquick_lib::translate::{
    Lang, ProviderCapability, ProviderHttpRequest, TranslateError, TranslateProvider,
    TranslateRequest, TranslateResponse, registry,
};
use quickquick_lib::translate::lang::{detect_is_chinese, detect_lang, map_lang_for_provider, resolve_direction};

// 测试用 stub provider

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

// V2-F1-A01 provider_trait_contract

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

// V2-F1-A08 static_registry_lists_four

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

// V2-F1-A02 lang_normalize_and_direction

/// A02-a：detect_is_chinese 对中文串返回 true，对纯英文串返回 false。
#[test]
fn lang_norm_detect_is_chinese_returns_true_for_cjk_text() {
    // Arrange
    let chinese_text = "你好，世界！";

    // Act
    let result = detect_is_chinese(chinese_text);

    // Assert
    assert!(result, "含 CJK 字符的串应判定为中文");
}

#[test]
fn lang_norm_detect_is_chinese_returns_false_for_ascii_text() {
    // Arrange
    let english_text = "Hello, world!";

    // Act
    let result = detect_is_chinese(english_text);

    // Assert
    assert!(!result, "纯 ASCII 串不应判定为中文");
}

#[test]
fn lang_norm_detect_is_chinese_returns_false_for_japanese_kana() {
    // Arrange：片假名不属于 CJK 统一表意文字区
    let kana_text = "こんにちは";

    // Act
    let result = detect_is_chinese(kana_text);

    // Assert：假名不在 CJK 表意文字区间，应返回 false
    assert!(!result, "纯假名不应判定为中文");
}

#[test]
fn lang_norm_detect_is_chinese_returns_true_for_mixed_text() {
    // Arrange：中英混合文本，含 CJK 字符即判中文（§4.3）
    let mixed_text = "hello 你好";

    // Act
    let result = detect_is_chinese(mixed_text);

    // Assert
    assert!(result, "含 CJK 字符的混合文本应判定为中文");
}

/// A02-b：resolve_direction——中文输入 → (zh, en)；英文输入 → (en, zh)；configured_target 覆盖默认。
#[test]
fn lang_norm_direction_chinese_input_targets_english() {
    // Arrange
    let chinese_text = "今天天气很好";

    // Act
    let (source, target) = resolve_direction(chinese_text, None);

    // Assert
    assert_eq!(source.as_str(), "zh", "中文输入源语言应为 zh");
    assert_eq!(target.as_str(), "en", "中文输入默认目标应为 en");
}

#[test]
fn lang_norm_direction_english_input_targets_chinese() {
    // Arrange
    let english_text = "The weather is nice today";

    // Act
    let (source, target) = resolve_direction(english_text, None);

    // Assert
    assert_eq!(source.as_str(), "en", "英文输入源语言应为 en");
    assert_eq!(target.as_str(), "zh", "英文输入默认目标应为 zh");
}

#[test]
fn lang_norm_direction_configured_target_overrides_default() {
    // Arrange：英文输入，但用户指定目标为日语
    let english_text = "Hello";
    let configured = Some(Lang::new("ja"));

    // Act
    let (source, target) = resolve_direction(english_text, configured);

    // Assert
    assert_eq!(source.as_str(), "en", "源语言仍应检测为 en");
    assert_eq!(target.as_str(), "ja", "configured_target 应覆盖默认目标");
}

/// A02-c：map_lang_for_provider——zh/zh-CN/zh-Hans 经各 provider 映射表归一。
#[test]
fn lang_norm_deepl_maps_zh_variants_to_zh_uppercase() {
    // Arrange：DeepL 期望大写 "ZH"
    let zh = Lang::new("zh");
    let zh_cn = Lang::new("zh-CN");
    let zh_hans = Lang::new("zh-Hans");

    // Act + Assert：三种中文变体都应映射到 DeepL 的 "ZH"
    assert_eq!(map_lang_for_provider("deepl_free", &zh), "ZH");
    assert_eq!(map_lang_for_provider("deepl_free", &zh_cn), "ZH");
    assert_eq!(map_lang_for_provider("deepl_free", &zh_hans), "ZH");
}

#[test]
fn lang_norm_deepl_maps_en_to_en_uppercase() {
    // Arrange
    let en = Lang::new("en");

    // Act
    let result = map_lang_for_provider("deepl_free", &en);

    // Assert
    assert_eq!(result, "EN", "DeepL 英语代码应为大写 EN");
}

#[test]
fn lang_norm_mymemory_maps_zh_variants_to_zh_cn() {
    // Arrange：MyMemory 期望 "zh-CN"
    let zh = Lang::new("zh");
    let zh_hans = Lang::new("zh-Hans");

    // Act + Assert
    assert_eq!(map_lang_for_provider("mymemory", &zh), "zh-CN");
    assert_eq!(map_lang_for_provider("mymemory", &zh_hans), "zh-CN");
}

#[test]
fn lang_norm_baidu_maps_zh_variants_to_zh() {
    // Arrange：百度期望 "zh"
    let zh_cn = Lang::new("zh-CN");
    let zh_hans = Lang::new("zh-Hans");

    // Act + Assert
    assert_eq!(map_lang_for_provider("baidu", &zh_cn), "zh");
    assert_eq!(map_lang_for_provider("baidu", &zh_hans), "zh");
}

#[test]
fn lang_norm_google_maps_zh_variants_to_zh_cn() {
    // Arrange：Google 期望 "zh-CN"
    let zh = Lang::new("zh");
    let zh_hans = Lang::new("zh-Hans");

    // Act + Assert
    assert_eq!(map_lang_for_provider("google", &zh), "zh-CN");
    assert_eq!(map_lang_for_provider("google", &zh_hans), "zh-CN");
}

#[test]
fn lang_norm_unknown_provider_passes_through_lang_as_is() {
    // Arrange：未知 provider 应原样返回内部代码，不 panic
    let lang = Lang::new("fr");

    // Act
    let result = map_lang_for_provider("unknown_provider", &lang);

    // Assert
    assert_eq!(result, "fr", "未知 provider 应原样透传语言代码");
}

/// detect_lang 至少能区分中文与非中文。
#[test]
fn lang_norm_detect_lang_returns_zh_for_chinese() {
    // Arrange
    let text = "这是中文";

    // Act
    let lang = detect_lang(text);

    // Assert
    assert_eq!(lang.as_str(), "zh", "中文文本应检测为 zh");
}

#[test]
fn lang_norm_detect_lang_returns_en_for_ascii() {
    // Arrange
    let text = "This is English";

    // Act
    let lang = detect_lang(text);

    // Assert：ASCII 文本默认检测为 en
    assert_eq!(lang.as_str(), "en", "ASCII 文本应检测为 en");
}
