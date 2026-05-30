//! 编译期静态注册表与 4 家 provider 的占位实现。
//!
//! 本文件只声明 capability 元数据（id/name/needs_key）和最小 trait 实现，
//! 真实 HTTP 构造与响应解析在 s06/s07 各 provider 子模块中补全。

use super::{
    ProviderCapability, ProviderHttpRequest, TranslateError, TranslateProvider, TranslateRequest,
    TranslateResponse,
};

/// 返回编译期静态注册表：4 家 provider 的能力声明列表。
///
/// 调用方可枚举此列表渲染 UI 选择器或构建凭据表单，无需运行时反射。
pub fn registry() -> Vec<ProviderCapability> {
    vec![
        MyMemoryProvider.capability(),
        BaiduProvider.capability(),
        DeepLFreeProvider.capability(),
        GoogleProvider.capability(),
    ]
}

/// MyMemory provider（默认源，无需 API Key）。
pub struct MyMemoryProvider;

impl TranslateProvider for MyMemoryProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "mymemory",
            name: "MyMemory",
            needs_key: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let lang_pair = format!("{}|{}", req.source_lang.as_str(), req.target_lang.as_str());
        ProviderHttpRequest {
            url: format!(
                "https://api.mymemory.translated.net/get?q={}&langpair={}",
                req.text, lang_pair
            ),
            body: None,
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;
        let translated = v["responseData"]["translatedText"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing translatedText".to_string()))?
            .to_string();
        Ok(TranslateResponse { translated })
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
