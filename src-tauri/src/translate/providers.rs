//! 编译期静态注册表与 4 家 provider 的完整实现。
//!
//! 本文件实现薄 provider 三件职责（capability / build_request / parse_response）；
//! HTTP 执行、重试、超时、凭据读取等横切关注点由核心框架层统一处理。

use super::{
    lang::map_lang_for_provider, ProviderCapability, ProviderHttpRequest, TranslateError,
    TranslateProvider, TranslateRequest, TranslateResponse,
};

/// 返回编译期静态注册表：4 家 provider 的能力声明列表。
///
/// 调用方可枚举此列表渲染 UI 选择器或构建凭据表单，无需运行时反射。
pub fn registry() -> Vec<ProviderCapability> {
    vec![
        MyMemoryProvider::new(None).capability(),
        BaiduProvider::new("", "").capability(),
        DeepLFreeProvider::new("").capability(),
        GoogleProvider::new("").capability(),
    ]
}

// MyMemory

/// MyMemory provider（默认源，无需 API Key）。
///
/// 匿名使用时每天 5000 字符配额；提供邮箱（`email`）可提升至 5 万字符/天。
/// 详见 MyMemory API 文档 §4.2。
pub struct MyMemoryProvider {
    /// 可选邮箱，用于提升每日配额。填入后请求 URL 会附加 `de=<email>` 参数。
    email: Option<String>,
}

impl MyMemoryProvider {
    /// 构造 MyMemory provider。
    ///
    /// `email` 为 `None` 时使用匿名配额（5000 字符/天）；
    /// 提供邮箱可提升至 5 万字符/天。
    pub fn new(email: Option<String>) -> Self {
        Self { email }
    }
}

impl TranslateProvider for MyMemoryProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "mymemory",
            name: "MyMemory",
            needs_key: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("mymemory", &req.source_lang);
        let tgt = map_lang_for_provider("mymemory", &req.target_lang);
        let lang_pair = format!("{}|{}", src, tgt);

        let mut url = format!(
            "https://api.mymemory.translated.net/get?q={}&langpair={}",
            percent_encode(&req.text),
            percent_encode_langpair(&lang_pair),
        );

        if let Some(email) = &self.email {
            url.push_str(&format!("&de={}", percent_encode(email)));
        }

        ProviderHttpRequest {
            method: "GET",
            url,
            body: None,
            headers: vec![],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        let status = match &v["responseStatus"] {
            serde_json::Value::Number(n) => n.as_u64().unwrap_or(0),
            serde_json::Value::String(s) => s.parse::<u64>().unwrap_or(0),
            _ => 0,
        };
        if status == 0 {
            return Err(TranslateError::ParseError(
                "responseStatus missing or unparseable".to_string(),
            ));
        }
        if status != 200 {
            return Err(map_mymemory_error(status, &v));
        }

        let translated = v["responseData"]["translatedText"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing translatedText".to_string()))?
            .to_string();

        Ok(TranslateResponse { translated })
    }
}

/// 将 MyMemory 非 200 状态码与响应体归一为 `TranslateError`。
///
/// MyMemory 用 403 表示配额耗尽（`responseDetails` 含 "FREE TRANSLATIONS" 文案）；
/// 429 表示频率超限；其余 4xx/5xx 保守归入 `Auth` 或 `ServerError`。
fn map_mymemory_error(status: u64, v: &serde_json::Value) -> TranslateError {
    let detail = v["responseDetails"]
        .as_str()
        .unwrap_or("")
        .to_ascii_uppercase();

    match status {
        403 if detail.contains("FREE TRANSLATIONS") || detail.contains("QUOTA") => {
            TranslateError::Quota(format!("MyMemory 配额耗尽: {detail}"))
        }
        403 => TranslateError::Auth(format!("MyMemory 认证失败: {detail}")),
        429 => TranslateError::RateLimit(format!("MyMemory 频率超限: {detail}")),
        500..=599 => TranslateError::ServerError(format!("MyMemory 服务端错误: HTTP {status}")),
        _ => TranslateError::ServerError(format!("MyMemory 未知错误: HTTP {status}")),
    }
}

// 百度翻译

/// 百度通用翻译 provider（需要 AppID + SecretKey）。
///
/// 签名算法（百度翻译 API 文档 §3.1）：
/// `sign = MD5(appid + q + salt + secret_key)`
/// salt 取随机整数字符串，防重放。
pub struct BaiduProvider {
    app_id: String,
    secret_key: String,
}

impl BaiduProvider {
    /// 构造百度翻译 provider。
    pub fn new(app_id: &str, secret_key: &str) -> Self {
        Self {
            app_id: app_id.to_string(),
            secret_key: secret_key.to_string(),
        }
    }
}

impl TranslateProvider for BaiduProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "baidu",
            name: "百度翻译",
            needs_key: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("baidu", &req.source_lang);
        let tgt = map_lang_for_provider("baidu", &req.target_lang);

        // 每次请求生成随机 salt 防重放攻击（百度 API 文档 §3.1 要求）。
        let salt = uuid::Uuid::new_v4().simple().to_string();
        let sign = baidu_sign(&self.app_id, &req.text, &salt, &self.secret_key);

        let body = format!(
            "q={}&from={}&to={}&appid={}&salt={}&sign={}",
            percent_encode(&req.text),
            src,
            tgt,
            percent_encode(&self.app_id),
            salt,
            sign,
        );

        ProviderHttpRequest {
            method: "POST",
            url: "https://fanyi-api.baidu.com/api/trans/vip/translate".to_string(),
            body: Some(body),
            headers: vec![(
                "Content-Type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            )],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 百度 API 在出错时返回 error_code 字段（字符串或数字）
        if let Some(code) = extract_number_or_string(&v["error_code"]) {
            return Err(map_baidu_error(code, &v));
        }

        let translated = v["trans_result"][0]["dst"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing trans_result[0].dst".to_string()))?
            .to_string();

        Ok(TranslateResponse { translated })
    }
}

/// 计算百度翻译请求签名。
///
/// 算法（百度翻译 API 文档 §3.1）：`sign = MD5(appid + q + salt + secret_key)` 的十六进制小写。
/// 抽为纯函数使测试可对任意 salt 直接验证签名算法的正确性，无需固定 salt。
pub fn baidu_sign(appid: &str, q: &str, salt: &str, secret_key: &str) -> String {
    let input = format!("{appid}{q}{salt}{secret_key}");
    format!("{:x}", md5::compute(input.as_bytes()))
}

/// 将百度 API 错误码归一为 `TranslateError`。
///
/// 错误码来源：百度翻译 API 文档 §5.1 错误码列表。
fn map_baidu_error(code: u64, v: &serde_json::Value) -> TranslateError {
    let msg = v["error_msg"].as_str().unwrap_or("unknown").to_string();
    match code {
        52001 | 52002 => TranslateError::Network(format!("百度翻译网络超时: {msg}")),
        52003 => TranslateError::Auth(format!("百度翻译认证失败（未授权 appid）: {msg}")),
        54000 => TranslateError::TooLong(format!("百度翻译原文过长: {msg}")),
        54001 => TranslateError::Auth(format!("百度翻译签名错误: {msg}")),
        54003 => TranslateError::RateLimit(format!("百度翻译频率超限: {msg}")),
        54004 => TranslateError::Quota(format!("百度翻译余额不足: {msg}")),
        54005 => TranslateError::RateLimit(format!("百度翻译长文本频率超限: {msg}")),
        58001 => TranslateError::Unsupported(format!("百度翻译语言不支持: {msg}")),
        58002 => TranslateError::ServerError(format!("百度翻译服务不可用: {msg}")),
        _ => TranslateError::ServerError(format!("百度翻译未知错误 {code}: {msg}")),
    }
}

// DeepL Free

/// DeepL Free API provider（需要 Auth Key）。
///
/// 端点：`https://api-free.deepl.com/v2/translate`（Free 层级专用）。
/// 鉴权：HTTP header `Authorization: DeepL-Auth-Key <auth_key>`。
pub struct DeepLFreeProvider {
    auth_key: String,
}

impl DeepLFreeProvider {
    /// 构造 DeepL Free provider。
    pub fn new(auth_key: &str) -> Self {
        Self {
            auth_key: auth_key.to_string(),
        }
    }
}

impl TranslateProvider for DeepLFreeProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "deepl_free",
            name: "DeepL Free",
            needs_key: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("deepl_free", &req.source_lang);
        let tgt = map_lang_for_provider("deepl_free", &req.target_lang);

        let body = format!(
            "text={}&source_lang={}&target_lang={}",
            percent_encode(&req.text),
            src,
            tgt,
        );

        ProviderHttpRequest {
            method: "POST",
            url: "https://api-free.deepl.com/v2/translate".to_string(),
            body: Some(body),
            headers: vec![
                (
                    "Authorization".to_string(),
                    format!("DeepL-Auth-Key {}", self.auth_key),
                ),
                (
                    "Content-Type".to_string(),
                    "application/x-www-form-urlencoded".to_string(),
                ),
            ],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        // DeepL 在错误时返回 HTTP 状态码，但本 trait 只拿到响应体。
        // 错误响应体格式：{"message":"..."} 或 {"detail":"..."}。
        // 成功响应体：{"translations":[{"text":"...","detected_source_language":"..."}]}
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 检测错误响应：含 message 字段且无 translations 字段时视为错误
        if v["translations"].is_null() {
            let msg = v["message"]
                .as_str()
                .or_else(|| v["detail"].as_str())
                .unwrap_or("unknown error");
            return Err(map_deepl_error_from_body(msg));
        }

        let translated = v["translations"][0]["text"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing translations[0].text".to_string()))?
            .to_string();

        Ok(TranslateResponse { translated })
    }
}

/// 将 DeepL 错误响应体归一为 `TranslateError`。
///
/// DeepL 用 HTTP 状态码表达错误；框架层可将状态码注入 body 供此处解析，
/// 或由框架层直接拦截 HTTP 错误状态码后转换（s03 职责）。
/// 此处处理 body 层面可识别的错误文案。
fn map_deepl_error_from_body(msg: &str) -> TranslateError {
    let upper = msg.to_ascii_uppercase();
    // 仅匹配 DeepL 文档明确用词或 HTTP 状态码数字，避免宽泛字母子串误匹配。
    if upper.contains("QUOTA") || upper.contains("456") || upper.contains("LIMIT EXCEEDED") {
        TranslateError::Quota(format!("DeepL Free 配额耗尽: {msg}"))
    } else if upper.contains("403") {
        TranslateError::Auth(format!("DeepL Free 认证失败: {msg}"))
    } else if upper.contains("TOO MANY") || upper.contains("429") {
        TranslateError::RateLimit(format!("DeepL Free 频率超限: {msg}"))
    } else {
        TranslateError::ServerError(format!("DeepL Free 错误: {msg}"))
    }
}

/// 将 DeepL HTTP 状态码（由框架层传入）归一为 `TranslateError`。
///
/// 供框架层（s03）在拿到 HTTP 状态码后调用，不依赖响应体文案。
pub fn map_deepl_http_status(status: u16, body: &str) -> TranslateError {
    match status {
        403 => TranslateError::Auth(format!("DeepL Free 认证失败: HTTP 403 {body}")),
        429 => TranslateError::RateLimit(format!("DeepL Free 频率超限: HTTP 429 {body}")),
        456 => TranslateError::Quota(format!("DeepL Free 配额耗尽: HTTP 456 {body}")),
        500..=599 => {
            TranslateError::ServerError(format!("DeepL Free 服务端错误: HTTP {status} {body}"))
        }
        _ => TranslateError::ServerError(format!("DeepL Free 未知错误: HTTP {status} {body}")),
    }
}

// Google Cloud Translation

/// Google Cloud Translation API provider（需要 API Key）。
///
/// 端点：`https://translation.googleapis.com/language/translate/v2`
/// 鉴权：查询参数 `key=<api_key>`（简单 API Key 认证）。
pub struct GoogleProvider {
    api_key: String,
}

impl GoogleProvider {
    /// 构造 Google 翻译 provider。
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
        }
    }
}

impl TranslateProvider for GoogleProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "google",
            name: "Google 翻译",
            needs_key: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("google", &req.source_lang);
        let tgt = map_lang_for_provider("google", &req.target_lang);

        let url = format!(
            "https://translation.googleapis.com/language/translate/v2?key={}",
            percent_encode(&self.api_key),
        );

        let body = format!(
            "q={}&source={}&target={}&format=text",
            percent_encode(&req.text),
            src,
            tgt,
        );

        ProviderHttpRequest {
            method: "POST",
            url,
            body: Some(body),
            headers: vec![(
                "Content-Type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            )],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // Google API 在出错时返回顶层 error 对象
        if !v["error"].is_null() {
            return Err(map_google_error(&v["error"]));
        }

        let translated = v["data"]["translations"][0]["translatedText"]
            .as_str()
            .ok_or_else(|| {
                TranslateError::ParseError(
                    "missing data.translations[0].translatedText".to_string(),
                )
            })?
            .to_string();

        Ok(TranslateResponse { translated })
    }
}

/// 将 Google API 错误对象归一为 `TranslateError`。
///
/// Google 错误格式：`{"code": 403, "status": "PERMISSION_DENIED", "message": "..."}`
/// code/status 可能是数字或字符串，需兼容两种形态（吸取 s06 审查教训）。
fn map_google_error(error: &serde_json::Value) -> TranslateError {
    let msg = error["message"].as_str().unwrap_or("unknown").to_string();
    let status_str = error["status"].as_str().unwrap_or("").to_ascii_uppercase();

    // 优先从 status 字符串判断，再用 code 数字兜底
    if status_str.contains("QUOTA") || status_str.contains("RESOURCE_EXHAUSTED") {
        return TranslateError::Quota(format!("Google 翻译配额耗尽: {msg}"));
    }
    if status_str.contains("PERMISSION_DENIED") || status_str.contains("UNAUTHENTICATED") {
        return TranslateError::Auth(format!("Google 翻译认证失败: {msg}"));
    }

    // code 字段兼容 Number 和 String 两种 JSON 形态
    let code = extract_number_or_string(&error["code"]).unwrap_or(0);
    match code {
        403 => TranslateError::Auth(format!("Google 翻译认证失败: HTTP 403 {msg}")),
        429 => TranslateError::RateLimit(format!("Google 翻译频率超限: HTTP 429 {msg}")),
        500..=599 => {
            TranslateError::ServerError(format!("Google 翻译服务端错误: HTTP {code} {msg}"))
        }
        _ => TranslateError::ServerError(format!("Google 翻译未知错误 {code}: {msg}")),
    }
}

// 撞额度显式提示（A11）

/// 用户提示类型：当 provider 撞额度/鉴权失败时应向用户展示的引导信息。
///
/// 铁律：此函数**绝不**返回"自动切换到另一个 provider"的动作。
/// 换源必须由用户显式操作（§4.2 铁律）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserPromptKind {
    /// 引导用户填入邮箱（MyMemory 配额提升）。
    NeedEmail,
    /// 引导用户填入 API Key（有 key 的 provider）。
    NeedKey,
    /// 通用显式提示（其他需要用户干预的错误）。
    Explicit,
}

/// 撞额度或失败时向用户展示的显式提示。
#[derive(Debug, Clone)]
pub struct UserPrompt {
    /// 提示类型，UI 层据此渲染不同的引导交互。
    pub kind: UserPromptKind,
    /// 面向用户的提示文案。
    pub message: String,
}

/// 当 provider 返回 `Quota` 或 `Auth` 错误时，生成**显式用户提示**。
///
/// 设计铁律（§4.2）：此函数**绝不**包含跨 provider 自动切换逻辑。
/// 换源必须用户显式操作；此处只负责告知用户需要做什么（填邮箱 / 填 key）。
///
/// 调用方负责把返回的 `UserPrompt` 展示给用户，而非自动执行任何切换。
pub fn on_quota_or_failure(provider_id: &str, err: &TranslateError) -> Option<UserPrompt> {
    match err {
        TranslateError::Quota(_) => {
            let prompt = if provider_id == "mymemory" {
                UserPrompt {
                    kind: UserPromptKind::NeedEmail,
                    message: "MyMemory 匿名配额已耗尽。填入邮箱可提升至每日 5 万字符配额。"
                        .to_string(),
                }
            } else {
                UserPrompt {
                    kind: UserPromptKind::NeedKey,
                    message: format!(
                        "「{provider_id}」配额已耗尽，请检查账户余额或升级套餐，或填入有效 API Key。"
                    ),
                }
            };
            Some(prompt)
        }
        TranslateError::Auth(_) => Some(UserPrompt {
            kind: UserPromptKind::NeedKey,
            message: format!("「{provider_id}」认证失败，请填入有效的 API Key。"),
        }),
        _ => None,
    }
}

// 工具函数

/// 从 JSON 值中提取数字，兼容 Number 和 String 两种 JSON 形态。
///
/// 吸取 s06 审查教训：错误码可能以字符串 "403" 或数字 403 出现，
/// 需兼容两种形态；无法解析时返回 None，不静默当成功。
fn extract_number_or_string(v: &serde_json::Value) -> Option<u64> {
    match v {
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

/// 对字符串做 RFC 3986 percent-encoding，可额外传入允许不编码的字节集。
///
/// 始终保留 unreserved 字符集（`A-Z a-z 0-9 - _ . ~`），其余字节编码为 `%XX`。
/// `extra_safe` 中列举的字节同样原样透传，用于 langpair 中保留 `|` 分隔符。
fn percent_encode_with_extra(s: &str, extra_safe: &[u8]) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        let is_unreserved = matches!(byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~'
        );
        if is_unreserved || extra_safe.contains(&byte) {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(hex_upper(byte >> 4));
            out.push(hex_upper(byte & 0x0F));
        }
    }
    out
}

/// 对查询参数值做 RFC 3986 percent-encoding（不保留任何额外字符）。
fn percent_encode(s: &str) -> String {
    percent_encode_with_extra(s, &[])
}

/// 对 langpair（如 `en|zh-CN`）做 percent-encoding，保留 `|` 分隔符。
///
/// MyMemory API 要求 langpair 以 `src|tgt` 形式传递，`|` 是语义分隔符，
/// 不能被编码为 `%7C`，否则 API 无法识别语言对。
fn percent_encode_langpair(lang_pair: &str) -> String {
    percent_encode_with_extra(lang_pair, b"|")
}

/// 将 4 位 nibble 转为大写十六进制字符。
fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + nibble - 10) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_idempotent() {
        let first = registry();
        let second = registry();
        assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.id, b.id);
        }
    }
}
