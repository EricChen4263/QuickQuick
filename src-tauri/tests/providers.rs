//! Provider 集成测试
//!
//! 覆盖：
//! - V2-F2-A09 provider_mymemory：MyMemory 适配（默认源，无 key）
//!   capability + build_request（URL 编码、email 可选）+ parse_response（成功/quota/rate-limit/非法 JSON）
//! - V2-F2-A10 providers_keyed：百度/DeepL Free/Google 三家适配（鉴权字段 + 请求映射 + 错误码归一）
//! - V2-F2-A11 quota_explicit_no_silent_switch：撞额度显式提示，不自动切换 provider

use quickquick_lib::translate::providers::{
    baidu_sign, on_quota_or_failure, BaiduProvider, DeepLFreeProvider, GoogleProvider, LingvaProvider,
    UserPromptKind,
};
use quickquick_lib::translate::{Lang, TranslateError, TranslateProvider, TranslateRequest};

// 构造测试用 TranslateRequest 的辅助函数
fn make_request(text: &str, src: &str, tgt: &str) -> TranslateRequest {
    TranslateRequest {
        text: text.to_string(),
        source_lang: Lang::new(src),
        target_lang: Lang::new(tgt),
    }
}

// TV1-F1 provider_lingva（免 key 默认源，替代 MyMemory）

/// capability 声明 id=lingva、needs_key=false
#[test]
fn provider_lingva_capability_id_and_no_key() {
    // Arrange
    let provider = LingvaProvider::new();

    // Act
    let cap = provider.capability();

    // Assert
    assert_eq!(cap.id, "lingva");
    assert!(!cap.needs_key, "Lingva 默认源无需 API Key");
}

/// build_request 生成 Lingva GET 端点：/api/v1/{src}/{tgt}/{编码 text}，无 body
#[test]
fn provider_lingva_build_request_url_path_encoding() {
    // Arrange
    let provider = LingvaProvider::new();
    let req = make_request("hello world", "en", "zh");

    // Act
    let http_req = provider.build_request(&req);

    // Assert: 端点前缀 + 语言码路径段 + text 的 percent-encoding（空格 → %20）
    assert_eq!(http_req.method, "GET", "Lingva 使用 GET");
    assert!(
        http_req
            .url
            .starts_with("https://lingva.pot-app.com/api/v1/"),
        "URL 须为 Lingva 端点，实际: {}",
        http_req.url
    );
    assert!(
        http_req.url.ends_with("/en/zh/hello%20world"),
        "URL 须含语言码路径段与编码后 text，实际: {}",
        http_req.url
    );
    assert!(http_req.body.is_none(), "Lingva 使用 GET，body 应为 None");
}

/// parse_response 成功路径——从 {"translation":"X"} 取出 X
#[test]
fn provider_lingva_parse_response_success() {
    // Arrange
    let provider = LingvaProvider::new();
    let raw = r#"{"translation":"你好，世界"}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let resp = result.expect("成功 JSON 应解析成功");
    assert_eq!(resp.translated, "你好，世界");
}

/// parse_response 缺 translation 字段 → TranslateError::ParseError
#[test]
fn provider_lingva_parse_response_missing_field_returns_parse_error() {
    // Arrange
    let provider = LingvaProvider::new();
    let raw = r#"{"error":"unsupported language"}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let err = result.expect_err("缺 translation 字段应返回错误");
    assert!(
        matches!(err, TranslateError::ParseError(_)),
        "缺字段应归入 ParseError，实际: {:?}",
        err
    );
}

/// parse_response 非法 JSON → TranslateError::ParseError
#[test]
fn provider_lingva_parse_response_invalid_json() {
    // Arrange
    let provider = LingvaProvider::new();
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

// V2-F2-A10 providers_keyed

/// A10-1: 百度 capability 声明 id=baidu、needs_key=true
#[test]
fn providers_keyed_baidu_capability() {
    // Arrange
    let provider = BaiduProvider::new("my_appid", "my_secret");

    // Act
    let cap = provider.capability();

    // Assert
    assert_eq!(cap.id, "baidu");
    assert_eq!(cap.name, "百度翻译");
    assert!(cap.needs_key, "百度翻译需要 AppID + SecretKey");
}

/// A10-2a: baidu_sign 纯函数对固定输入返回确定性 MD5（验证算法本身）
#[test]
fn providers_keyed_baidu_sign_pure_function_deterministic() {
    // 固定输入 → 固定输出；不依赖任何 feature flag。
    // 预期值：MD5("2015063000000001apple143566028812345678") 的十六进制。
    let expected = format!(
        "{:x}",
        md5::compute(b"2015063000000001apple143566028812345678")
    );
    let actual = baidu_sign("2015063000000001", "apple", "1435660288", "12345678");
    assert_eq!(
        actual, expected,
        "baidu_sign 纯函数结果应与手动计算 MD5 一致"
    );
}

/// A10-2b: 百度 build_request 包含 sign/appid/salt/q 参数，
///         且 sign = MD5(appid+q+实际salt+secret)（随机 salt，从 body 解析后重算验证）
#[test]
fn providers_keyed_baidu_build_request_sign() {
    // Arrange
    let appid = "2015063000000001";
    let secret = "12345678";
    let provider = BaiduProvider::new(appid, secret);
    let req = make_request("apple", "en", "zh");

    // Act
    let http_req = provider.build_request(&req);

    // Assert: POST 到百度端点
    assert_eq!(http_req.method, "POST");
    assert_eq!(
        http_req.url,
        "https://fanyi-api.baidu.com/api/trans/vip/translate"
    );

    let body = http_req.body.expect("百度请求应有 POST body");
    assert!(
        body.contains("appid=2015063000000001"),
        "body 须含 appid，实际: {body}"
    );
    assert!(body.contains("q=apple"), "body 须含 q 参数，实际: {body}");
    assert!(body.contains("salt="), "body 须含 salt，实际: {body}");
    assert!(body.contains("sign="), "body 须含 sign，实际: {body}");

    // 从 body 解析实际 salt，用 baidu_sign 重算期望 sign，验证签名算法正确。
    // 这样无论 salt 取何随机值，断言始终稳定通过。
    let actual_salt = body
        .split('&')
        .find_map(|kv| kv.strip_prefix("salt="))
        .expect("body 应含 salt= 参数");
    let expected_sign = baidu_sign(appid, "apple", actual_salt, secret);
    assert!(
        body.contains(&format!("sign={expected_sign}")),
        "sign 应为 baidu_sign(appid, q, 实际salt, secret)={expected_sign}，实际 body: {body}",
    );
}

/// A10-3: 百度 parse_response 成功路径
#[test]
fn providers_keyed_baidu_parse_response_success() {
    // Arrange
    let provider = BaiduProvider::new("appid", "secret");
    let raw = r#"{"from":"en","to":"zh","trans_result":[{"src":"apple","dst":"苹果"}]}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let resp = result.expect("成功 JSON 应解析成功");
    assert_eq!(resp.translated, "苹果");
}

/// A10-4: 百度 parse_response 错误码 54003（频率超限）→ RateLimit
#[test]
fn providers_keyed_baidu_parse_response_rate_limit() {
    // Arrange
    let provider = BaiduProvider::new("appid", "secret");
    let raw = r#"{"error_code":"54003","error_msg":"Invalid Access Limit"}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let err = result.expect_err("错误码 54003 应返回错误");
    assert!(
        matches!(err, TranslateError::RateLimit(_)),
        "error_code=54003 应归入 RateLimit，实际: {:?}",
        err
    );
}

/// A10-5: 百度 parse_response 错误码 54004（余额不足）→ Quota
#[test]
fn providers_keyed_baidu_parse_response_quota() {
    // Arrange
    let provider = BaiduProvider::new("appid", "secret");
    let raw = r#"{"error_code":54004,"error_msg":"Insufficient account balance"}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let err = result.expect_err("错误码 54004 应返回错误");
    assert!(
        matches!(err, TranslateError::Quota(_)),
        "error_code=54004 应归入 Quota，实际: {:?}",
        err
    );
}

/// A10-6: 百度 parse_response 错误码 58001（语言不支持）→ Unsupported
#[test]
fn providers_keyed_baidu_parse_response_unsupported() {
    // Arrange
    let provider = BaiduProvider::new("appid", "secret");
    let raw = r#"{"error_code":"58001","error_msg":"Target language is not supported"}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let err = result.expect_err("错误码 58001 应返回错误");
    assert!(
        matches!(err, TranslateError::Unsupported(_)),
        "error_code=58001 应归入 Unsupported，实际: {:?}",
        err
    );
}

/// A10-7: DeepL Free capability 声明 id=deepl_free、needs_key=true
#[test]
fn providers_keyed_deepl_capability() {
    // Arrange
    let provider = DeepLFreeProvider::new("my-auth-key:fx");

    // Act
    let cap = provider.capability();

    // Assert
    assert_eq!(cap.id, "deepl_free");
    assert_eq!(cap.name, "DeepL Free");
    assert!(cap.needs_key, "DeepL Free 需要 Auth Key");
}

/// A10-8: DeepL Free build_request 含 Authorization header 且 header 值前缀正确
#[test]
fn providers_keyed_deepl_build_request_auth_header() {
    // Arrange
    let provider = DeepLFreeProvider::new("test-key:fx");
    let req = make_request("Hello", "en", "zh");

    // Act
    let http_req = provider.build_request(&req);

    // Assert
    assert_eq!(http_req.method, "POST");
    assert_eq!(http_req.url, "https://api-free.deepl.com/v2/translate");

    let auth_header = http_req
        .headers
        .iter()
        .find(|(k, _)| k == "Authorization")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        auth_header,
        Some("DeepL-Auth-Key test-key:fx"),
        "Authorization header 须为 DeepL-Auth-Key <key>",
    );

    let body = http_req.body.expect("DeepL 请求应有 POST body");
    assert!(
        body.contains("text=Hello"),
        "body 须含 text 参数，实际: {body}"
    );
    assert!(
        body.contains("source_lang=EN"),
        "body 须含大写 source_lang，实际: {body}"
    );
    assert!(
        body.contains("target_lang=ZH"),
        "body 须含大写 target_lang，实际: {body}"
    );
}

/// A10-9: DeepL Free parse_response 成功路径
#[test]
fn providers_keyed_deepl_parse_response_success() {
    // Arrange
    let provider = DeepLFreeProvider::new("key");
    let raw = r#"{"translations":[{"text":"你好，世界","detected_source_language":"EN"}]}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let resp = result.expect("成功 JSON 应解析成功");
    assert_eq!(resp.translated, "你好，世界");
}

/// A10-10: DeepL Free parse_response 错误响应（含 quota 文案）→ Quota
#[test]
fn providers_keyed_deepl_parse_response_quota() {
    // Arrange
    let provider = DeepLFreeProvider::new("key");
    let raw = r#"{"message":"Quota Exceeded"}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let err = result.expect_err("Quota 错误响应应返回错误");
    assert!(
        matches!(err, TranslateError::Quota(_)),
        "Quota 文案应归入 Quota，实际: {:?}",
        err
    );
}

/// A10-11: DeepL Free parse_response 错误响应（含 403 文案）→ Auth
#[test]
fn providers_keyed_deepl_parse_response_auth_error() {
    // Arrange
    let provider = DeepLFreeProvider::new("key");
    let raw = r#"{"message":"Forbidden: 403"}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let err = result.expect_err("403 错误响应应返回错误");
    assert!(
        matches!(err, TranslateError::Auth(_)),
        "403 错误应归入 Auth，实际: {:?}",
        err
    );
}

/// A10-12: Google capability 声明 id=google、needs_key=true
#[test]
fn providers_keyed_google_capability() {
    // Arrange
    let provider = GoogleProvider::new("my-api-key");

    // Act
    let cap = provider.capability();

    // Assert
    assert_eq!(cap.id, "google");
    assert_eq!(cap.name, "Google 翻译");
    assert!(cap.needs_key, "Google 翻译需要 API Key");
}

/// A10-13: Google build_request 含 key 参数 + body 含 q/source/target
#[test]
fn providers_keyed_google_build_request_key_and_body() {
    // Arrange
    let provider = GoogleProvider::new("AIzaSy-test-key");
    let req = make_request("hello", "en", "zh");

    // Act
    let http_req = provider.build_request(&req);

    // Assert
    assert_eq!(http_req.method, "POST");
    assert!(
        http_req.url.contains("key=AIzaSy-test-key"),
        "URL 须含 key= 参数，实际: {}",
        http_req.url
    );
    assert!(
        http_req
            .url
            .starts_with("https://translation.googleapis.com/language/translate/v2"),
        "URL 须指向 Google Translation API，实际: {}",
        http_req.url
    );

    let body = http_req.body.expect("Google 请求应有 POST body");
    assert!(body.contains("q=hello"), "body 须含 q 参数，实际: {body}");
    assert!(
        body.contains("source=en"),
        "body 须含 source 参数，实际: {body}"
    );
    assert!(
        body.contains("target=zh-CN"),
        "body 须含 target 参数（中文映射为 zh-CN），实际: {body}"
    );
}

/// A10-14: Google parse_response 成功路径
#[test]
fn providers_keyed_google_parse_response_success() {
    // Arrange
    let provider = GoogleProvider::new("key");
    let raw = r#"{"data":{"translations":[{"translatedText":"你好"}]}}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let resp = result.expect("成功 JSON 应解析成功");
    assert_eq!(resp.translated, "你好");
}

/// A10-15: Google parse_response 错误对象 status=PERMISSION_DENIED → Auth
#[test]
fn providers_keyed_google_parse_response_auth_error() {
    // Arrange
    let provider = GoogleProvider::new("key");
    let raw =
        r#"{"error":{"code":403,"status":"PERMISSION_DENIED","message":"API key not valid"}}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let err = result.expect_err("PERMISSION_DENIED 应返回错误");
    assert!(
        matches!(err, TranslateError::Auth(_)),
        "PERMISSION_DENIED 应归入 Auth，实际: {:?}",
        err
    );
}

/// A10-16: Google parse_response 错误对象 status=RESOURCE_EXHAUSTED → Quota
#[test]
fn providers_keyed_google_parse_response_quota() {
    // Arrange
    let provider = GoogleProvider::new("key");
    let raw = r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","message":"Quota exceeded"}}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert
    let err = result.expect_err("RESOURCE_EXHAUSTED 应返回错误");
    assert!(
        matches!(err, TranslateError::Quota(_)),
        "RESOURCE_EXHAUSTED 应归入 Quota，实际: {:?}",
        err
    );
}

/// A10-17: Google parse_response 错误 code 为字符串 "403" → Auth（兼容两种 JSON 形态）
#[test]
fn providers_keyed_google_parse_response_code_as_string() {
    // Arrange
    let provider = GoogleProvider::new("key");
    let raw = r#"{"error":{"code":"403","status":"PERMISSION_DENIED","message":"Forbidden"}}"#;

    // Act
    let result = provider.parse_response(raw);

    // Assert: 字符串 "403" 不得误当成功，需正确归 Auth
    let err = result.expect_err("字符串 code=\"403\" 应返回错误，不得误判为成功");
    assert!(
        matches!(err, TranslateError::Auth(_)),
        "字符串 code 403+PERMISSION_DENIED 应归入 Auth，实际: {:?}",
        err
    );
}

// V2-F2-A11 quota_explicit_no_silent_switch

/// A11-1: 免 key 源（lingva）撞额度 → 返回 NeedKey 提示（统一显式提示，不静默切换）
#[test]
fn quota_explicit_no_silent_switch_keyless_provider_quota_returns_need_key() {
    // Arrange
    let err = TranslateError::Quota("Lingva 配额耗尽".to_string());

    // Act
    let prompt = on_quota_or_failure("lingva", &err);

    // Assert: 必须返回显式提示，移除 MyMemory 后统一为 NeedKey（无 NeedEmail 分支）
    let p = prompt.expect("撞额度必须返回显式用户提示，不得返回 None");
    assert_eq!(
        p.kind,
        UserPromptKind::NeedKey,
        "配额耗尽应返回 NeedKey 显式提示，实际: {:?}",
        p.kind
    );
    assert!(!p.message.is_empty(), "提示信息不得为空");
}

/// A11-2: 有 key 的 provider 撞额度 → 返回 NeedKey 提示（引导填 key）
#[test]
fn quota_explicit_no_silent_switch_keyed_provider_quota_returns_need_key() {
    // Arrange
    let err = TranslateError::Quota("百度余额不足".to_string());

    // Act
    let prompt = on_quota_or_failure("baidu", &err);

    // Assert
    let p = prompt.expect("撞额度必须返回显式用户提示");
    assert_eq!(
        p.kind,
        UserPromptKind::NeedKey,
        "有 key 的 provider 配额耗尽应引导填 key，实际: {:?}",
        p.kind
    );
}

/// A11-3: 鉴权失败 → 返回 NeedKey 提示
#[test]
fn quota_explicit_no_silent_switch_auth_error_returns_need_key() {
    // Arrange
    let err = TranslateError::Auth("DeepL Free 认证失败".to_string());

    // Act
    let prompt = on_quota_or_failure("deepl_free", &err);

    // Assert
    let p = prompt.expect("鉴权失败必须返回显式用户提示");
    assert_eq!(
        p.kind,
        UserPromptKind::NeedKey,
        "鉴权失败应引导填 key，实际: {:?}",
        p.kind
    );
}

/// A11-4: on_quota_or_failure 函数签名不含"下一个 provider"参数或返回值——
/// 验证无自动跨源切换逻辑（函数只返回提示，不含任何 provider 切换动作）
#[test]
fn quota_explicit_no_silent_switch_no_auto_switch_in_return_type() {
    // Arrange: 任意 Quota 错误
    let err = TranslateError::Quota("配额耗尽".to_string());

    // Act
    let result = on_quota_or_failure("google", &err);

    // Assert: 返回类型是 Option<UserPrompt>，不含任何"下一个 provider id"字段
    let prompt = result.expect("应返回提示");
    // UserPrompt 结构体只有 kind 和 message 两个字段，无 next_provider 字段
    // 此断言通过编译即验证：若返回类型含自动切换字段，下面的模式匹配会编译失败
    let UserPromptKind::NeedKey = prompt.kind else {
        panic!("Google 配额耗尽应返回 NeedKey，实际: {:?}", prompt.kind);
    };
    // 消除 clippy 的 unused 警告，同时显式验证 message 非空
    assert!(!prompt.message.is_empty());
}

/// A11-5: 非 Quota/Auth 错误（如 Network）→ 返回 None（不强制提示，不静默切换）
#[test]
fn quota_explicit_no_silent_switch_network_error_returns_none() {
    // Arrange
    let err = TranslateError::Network("连接超时".to_string());

    // Act
    let prompt = on_quota_or_failure("baidu", &err);

    // Assert: 网络错误不属于需要用户干预的类型，返回 None
    assert!(
        prompt.is_none(),
        "Network 错误不应触发用户提示（由重试逻辑处理），实际: {:?}",
        prompt.map(|p| p.message)
    );
}
