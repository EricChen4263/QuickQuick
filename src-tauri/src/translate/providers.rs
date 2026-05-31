//! 编译期静态注册表与 4 家 provider 的占位实现。
//!
//! 本文件只声明 capability 元数据（id/name/needs_key）和最小 trait 实现，
//! 真实 HTTP 构造与响应解析在 s06/s07 各 provider 子模块中补全。

use super::{
    lang::map_lang_for_provider,
    ProviderCapability, ProviderHttpRequest, TranslateError, TranslateProvider, TranslateRequest,
    TranslateResponse,
};

/// 返回编译期静态注册表：4 家 provider 的能力声明列表。
///
/// 调用方可枚举此列表渲染 UI 选择器或构建凭据表单，无需运行时反射。
pub fn registry() -> Vec<ProviderCapability> {
    vec![
        MyMemoryProvider::new(None).capability(),
        BaiduProvider.capability(),
        DeepLFreeProvider.capability(),
        GoogleProvider.capability(),
    ]
}

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

        ProviderHttpRequest { url, body: None }
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
    let detail = v["responseDetails"].as_str().unwrap_or("").to_ascii_uppercase();

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

/// 百度翻译 provider（需要 AppID + SecretKey）。
pub struct BaiduProvider;

impl TranslateProvider for BaiduProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "baidu",
            name: "百度翻译",
            needs_key: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        // 真实签名构造在 s07 实现；此处返回描述结构供测试断言
        ProviderHttpRequest {
            url: "https://fanyi-api.baidu.com/api/trans/vip/translate".to_string(),
            body: Some(format!(
                "q={}&from={}&to={}",
                req.text,
                req.source_lang.as_str(),
                req.target_lang.as_str()
            )),
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;
        let translated = v["trans_result"][0]["dst"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing trans_result[0].dst".to_string()))?
            .to_string();
        Ok(TranslateResponse { translated })
    }
}

/// DeepL Free provider（需要 Auth Key）。
pub struct DeepLFreeProvider;

impl TranslateProvider for DeepLFreeProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "deepl_free",
            name: "DeepL Free",
            needs_key: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        ProviderHttpRequest {
            url: "https://api-free.deepl.com/v2/translate".to_string(),
            body: Some(format!(
                "text={}&source_lang={}&target_lang={}",
                req.text,
                req.source_lang.as_str().to_uppercase(),
                req.target_lang.as_str().to_uppercase()
            )),
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;
        let translated = v["translations"][0]["text"]
            .as_str()
            .ok_or_else(|| {
                TranslateError::ParseError("missing translations[0].text".to_string())
            })?
            .to_string();
        Ok(TranslateResponse { translated })
    }
}

/// Google Cloud Translation provider（需要 API Key）。
pub struct GoogleProvider;

impl TranslateProvider for GoogleProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "google",
            name: "Google 翻译",
            needs_key: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        ProviderHttpRequest {
            url: format!(
                "https://translation.googleapis.com/language/translate/v2?q={}&source={}&target={}",
                req.text,
                req.source_lang.as_str(),
                req.target_lang.as_str()
            ),
            body: None,
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 冒烟：registry() 是幂等的纯函数，多次调用结果一致
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
