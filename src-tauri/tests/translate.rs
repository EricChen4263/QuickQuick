//! 翻译框架集成测试
//!
//! 覆盖：
//! - V2-F1-A01 provider_trait_contract：薄 provider 契约三件职责可用，缓存/限流等不在 trait 上
//! - V2-F1-A02 lang_normalize_and_direction：语言归一，本地检测定方向，BCP-47，provider 映射表
//! - V2-F1-A03 error_enum_mapping：统一错误枚举，provider 原始错误归一到具体变体
//! - V2-F1-A04 same_source_retry_no_cross_failover：错误降级，同源退避重试，绝不自动跨源
//! - V2-F1-A07 timeout_and_cancel_inflight：超时归 Network，连续选中只认最新请求
//! - V2-F1-A08 static_registry_lists_four：静态注册表枚举 4 家 provider

use quickquick_lib::translate::{
    Lang, ProviderCapability, ProviderHttpRequest, TranslateError, TranslateProvider,
    TranslateRequest, TranslateResponse, registry,
};
use quickquick_lib::translate::lang::{detect_is_chinese, detect_lang, map_lang_for_provider, resolve_direction};
use quickquick_lib::translate::error::{map_provider_error, classify_timeout};
use quickquick_lib::translate::retry::{is_transient, retry_with_backoff};
use quickquick_lib::translate::cancel::InflightTracker;

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
    assert!(matches!(err, TranslateError::RateLimit(_)), "429 应归一为 RateLimit");
}

/// A03：5xx 归一为 ServerError。
#[test]
fn error_enum_mapping_5xx_maps_to_server_error() {
    // Arrange
    let status = 500_u16;

    // Act
    let err = map_provider_error(status, None);

    // Assert
    assert!(matches!(err, TranslateError::ServerError(_)), "500 应归一为 ServerError");
}

/// A03：provider_code "quota_exceeded" 归一为 Quota。
#[test]
fn error_enum_mapping_quota_code_maps_to_quota() {
    // Arrange：200 状态码但带配额超限 provider_code
    let status = 200_u16;

    // Act
    let err = map_provider_error(status, Some("quota_exceeded"));

    // Assert
    assert!(matches!(err, TranslateError::Quota(_)), "quota_exceeded 应归一为 Quota");
}

/// A03：provider_code "text_too_long" 归一为 TooLong。
#[test]
fn error_enum_mapping_too_long_code_maps_to_too_long() {
    // Arrange
    let status = 400_u16;

    // Act
    let err = map_provider_error(status, Some("text_too_long"));

    // Assert
    assert!(matches!(err, TranslateError::TooLong(_)), "text_too_long 应归一为 TooLong");
}

/// A03：provider_code "unsupported_lang" 归一为 Unsupported。
#[test]
fn error_enum_mapping_unsupported_lang_maps_to_unsupported() {
    // Arrange
    let status = 400_u16;

    // Act
    let err = map_provider_error(status, Some("unsupported_lang"));

    // Assert
    assert!(matches!(err, TranslateError::Unsupported(_)), "unsupported_lang 应归一为 Unsupported");
}

/// A03：status=0（网络层失败）归一为 Network。
#[test]
fn error_enum_mapping_status_zero_maps_to_network() {
    // Arrange：0 表示未收到 HTTP 响应（网络层失败）
    let status = 0_u16;

    // Act
    let err = map_provider_error(status, None);

    // Assert
    assert!(matches!(err, TranslateError::Network(_)), "status=0 应归一为 Network");
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
    assert_eq!(sleeps.len(), 2, "应退避 2 次（对应 2 次瞬时失败），实际: {sleeps:?}");
    assert_eq!(sleeps[0], 500, "第 1 次退避应为 500ms，实际: {}", sleeps[0]);
    assert_eq!(sleeps[1], 1000, "第 2 次退避应为 1000ms，实际: {}", sleeps[1]);
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
    assert!(matches!(err, TranslateError::Network(_)), "超时应归一为 Network 错误");
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
    assert!(tracker.is_current(new_gen), "新 generation 应为当前有效请求");
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
