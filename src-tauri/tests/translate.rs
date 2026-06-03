//! 翻译框架集成测试
//!
//! 覆盖：
//! - V2-F1-A01 provider_trait_contract：薄 provider 契约三件职责可用，缓存/限流等不在 trait 上
//! - V2-F1-A02 lang_normalize_and_direction：语言归一，本地检测定方向，BCP-47，provider 映射表
//! - V2-F1-A03 error_enum_mapping：统一错误枚举，provider 原始错误归一到具体变体
//! - V2-F1-A04 same_source_retry_no_cross_failover：错误降级，同源退避重试，绝不自动跨源
//! - V2-F1-A05 credential_schema_keychain：provider 声明结构化字段 schema，secret→keychain，非密→加密 DB
//! - V2-F1-A07 timeout_and_cancel_inflight：超时归 Network，连续选中只认最新请求
//! - V2-F1-A08 static_registry_lists_four：静态注册表枚举 4 家 provider

use quickquick_lib::translate::cancel::InflightTracker;
use quickquick_lib::translate::error::{classify_timeout, map_provider_error};
use quickquick_lib::translate::lang::{
    detect_is_chinese, detect_lang, map_lang_for_provider, resolve_direction,
};
use quickquick_lib::translate::retry::{is_transient, retry_with_backoff};
use quickquick_lib::translate::{
    registry, Lang, ProviderCapability, ProviderHttpRequest, TranslateError, TranslateProvider,
    TranslateRequest, TranslateResponse,
};

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
            method: "GET",
            url: format!(
                "https://stub.example.com/translate?q={}&source={}&target={}",
                req.text,
                req.source_lang.as_str(),
                req.target_lang.as_str()
            ),
            body: None,
            headers: vec![],
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
        assert!(cap.needs_key, "provider '{}' 应为 needs_key=true", cap.id);
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

// V2-F1-A03 error_enum_mapping

/// A03：401/403 HTTP 状态码归一为 Auth 变体。
#[test]
fn error_enum_mapping_401_maps_to_auth() {
    // Arrange
    let status = 401_u16;

    // Act
    let err = map_provider_error(status, None);

    // Assert
    assert!(matches!(err, TranslateError::Auth(_)), "401 应归一为 Auth");
}

/// A03：403 归一为 Auth。
#[test]
fn error_enum_mapping_403_maps_to_auth() {
    // Arrange
    let status = 403_u16;

    // Act
    let err = map_provider_error(status, None);

    // Assert
    assert!(matches!(err, TranslateError::Auth(_)), "403 应归一为 Auth");
}

/// A03：429 归一为 RateLimit。
#[test]
fn error_enum_mapping_429_maps_to_rate_limit() {
    // Arrange
    let status = 429_u16;

    // Act
    let err = map_provider_error(status, None);

    // Assert
    assert!(
        matches!(err, TranslateError::RateLimit(_)),
        "429 应归一为 RateLimit"
    );
}

/// A03：5xx 归一为 ServerError。
#[test]
fn error_enum_mapping_5xx_maps_to_server_error() {
    // Arrange
    let status = 500_u16;

    // Act
    let err = map_provider_error(status, None);

    // Assert
    assert!(
        matches!(err, TranslateError::ServerError(_)),
        "500 应归一为 ServerError"
    );
}

/// A03：provider_code "quota_exceeded" 归一为 Quota。
#[test]
fn error_enum_mapping_quota_code_maps_to_quota() {
    // Arrange：200 状态码但带配额超限 provider_code
    let status = 200_u16;

    // Act
    let err = map_provider_error(status, Some("quota_exceeded"));

    // Assert
    assert!(
        matches!(err, TranslateError::Quota(_)),
        "quota_exceeded 应归一为 Quota"
    );
}

/// A03：provider_code "text_too_long" 归一为 TooLong。
#[test]
fn error_enum_mapping_too_long_code_maps_to_too_long() {
    // Arrange
    let status = 400_u16;

    // Act
    let err = map_provider_error(status, Some("text_too_long"));

    // Assert
    assert!(
        matches!(err, TranslateError::TooLong(_)),
        "text_too_long 应归一为 TooLong"
    );
}

/// A03：provider_code "unsupported_lang" 归一为 Unsupported。
#[test]
fn error_enum_mapping_unsupported_lang_maps_to_unsupported() {
    // Arrange
    let status = 400_u16;

    // Act
    let err = map_provider_error(status, Some("unsupported_lang"));

    // Assert
    assert!(
        matches!(err, TranslateError::Unsupported(_)),
        "unsupported_lang 应归一为 Unsupported"
    );
}

/// A03：status=0（网络层失败）归一为 Network。
#[test]
fn error_enum_mapping_status_zero_maps_to_network() {
    // Arrange：0 表示未收到 HTTP 响应（网络层失败）
    let status = 0_u16;

    // Act
    let err = map_provider_error(status, None);

    // Assert
    assert!(
        matches!(err, TranslateError::Network(_)),
        "status=0 应归一为 Network"
    );
}

/// A03（P3 边界）：provider_code "quota_remaining" 含子串 "quota" 但不在已知集合中，不应误判为 Quota。
#[test]
fn error_enum_mapping_quota_remaining_does_not_map_to_quota() {
    // Arrange：quota_remaining 表示"剩余配额"，语义与超限相反，不应触发 Quota 变体
    let status = 200_u16;

    // Act
    let err = map_provider_error(status, Some("quota_remaining"));

    // Assert：应回落到 HTTP 状态码兜底（200 → ServerError），而非误判为 Quota
    assert!(
        !matches!(err, TranslateError::Quota(_)),
        "quota_remaining 不应误命中 Quota 变体"
    );
}

/// A03（P3 边界）：provider_code "unsupported_format" 含子串 "unsupported" 但不在已知集合中，不应误判为 Unsupported。
#[test]
fn error_enum_mapping_unsupported_format_does_not_map_to_unsupported() {
    // Arrange：unsupported_format 是格式不支持，与语言不支持语义不同
    let status = 400_u16;

    // Act
    let err = map_provider_error(status, Some("unsupported_format"));

    // Assert：应回落到 HTTP 状态码兜底（400 → ServerError），而非误判为 Unsupported
    assert!(
        !matches!(err, TranslateError::Unsupported(_)),
        "unsupported_format 不应误命中 Unsupported 变体"
    );
}

// V2-F1-A04 same_source_retry_no_cross_failover

/// A04：is_transient 对瞬时错误返回 true。
#[test]
fn retry_policy_network_error_is_transient() {
    // Arrange
    let err = TranslateError::Network("连接超时".to_string());

    // Act + Assert
    assert!(is_transient(&err), "Network 错误应为瞬时可重试");
}

/// A04：is_transient 对 RateLimit 返回 true。
#[test]
fn retry_policy_rate_limit_is_transient() {
    // Arrange
    let err = TranslateError::RateLimit("请求频率过高".to_string());

    // Act + Assert
    assert!(is_transient(&err), "RateLimit 应为瞬时可重试");
}

/// A04：is_transient 对 ServerError 返回 true。
#[test]
fn retry_policy_server_error_is_transient() {
    // Arrange
    let err = TranslateError::ServerError("服务器内部错误".to_string());

    // Act + Assert
    assert!(is_transient(&err), "ServerError 应为瞬时可重试");
}

/// A04：is_transient 对永久错误 Auth 返回 false。
#[test]
fn retry_policy_auth_error_is_not_transient() {
    // Arrange
    let err = TranslateError::Auth("API Key 无效".to_string());

    // Act + Assert
    assert!(!is_transient(&err), "Auth 错误应为永久不重试");
}

/// A04：is_transient 对 Quota 返回 false。
#[test]
fn retry_policy_quota_error_is_not_transient() {
    // Arrange
    let err = TranslateError::Quota("配额已耗尽".to_string());

    // Act + Assert
    assert!(!is_transient(&err), "Quota 错误应为永久不重试");
}

/// A04：瞬时错误前 2 次失败，第 3 次成功——同源重试成功，provider_id 全程不变（无跨源切换），
/// sleep_fn 被调用正确次数且传入指数退避值（验证退避真实生效）。
#[test]
fn retry_policy_same_source_retry_no_cross_failover_succeeds_on_third_attempt() {
    // Arrange：可编程 fake op，前 2 次返回 Network 错误，第 3 次成功
    let attempt_count = std::cell::Cell::new(0_u32);
    let provider_ids_seen = std::cell::RefCell::new(Vec::<&str>::new());
    let fixed_provider_id = "mymemory";

    let sleep_calls = std::cell::RefCell::new(Vec::<u64>::new());

    let op = || {
        // 每次调用记录当前 provider_id——全程必须保持同一个（框架不切换）
        provider_ids_seen.borrow_mut().push(fixed_provider_id);
        let n = attempt_count.get();
        attempt_count.set(n + 1);
        if n < 2 {
            Err(TranslateError::Network("抖动".to_string()))
        } else {
            Ok("translated".to_string())
        }
    };

    // Act
    let result = retry_with_backoff(3, op, |ms| sleep_calls.borrow_mut().push(ms));

    // Assert：第 3 次成功
    assert!(result.is_ok(), "前 2 次瞬时错误后第 3 次应成功");
    assert_eq!(result.unwrap(), "translated");
    assert_eq!(attempt_count.get(), 3, "应恰好调用 3 次");

    // provider_id 全程不变——框架绝不自动切换 provider
    let ids = provider_ids_seen.borrow();
    assert_eq!(ids.len(), 3, "op 应被调用 3 次");
    assert!(
        ids.iter().all(|&id| id == fixed_provider_id),
        "全部 3 次调用均应使用同一 provider_id，实际: {ids:?}"
    );

    // sleep_fn 被调用 2 次（前 2 次失败各触发一次退避），且退避值符合指数序列
    let sleeps = sleep_calls.borrow();
    assert_eq!(
        sleeps.len(),
        2,
        "应退避 2 次（对应 2 次瞬时失败），实际: {sleeps:?}"
    );
    assert_eq!(sleeps[0], 500, "第 1 次退避应为 500ms，实际: {}", sleeps[0]);
    assert_eq!(
        sleeps[1], 1000,
        "第 2 次退避应为 1000ms，实际: {}",
        sleeps[1]
    );
}

/// A04：永久错误（Auth）立即返回，不触发重试，sleep_fn 不被调用。
#[test]
fn retry_policy_permanent_error_returns_immediately_without_retry() {
    // Arrange：每次都返回 Auth 错误
    let attempt_count = std::cell::Cell::new(0_u32);
    let sleep_calls = std::cell::Cell::new(0_u32);

    let op = || {
        attempt_count.set(attempt_count.get() + 1);
        Err::<String, _>(TranslateError::Auth("无效 Key".to_string()))
    };

    // Act
    let result = retry_with_backoff(3, op, |_| sleep_calls.set(sleep_calls.get() + 1));

    // Assert：永久错误立即返回，不重试，sleep_fn 不被调用
    assert!(result.is_err(), "永久错误应返回 Err");
    assert!(matches!(result.unwrap_err(), TranslateError::Auth(_)));
    assert_eq!(attempt_count.get(), 1, "永久错误应只调用 1 次，不重试");
    assert_eq!(sleep_calls.get(), 0, "永久错误不应触发退避 sleep");
}

// V2-F1-A07 timeout_and_cancel_inflight

/// A07：classify_timeout 返回 Network 变体（超时语义归入 network 错误枚举）。
#[test]
fn timeout_and_cancel_inflight_timeout_classified_as_network() {
    // Arrange + Act
    let err = classify_timeout();

    // Assert：超时驱动 network 错误枚举
    assert!(
        matches!(err, TranslateError::Network(_)),
        "超时应归一为 Network 错误"
    );
}

/// A07：InflightTracker 连续两次 begin 后，旧 generation is_current=false，新 generation=true。
#[test]
fn timeout_and_cancel_inflight_old_gen_invalidated_by_new_begin() {
    // Arrange
    let tracker = InflightTracker::new();

    // Act：第一次发请求
    let old_gen = tracker.begin();

    // Act：连续第二次发请求（模拟"选中即译"新触发）
    let new_gen = tracker.begin();

    // Assert：只认最新，旧请求应被取消
    assert!(!tracker.is_current(old_gen), "旧 generation 应已失效");
    assert!(
        tracker.is_current(new_gen),
        "新 generation 应为当前有效请求"
    );
}

/// A07：单次 begin 后，该 generation 仍为 current（未被后续 begin 覆盖）。
#[test]
fn timeout_and_cancel_inflight_single_begin_remains_current() {
    // Arrange
    let tracker = InflightTracker::new();

    // Act
    let gen = tracker.begin();

    // Assert
    assert!(tracker.is_current(gen), "唯一的 generation 应为 current");
}

// V2-F1-A05 credential_schema_keychain

use quickquick_lib::db;
use quickquick_lib::translate::credential::{
    credential_schema, load_credentials, save_credentials, CredError, CredStore,
};
use std::collections::HashMap;
use std::sync::Mutex;
use tempfile::tempdir;

const TEST_DB_KEY: [u8; 32] = [42u8; 32];

/// 测试用内存 CredStore——不触碰 OS keychain，headless，线程安全。
struct MockCredStore {
    inner: Mutex<HashMap<String, String>>,
}

impl MockCredStore {
    fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// 直接查询内部存储（用于路由正确性负向断言）
    fn contains_key(&self, provider_id: &str, field_key: &str) -> bool {
        let k = format!("{provider_id}.{field_key}");
        self.inner.lock().unwrap().contains_key(&k)
    }
}

impl CredStore for MockCredStore {
    fn set_secret(&self, provider_id: &str, field_key: &str, value: &str) -> Result<(), CredError> {
        let k = format!("{provider_id}.{field_key}");
        self.inner.lock().unwrap().insert(k, value.to_string());
        Ok(())
    }

    fn get_secret(&self, provider_id: &str, field_key: &str) -> Result<Option<String>, CredError> {
        let k = format!("{provider_id}.{field_key}");
        Ok(self.inner.lock().unwrap().get(&k).cloned())
    }

    fn delete_secret(&self, provider_id: &str, field_key: &str) -> Result<(), CredError> {
        let k = format!("{provider_id}.{field_key}");
        self.inner.lock().unwrap().remove(&k);
        Ok(())
    }
}

/// A05-a：百度 schema 含 app_id(非密必填) + secret_key(密必填)
#[test]
fn credential_schema_keychain_baidu_has_app_id_and_secret_key() {
    // Arrange + Act
    let fields = credential_schema("baidu");

    // Assert：必须含 app_id
    let app_id = fields.iter().find(|f| f.key == "app_id");
    assert!(app_id.is_some(), "百度 schema 应含 app_id 字段");
    let app_id = app_id.unwrap();
    assert!(!app_id.is_secret, "app_id 应为非密字段");
    assert!(app_id.required, "app_id 应为必填");

    // Assert：必须含 secret_key
    let secret_key = fields.iter().find(|f| f.key == "secret_key");
    assert!(secret_key.is_some(), "百度 schema 应含 secret_key 字段");
    let secret_key = secret_key.unwrap();
    assert!(secret_key.is_secret, "secret_key 应为 secret 字段");
    assert!(secret_key.required, "secret_key 应为必填");
}

/// A05-b：DeepL schema 含 auth_key(密必填)
#[test]
fn credential_schema_keychain_deepl_has_auth_key() {
    // Arrange + Act
    let fields = credential_schema("deepl_free");

    // Assert
    let auth_key = fields.iter().find(|f| f.key == "auth_key");
    assert!(auth_key.is_some(), "DeepL schema 应含 auth_key");
    let auth_key = auth_key.unwrap();
    assert!(auth_key.is_secret, "auth_key 应为 secret");
    assert!(auth_key.required, "auth_key 应为必填");
}

/// A05-c：Google schema 含 api_key(密必填)
#[test]
fn credential_schema_keychain_google_has_api_key() {
    // Arrange + Act
    let fields = credential_schema("google");

    // Assert
    let api_key = fields.iter().find(|f| f.key == "api_key");
    assert!(api_key.is_some(), "Google schema 应含 api_key");
    let api_key = api_key.unwrap();
    assert!(api_key.is_secret, "api_key 应为 secret");
    assert!(api_key.required, "api_key 应为必填");
}

/// A05-d：MyMemory schema 含 email(非密可选)
#[test]
fn credential_schema_keychain_mymemory_has_optional_email() {
    // Arrange + Act
    let fields = credential_schema("mymemory");

    // Assert
    let email = fields.iter().find(|f| f.key == "email");
    assert!(email.is_some(), "MyMemory schema 应含 email");
    let email = email.unwrap();
    assert!(!email.is_secret, "email 应为非密字段");
    assert!(!email.required, "email 应为可选");
}

/// A05-e：save 百度凭据后，secret_key 存 MockCredStore 可读回，app_id 存 DB 可读回；
///         路由正确：secret_key 不在 DB，app_id 不在 store（不触碰 OS keychain）。
#[test]
fn credential_schema_keychain_secret_routes_to_keychain_non_secret_routes_to_db() {
    // Arrange
    let store = MockCredStore::new();
    let dir = tempdir().expect("tempdir 应成功");
    let db_path = dir.path().join("cred_test.db");
    let conn = db::open_or_create(&db_path, &TEST_DB_KEY).expect("建库应成功");

    let values = vec![("app_id", "my_app_123"), ("secret_key", "super_secret_456")];

    // Act
    save_credentials("baidu", &values, &store, &conn).expect("save_credentials 应成功");

    // Assert：secret_key 存在 store（可读回且值一致）
    let kc_val = store
        .get_secret("baidu", "secret_key")
        .expect("get_secret 不应返回 Err");
    assert_eq!(
        kc_val,
        Some("super_secret_456".to_string()),
        "store 中 secret_key 值应与写入一致"
    );

    // Assert：app_id 存在 DB（可读回）
    let db_val: String = conn
        .query_row(
            "SELECT value FROM provider_config WHERE provider_id = 'baidu' AND field_key = 'app_id'",
            [],
            |row| row.get(0),
        )
        .expect("DB 中应能读回 app_id");
    assert_eq!(db_val, "my_app_123", "DB 中 app_id 值应与写入一致");

    // Assert：路由正确——secret_key 不在 DB
    let secret_in_db: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM provider_config WHERE provider_id = 'baidu' AND field_key = 'secret_key'",
            [],
            |row| row.get(0),
        )
        .expect("查询应成功");
    assert_eq!(secret_in_db, 0, "secret_key 绝不应写入 DB（路由正确性）");

    // Assert：路由正确——app_id 不在 store
    assert!(
        !store.contains_key("baidu", "app_id"),
        "app_id 不应写入 store（路由正确性）"
    );
}

/// A05-f：load_credentials 读回 save 写入的完整凭据（store + DB 均能读回）
#[test]
fn credential_schema_keychain_load_returns_saved_values() {
    // Arrange
    let store = MockCredStore::new();
    let dir = tempdir().expect("tempdir 应成功");
    let db_path = dir.path().join("cred_load_test.db");
    let conn = db::open_or_create(&db_path, &TEST_DB_KEY).expect("建库应成功");

    let values = vec![("app_id", "load_app_id"), ("secret_key", "load_secret")];
    save_credentials("baidu", &values, &store, &conn).expect("save 应成功");

    // Act
    let loaded = load_credentials("baidu", &store, &conn).expect("load_credentials 应成功");

    // Assert：两个字段均能读回
    let app_id = loaded.iter().find(|(k, _)| k == "app_id");
    assert!(app_id.is_some(), "load 应包含 app_id");
    assert_eq!(app_id.unwrap().1, "load_app_id");

    let secret_key = loaded.iter().find(|(k, _)| k == "secret_key");
    assert!(secret_key.is_some(), "load 应包含 secret_key");
    assert_eq!(secret_key.unwrap().1, "load_secret");
}

// A05 负向用例（对应 I-1/I-3 修复）

/// A05-g：未知 provider_id 调 save_credentials 应返回 UnknownProvider 错误，而非静默写入。
#[test]
fn credential_schema_keychain_unknown_provider_returns_err() {
    // Arrange
    let store = MockCredStore::new();
    let dir = tempdir().expect("tempdir 应成功");
    let db_path = dir.path().join("cred_neg_provider.db");
    let conn = db::open_or_create(&db_path, &TEST_DB_KEY).expect("建库应成功");

    let values = vec![("some_field", "some_value")];

    // Act
    let result = save_credentials("nonexistent_provider", &values, &store, &conn);

    // Assert：应返回错误，不静默成功
    assert!(result.is_err(), "未知 provider 应返回 Err");
    assert!(
        matches!(result.unwrap_err(), CredError::UnknownProvider(_)),
        "应为 UnknownProvider 变体"
    );
}

// V2-F1-A06 cache_key_includes_provider_lru

use quickquick_lib::translate::cache::{
    cache_get, cache_get_at, cache_key, cache_put, cache_put_at, CacheEntry,
};

/// I-1：空段（source_lang=""）与非空段不发生前缀碰撞（非零分隔符生效）。
#[test]
fn cache_key_separator_empty_segment_differs_from_nonempty() {
    // Arrange：仅 source_lang 不同（"" vs "x"），其余三段相同
    // 若分隔符为 XOR 0（无效），哈希状态不变，两者可能碰撞
    let key_empty = cache_key("a", "", "c", "d");
    let key_nonempty = cache_key("a", "x", "c", "d");

    // Act + Assert：非零分隔符使段边界真正改变哈希状态，两者必须不同
    assert_ne!(
        key_empty, key_nonempty,
        "空段与非空段应产生不同 cache_key（非零分隔符防前缀碰撞）"
    );
}

/// I-2：cache_get_at 命中时用注入的 now_ms 刷新 last_used_utc（LRU 刷新路径可测）。
#[test]
fn cache_get_at_hit_refreshes_last_used_utc_to_injected_timestamp() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("cache_get_at.db");
    let conn = db::open_or_create(&db_path, &TEST_DB_KEY).expect("建库应成功");

    let provider = "mymemory";
    let key = cache_key("hello", "en", "zh", provider);

    // 写入时 last_used_utc=100
    cache_put_at(
        &conn,
        &CacheEntry {
            source_text: "hello",
            source_lang: "en",
            target_lang: "zh",
            provider_id: provider,
            translated: "你好",
        },
        100,
        100,
    )
    .expect("put 应成功");

    // Act：用 now_ms=500 调用 cache_get_at，命中后应将 last_used_utc 刷新为 500
    let result = cache_get_at(&conn, &key, 500).expect("cache_get_at 不应返回 Err");

    // Assert：命中且返回正确值
    assert_eq!(
        result,
        Some("你好".to_string()),
        "cache_get_at 应命中并返回正确 translated"
    );

    // Assert：直查 DB 确认 last_used_utc 已刷新为 500（证明 LRU 刷新路径确实执行）
    let last_used: i64 = conn
        .query_row(
            "SELECT last_used_utc FROM translation_cache WHERE cache_key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .expect("直查 last_used_utc 应成功");
    assert_eq!(
        last_used, 500,
        "cache_get_at 命中后 last_used_utc 应被刷新为注入的 now_ms=500，实际: {last_used}"
    );
}

/// A06-a：同 source_text/源语/目标语，provider 不同 → cache_key 不同（换源必 miss）。
#[test]
fn cache_key_includes_provider_lru_different_providers_produce_different_keys() {
    // Arrange
    let text = "hello world";
    let src = "en";
    let tgt = "zh";

    // Act
    let key_mymemory = cache_key(text, src, tgt, "mymemory");
    let key_deepl = cache_key(text, src, tgt, "deepl_free");

    // Assert：换源必产生不同 key
    assert_ne!(
        key_mymemory, key_deepl,
        "provider 不同时 cache_key 必须不同，否则换源不会 miss"
    );
}

/// A06-b：put(mymemory) 后用 deepl_free 的 key cache_get → None（换源必 miss）。
#[test]
fn cache_key_includes_provider_lru_cross_provider_cache_get_is_miss() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("cache_miss.db");
    let conn = db::open_or_create(&db_path, &TEST_DB_KEY).expect("建库应成功");

    let text = "hello";
    let src = "en";
    let tgt = "zh";

    // Act：用 mymemory 写入
    cache_put(&conn, text, src, tgt, "mymemory", "你好", 100).expect("cache_put 应成功");

    // 用 deepl_free 的 key 查询
    let deepl_key = cache_key(text, src, tgt, "deepl_free");
    let result = cache_get(&conn, &deepl_key).expect("cache_get 不应返回 Err");

    // Assert：换源必 miss
    assert!(
        result.is_none(),
        "换 provider 后 cache_get 应返回 None（换源必 miss），实际: {:?}",
        result
    );
}

/// A06-c：put 后 cache_get 命中，返回正确 translated（持久验证）。
#[test]
fn cache_key_includes_provider_lru_put_then_get_hits_with_correct_value() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("cache_hit.db");
    let conn = db::open_or_create(&db_path, &TEST_DB_KEY).expect("建库应成功");

    let text = "good morning";
    let src = "en";
    let tgt = "zh";
    let provider = "mymemory";
    let translated = "早上好";

    // Act
    cache_put(&conn, text, src, tgt, provider, translated, 100).expect("cache_put 应成功");

    let key = cache_key(text, src, tgt, provider);
    let result = cache_get(&conn, &key).expect("cache_get 不应返回 Err");

    // Assert：命中且值正确
    assert_eq!(
        result,
        Some(translated.to_string()),
        "cache_get 应命中并返回正确 translated"
    );
}

/// A06-d：LRU 淘汰——capacity=2，put 三条，访问顺序使 B 最久未用；
///         第三次 put C 触发淘汰，B 被淘汰，A/C 仍在。
///
/// 时间线（注入可控时间戳，避免同毫秒内顺序不确定）：
///   t=100  put A（last_used=100）
///   t=200  put B（last_used=200）
///   t=300  get A → A.last_used 刷新为 300，B 此时 last_used=200（最旧，成为 LRU）
///   t=400  put C（capacity=2，触发淘汰：删 B）
#[test]
fn cache_key_includes_provider_lru_evicts_least_recently_used_on_overflow() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("cache_lru.db");
    let conn = db::open_or_create(&db_path, &TEST_DB_KEY).expect("建库应成功");

    let provider = "mymemory";
    let capacity = 2_usize;

    // t=100：put A（初始 last_used=100）
    cache_put_at(
        &conn,
        &CacheEntry {
            source_text: "apple",
            source_lang: "en",
            target_lang: "zh",
            provider_id: provider,
            translated: "苹果",
        },
        capacity,
        100,
    )
    .expect("put A 应成功");

    // t=200：put B（last_used=200）；此时表内 2 条，未超 capacity，不淘汰
    cache_put_at(
        &conn,
        &CacheEntry {
            source_text: "banana",
            source_lang: "en",
            target_lang: "zh",
            provider_id: provider,
            translated: "香蕉",
        },
        capacity,
        200,
    )
    .expect("put B 应成功");

    // t=300：刷新 A 的 last_used（UPSERT 同值、时间戳=300）
    // cache_get 内部用 current_utc_ms() 刷新，此处用 put_at 注入确定性时间戳
    cache_put_at(
        &conn,
        &CacheEntry {
            source_text: "apple",
            source_lang: "en",
            target_lang: "zh",
            provider_id: provider,
            translated: "苹果",
        },
        999,
        300,
    )
    .expect("刷新 A 的 last_used 应成功");

    // 此时：A.last_used=300，B.last_used=200 → B 是 LRU

    // t=400：put C，capacity=2，触发淘汰，应删 B（last_used 最旧=200）
    cache_put_at(
        &conn,
        &CacheEntry {
            source_text: "cherry",
            source_lang: "en",
            target_lang: "zh",
            provider_id: provider,
            translated: "樱桃",
        },
        capacity,
        400,
    )
    .expect("put C 应成功");

    // Assert：B 已被淘汰（LRU，last_used=200 最旧）
    let key_b = cache_key("banana", "en", "zh", provider);
    let b_result = cache_get(&conn, &key_b).expect("get B 不应返回 Err");
    assert!(
        b_result.is_none(),
        "B 应被 LRU 淘汰（last_used 最旧），实际: {:?}",
        b_result
    );

    // Assert：A 仍在（last_used=300，晚于 B）
    let key_a = cache_key("apple", "en", "zh", provider);
    let a_result = cache_get(&conn, &key_a).expect("get A 不应返回 Err");
    assert_eq!(
        a_result,
        Some("苹果".to_string()),
        "A last_used 已刷新，不应被淘汰"
    );

    // Assert：C 仍在（last_used=400，最新）
    let key_c = cache_key("cherry", "en", "zh", provider);
    let c_result = cache_get(&conn, &key_c).expect("get C 不应返回 Err");
    assert_eq!(
        c_result,
        Some("樱桃".to_string()),
        "C 为最新写入，不应被淘汰"
    );
}

/// A05-h：百度 provider 传未在 schema 中的 field_key 应返回 UnknownField 错误，
///         且该值未写入 DB（证明不静默降级）。
#[test]
fn credential_schema_keychain_unknown_field_returns_err_and_does_not_write_db() {
    // Arrange
    let store = MockCredStore::new();
    let dir = tempdir().expect("tempdir 应成功");
    let db_path = dir.path().join("cred_neg_field.db");
    let conn = db::open_or_create(&db_path, &TEST_DB_KEY).expect("建库应成功");

    // "secret_keys" 不在百度 schema（正确 key 为 "secret_key"，单数）
    let values = vec![("secret_keys", "typo_value")];

    // Act
    let result = save_credentials("baidu", &values, &store, &conn);

    // Assert：应返回错误
    assert!(result.is_err(), "未知 field_key 应返回 Err");
    assert!(
        matches!(result.unwrap_err(), CredError::UnknownField { .. }),
        "应为 UnknownField 变体"
    );

    // Assert：值未写入 DB（不静默降级）
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM provider_config WHERE provider_id = 'baidu' AND field_key = 'secret_keys'",
            [],
            |row| row.get(0),
        )
        .expect("查询应成功");
    assert_eq!(count, 0, "未知 field_key 的值不应写入 DB（不静默降级）");
}

// V2-F3-A13 smart_direction

/// A13-a：英文输入 → 自动识别为 en，默认目标 zh（非中文→中文方向）。
#[test]
fn smart_direction_english_input_resolves_to_en_zh() {
    // Arrange
    let text = "Hello, world!";

    // Act
    let (source, target) = resolve_direction(text, None);

    // Assert
    assert_eq!(source.as_str(), "en", "英文输入源语言应为 en");
    assert_eq!(
        target.as_str(),
        "zh",
        "英文输入默认目标应为 zh（自动识别→默认中文）"
    );
}

/// A13-b：中文输入 → 识别为 zh，默认目标 en（本是中文→翻英文）。
#[test]
fn smart_direction_chinese_input_resolves_to_zh_en() {
    // Arrange
    let text = "今天天气真好";

    // Act
    let (source, target) = resolve_direction(text, None);

    // Assert
    assert_eq!(source.as_str(), "zh", "中文输入源语言应为 zh");
    assert_eq!(
        target.as_str(),
        "en",
        "中文输入默认目标应为 en（本是中文→翻英文）"
    );
}

/// A13-c：configured_target=Some(ja) 时，不论输入语言，目标语强制为 ja（目标语可配）。
#[test]
fn smart_direction_configured_target_ja_overrides_default() {
    // Arrange：英文输入，但用户指定目标为日语
    let text = "Good morning";
    let configured = Some(Lang::new("ja"));

    // Act
    let (source, target) = resolve_direction(text, configured);

    // Assert
    assert_eq!(source.as_str(), "en", "源语言仍应检测为 en");
    assert_eq!(target.as_str(), "ja", "configured_target=ja 应覆盖默认目标");
}

/// A13-d：非恒真：中文输入 + configured_target=Some(fr) → 目标 fr（验证 configured 优先于 default）。
#[test]
fn smart_direction_chinese_input_with_configured_fr_resolves_to_zh_fr() {
    // Arrange
    let text = "你好世界";
    let configured = Some(Lang::new("fr"));

    // Act
    let (source, target) = resolve_direction(text, configured);

    // Assert
    assert_eq!(source.as_str(), "zh", "中文输入源语言应为 zh");
    assert_eq!(
        target.as_str(),
        "fr",
        "configured_target=fr 应覆盖默认 en 目标"
    );
}

// V2-F3-A14 translate_history_separate

use quickquick_lib::translate::history::{
    add_translate_history, translate_clip_item, translate_history_count,
};

/// A14-a：translate_clip_item 将剪贴板条目写入 translate_history，
///         translate_history 有该条、clip_items 数量不变（两者分开存储、互不混入）。
#[test]
fn translate_history_separate_clip_item_writes_to_history_not_clip_items() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("th_separate.db");
    let conn = db::open_or_create(&db_path, &TEST_DB_KEY).expect("建库应成功");

    // 插入一条 clip_item
    let clip_id = uuid::Uuid::new_v4().to_string();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("系统时间应在 epoch 之后")
        .as_millis() as i64;
    conn.execute(
        "INSERT INTO clip_items (id, content, kind, created_utc, last_modified_utc, is_deleted)
         VALUES (?1, ?2, 'text', ?3, ?3, 0)",
        rusqlite::params![clip_id, "Hello, world!", now_ms],
    )
    .expect("插入 clip_item 应成功");

    // Act：一键翻译——剪贴板条目写入 translate_history
    let history_id = translate_clip_item(&conn, &clip_id, "你好，世界！", "en", "zh", "mymemory")
        .expect("translate_clip_item 应成功");

    // Assert：translate_history 有该条
    let history_count = translate_history_count(&conn).expect("translate_history_count 应成功");
    assert_eq!(history_count, 1, "translate_history 应有 1 条记录");

    // Assert：新增的历史条目 id 有效
    assert!(!history_id.is_empty(), "返回的 history_id 不应为空");

    // Assert：clip_items 数量不变（仍为 1，没有新增）
    let clip_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 0",
            [],
            |row| row.get(0),
        )
        .expect("查询 clip_items 应成功");
    assert_eq!(
        clip_count, 1,
        "translate_clip_item 不应改变 clip_items 的数量，两者分开存储"
    );
}

/// A14-b：add_translate_history 直接插入一条翻译历史，可查回（独立存储验证）。
#[test]
fn translate_history_separate_add_and_retrieve() {
    // Arrange
    let dir = tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("th_add.db");
    let conn = db::open_or_create(&db_path, &TEST_DB_KEY).expect("建库应成功");

    // Act
    let id = add_translate_history(&conn, "Good morning", "早上好", "en", "zh", "mymemory")
        .expect("add_translate_history 应成功");

    // Assert：可查回该条记录
    let count = translate_history_count(&conn).expect("translate_history_count 应成功");
    assert_eq!(count, 1, "add_translate_history 后应有 1 条记录");
    assert!(!id.is_empty(), "返回 id 不应为空");

    // Assert：clip_items 表无记录（与剪贴板历史完全分开）
    let clip_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM clip_items", [], |row| row.get(0))
        .expect("查询应成功");
    assert_eq!(clip_count, 0, "翻译历史不应写入 clip_items 表（独立存储）");
}
