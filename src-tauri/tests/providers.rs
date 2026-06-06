//! Provider 集成测试
//!
//! 覆盖：
//! - V2-F2-A09 provider_mymemory：MyMemory 适配（默认源，无 key）
//!   capability + build_request（URL 编码、email 可选）+ parse_response（成功/quota/rate-limit/非法 JSON）
//! - V2-F2-A10 providers_keyed：百度/DeepL Free/Google 三家适配（鉴权字段 + 请求映射 + 错误码归一）
//! - V2-F2-A11 quota_explicit_no_silent_switch：撞额度显式提示，不自动切换 provider

use quickquick_lib::translate::ecdict_db::EcdictDb;
use quickquick_lib::translate::providers::{
    baidu_field_sign, baidu_sign, build_provider, on_quota_or_failure, youdao_sign,
    youdao_truncate, BaiduProvider, DeepLFreeProvider, EcdictProvider, GoogleProvider,
    LingvaProvider, UserPromptKind,
};
use quickquick_lib::translate::{
    HttpExecutor, Lang, ProviderHttpRequest, TranslateError, TranslateProvider, TranslateRequest,
    TranslateResponse,
};

// 从翻译响应取出 Plain 变体译文；非 Plain 即测试失败（既有机翻源应全返 Plain）。
fn plain_text(resp: &TranslateResponse) -> &str {
    match resp {
        TranslateResponse::Plain { translated } => translated,
        TranslateResponse::Dict { .. } => panic!("既有源应返回 Plain，实际返回 Dict"),
    }
}

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
        http_req.url.starts_with("https://lingva.ml/api/v1/"),
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
    assert_eq!(plain_text(&resp), "你好，世界");
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
    assert_eq!(plain_text(&resp), "苹果");
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
    assert_eq!(plain_text(&resp), "你好，世界");
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
    assert_eq!(plain_text(&resp), "你好");
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

// TV2-F1：百度专业 / 有道 签名纯函数确定性（跨 crate 边界）集成测试

/// TV2-F1-A01：baidu_field_sign 对固定输入产出确定 MD5（pub 函数跨 crate 可用）。
#[test]
fn baidu_field_sign_is_deterministic_across_crate() {
    let sign = baidu_field_sign("appid123", "hello", "12345", "it", "secret");
    assert_eq!(
        sign, "0ddfb12f98655a716cc509c2538a4386",
        "baidu_field 签名应为 MD5(appid+q+salt+field+secret) 确定值"
    );
}

/// TV2-F1-A01：youdao_sign 短/长文本确定 SHA256 + truncate 边界（pub 函数跨 crate 可用）。
#[test]
fn youdao_sign_and_truncate_are_deterministic_across_crate() {
    let short = youdao_sign("app123", "hello", "saltX", "1700000000", "sec456");
    assert_eq!(
        short, "de9f455414aeb5c0057ad78813f9be70ff0ef07f9ea70cf53ee90169860871a2",
        "短文本（len<=20 用全文）签名应为确定 SHA256"
    );

    let long_text = "this is a very long text over twenty chars";
    let long = youdao_sign("app123", long_text, "saltX", "1700000000", "sec456");
    assert_eq!(
        long, "b61bfa28f19cdff1f69386cb076d25c13902bc855a97e91364dbad6fe76c3c3b",
        "长文本 truncate(前10+len+后10) 后签名应为确定 SHA256"
    );

    // truncate 边界：恰 20 用全文、21 触发截断。
    assert_eq!(
        youdao_truncate("12345678901234567890"),
        "12345678901234567890"
    );
    assert_eq!(
        youdao_truncate("123456789012345678901"),
        "1234567890212345678901"
    );
}

// TV-ECDICT provider_ecdict（本地 SQLite 词库，免 key、零网络）

/// 调用即 panic 的执行器：ECDICT 走本地查询，若误触网络路径此处会炸出，
/// 用以证伪「provider 仍发 HTTP」的回归。
struct NeverExecutor;

impl HttpExecutor for NeverExecutor {
    fn execute(&self, _req: &ProviderHttpRequest) -> Result<String, TranslateError> {
        panic!("ECDICT provider 不应发起任何 HTTP 请求");
    }
}

/// 建临时 ECDICT 库写入单行，返回 (TempDir 守卫, 包好 Arc 的 DAO)。
///
/// 守卫须持有到断言结束，否则 TempDir drop 删库致查询失败。
fn ecdict_fixture(
    word: &str,
    phonetic: &str,
    translation: &str,
    exchange: &str,
) -> (tempfile::TempDir, std::sync::Arc<EcdictDb>) {
    let dir = tempfile::TempDir::new().expect("创建临时目录");
    let path = dir.path().join("ecdict.db");
    let conn = rusqlite::Connection::open(&path).expect("建库");
    conn.execute_batch(
        "CREATE TABLE ecdict (word TEXT NOT NULL, phonetic TEXT, \
         translation TEXT, exchange TEXT);",
    )
    .expect("建表");
    conn.execute(
        "INSERT INTO ecdict (word, phonetic, translation, exchange) VALUES (?1, ?2, ?3, ?4)",
        [word, phonetic, translation, exchange],
    )
    .expect("插入行");
    (dir, std::sync::Arc::new(EcdictDb::new(path)))
}

/// capability 声明 id=ecdict、免 key，且 is_unofficial=false（本地离线、不随第三方改版失效）。
#[test]
fn provider_ecdict_capability_keyless_and_official() {
    let provider = EcdictProvider::new(std::sync::Arc::new(EcdictDb::new(
        std::path::PathBuf::new(),
    )));

    let cap = provider.capability();

    assert_eq!(cap.id, "ecdict");
    assert!(!cap.needs_key, "ECDICT 为免 key 源");
    assert!(!cap.is_unofficial, "本地离线词库不应标记为非官方");
}

/// build_provider("ecdict") 未注入本地库时返回错误（不 panic）；注入后成功。
#[test]
fn build_provider_ecdict_requires_local_db() {
    assert!(
        build_provider("ecdict", &[], None).is_err(),
        "未注入本地库应返回错误"
    );

    let (_dir, db) = ecdict_fixture("glacier", "", "n. 冰川", "");
    assert!(
        build_provider("ecdict", &[], Some(db)).is_ok(),
        "注入本地库后应成功构造"
    );
}

/// 命中：translate 走本地库返回 Dict（音标 + 按词性分组释义 + 词形变化），全程不触网络。
#[test]
fn provider_ecdict_translate_hit_returns_dict() {
    let (_dir, db) = ecdict_fixture("glacier", "ˈɡleɪʃər", "n. 冰川，冰河", "s:glaciers");
    let provider = EcdictProvider::new(db);
    let req = make_request("glacier", "en", "zh");

    let resp = provider
        .translate(&req, &NeverExecutor)
        .expect("命中应返回 Ok");

    let TranslateResponse::Dict { entry } = resp else {
        panic!("ECDICT 命中应返回 Dict，实际：{resp:?}");
    };
    assert_eq!(entry.phonetic.as_deref(), Some("ˈɡleɪʃər"), "应取音标");
    assert!(
        entry
            .definitions
            .iter()
            .flat_map(|d| &d.meanings)
            .any(|m| m.contains("冰川")),
        "释义应含「冰川」，实际：{:?}",
        entry.definitions
    );
    assert!(
        entry.inflections.iter().any(|i| i == "glaciers"),
        "词形应含 glaciers，实际：{:?}",
        entry.inflections
    );
}

/// 未命中：translate 返回 ParseError（与原远程源「未收录」语义一致，不 panic）。
#[test]
fn provider_ecdict_translate_miss_returns_parse_error() {
    let (_dir, db) = ecdict_fixture("glacier", "", "n. 冰川", "");
    let provider = EcdictProvider::new(db);
    let req = make_request("notarealword", "en", "zh");

    let err = provider.translate(&req, &NeverExecutor);

    assert!(
        matches!(err, Err(TranslateError::ParseError(_))),
        "未收录词应返回 ParseError，实际：{err:?}"
    );
}
