//! 编译期静态注册表与各 provider 的完整实现。
//!
//! 本文件实现薄 provider 三件职责（capability / build_request / parse_response）；
//! HTTP 执行、重试、超时、凭据读取等横切关注点由核心框架层统一处理。

use super::{
    lang::map_lang_for_provider, DictEntry, HttpExecutor, PosDefinition, ProviderCapability,
    ProviderHttpRequest, TranslateError, TranslateProvider, TranslateRequest, TranslateResponse,
};

/// 按 provider_id 与凭据切片动态构造对应的 `TranslateProvider`。
///
/// `credentials` 为 `(field_key, value)` 键值对切片，与 `load_credentials` 返回类型一致。
/// 字段名必须与 `credential_schema` 声明的 key 逐字对齐：
/// - `lingva`、`google_free`、`yandex`、`transmart`、`bing`、`ecdict`：无凭据（免 key）
/// - `baidu`：`app_id`（必填）、`secret_key`（必填）
/// - `baidu_field`：`app_id`、`secret_key`、`field`（领域，均必填）
/// - `youdao`、`youdao_dict`：`app_key`、`app_secret`（均必填，词典模式同翻译 key）
/// - `caiyun`：`token`（必填）
/// - `niutrans`：`apikey`（必填）
/// - `tencent`：`secret_id`、`secret_key`（均必填）
/// - `alibaba`：`accesskey_id`、`accesskey_secret`（均必填）
/// - `deepl_free`：`auth_key`（必填）
/// - `google`：`api_key`（必填）
///
/// # Errors
/// - 未知 provider_id → Err（中文描述）
/// - 必填字段缺失 → Err（中文描述，不含字段值，符合安全约定）
/// - 不 panic、不回退到任何源
pub fn build_provider(
    provider_id: &str,
    credentials: &[(String, String)],
) -> Result<Box<dyn super::TranslateProvider>, String> {
    // trim 后空字符串视同缺失，避免把全空白值当有效凭据，防止签名错误（如百度 54001）。
    let find = |key: &str| -> Option<&str> {
        credentials
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.trim())
            .filter(|s| !s.is_empty())
    };

    match provider_id {
        "lingva" => Ok(Box::new(LingvaProvider::new())),
        "google_free" => Ok(Box::new(GoogleFreeProvider::new())),
        "yandex" => Ok(Box::new(YandexProvider::new())),
        "transmart" => Ok(Box::new(TransmartProvider::new())),
        "bing" => Ok(Box::new(BingProvider::new())),
        "ecdict" => Ok(Box::new(EcdictProvider::new())),
        "baidu" => {
            let app_id = find("app_id")
                .ok_or_else(|| "baidu 未配置 AppID，请前往设置填入 API Key".to_string())?;
            let secret_key = find("secret_key")
                .ok_or_else(|| "baidu 未配置 SecretKey，请前往设置填入 API Key".to_string())?;
            Ok(Box::new(BaiduProvider::new(app_id, secret_key)))
        }
        "baidu_field" => {
            let app_id = find("app_id")
                .ok_or_else(|| "baidu_field 未配置 AppID，请前往设置填入 API Key".to_string())?;
            let secret_key = find("secret_key")
                .ok_or_else(|| "baidu_field 未配置 SecretKey，请前往设置填入 API Key".to_string())?;
            let field = find("field")
                .ok_or_else(|| "baidu_field 未配置领域 field，请前往设置填入".to_string())?;
            Ok(Box::new(BaiduFieldProvider::new(app_id, secret_key, field)))
        }
        "youdao" => {
            let app_key = find("app_key")
                .ok_or_else(|| "youdao 未配置应用 ID，请前往设置填入 API Key".to_string())?;
            let app_secret = find("app_secret")
                .ok_or_else(|| "youdao 未配置应用密钥，请前往设置填入 API Key".to_string())?;
            Ok(Box::new(YoudaoProvider::new(app_key, app_secret)))
        }
        "youdao_dict" => {
            let app_key = find("app_key")
                .ok_or_else(|| "youdao_dict 未配置应用 ID，请前往设置填入 API Key".to_string())?;
            let app_secret = find("app_secret")
                .ok_or_else(|| "youdao_dict 未配置应用密钥，请前往设置填入 API Key".to_string())?;
            Ok(Box::new(YoudaoDictProvider::new(app_key, app_secret)))
        }
        "caiyun" => {
            let token = find("token")
                .ok_or_else(|| "caiyun 未配置 token，请前往设置填入 API Key".to_string())?;
            Ok(Box::new(CaiyunProvider::new(token)))
        }
        "niutrans" => {
            let apikey = find("apikey")
                .ok_or_else(|| "niutrans 未配置 apikey，请前往设置填入 API Key".to_string())?;
            Ok(Box::new(NiutransProvider::new(apikey)))
        }
        "tencent" => {
            let secret_id = find("secret_id")
                .ok_or_else(|| "tencent 未配置 SecretId，请前往设置填入 API Key".to_string())?;
            let secret_key = find("secret_key")
                .ok_or_else(|| "tencent 未配置 SecretKey，请前往设置填入 API Key".to_string())?;
            Ok(Box::new(TencentProvider::new(secret_id, secret_key)))
        }
        "alibaba" => {
            let access_key_id = find("accesskey_id").ok_or_else(|| {
                "alibaba 未配置 AccessKey ID，请前往设置填入 API Key".to_string()
            })?;
            let access_key_secret = find("accesskey_secret").ok_or_else(|| {
                "alibaba 未配置 AccessKey Secret，请前往设置填入 API Key".to_string()
            })?;
            Ok(Box::new(AlibabaProvider::new(
                access_key_id,
                access_key_secret,
            )))
        }
        "volcengine" => {
            let access_key_id = find("access_key_id").ok_or_else(|| {
                "volcengine 未配置 AccessKeyId，请前往设置填入 API Key".to_string()
            })?;
            let secret_access_key = find("secret_access_key").ok_or_else(|| {
                "volcengine 未配置 SecretAccessKey，请前往设置填入 API Key".to_string()
            })?;
            // region 选填，留空回退默认地域。
            let region = find("region").unwrap_or(VOLCENGINE_DEFAULT_REGION);
            Ok(Box::new(VolcengineProvider::new(
                access_key_id,
                secret_access_key,
                region,
            )))
        }
        "deepl_free" => {
            let auth_key = find("auth_key")
                .ok_or_else(|| "deepl_free 未配置 auth_key，请前往设置填入 API Key".to_string())?;
            Ok(Box::new(DeepLFreeProvider::new(auth_key)))
        }
        "google" => {
            let api_key = find("api_key")
                .ok_or_else(|| "google 未配置 api_key，请前往设置填入 API Key".to_string())?;
            Ok(Box::new(GoogleProvider::new(api_key)))
        }
        "openai" => {
            let api_key = find("apiKey")
                .ok_or_else(|| "openai 未配置 apiKey，请前往设置填入 API Key".to_string())?;
            let model = find("model")
                .ok_or_else(|| "openai 未配置 model，请前往设置填入模型名".to_string())?;
            // base_url/prompt 选填：base_url 留空回退官方端点，prompt 留空回退内置默认。
            let base_url = find("base_url").unwrap_or("");
            let prompt = find("prompt").unwrap_or("");
            Ok(Box::new(OpenAiProvider::new(api_key, model, base_url, prompt)))
        }
        "ollama" => {
            let model = find("model")
                .ok_or_else(|| "ollama 未配置 model，请前往设置填入模型名".to_string())?;
            // base_url/prompt 选填：base_url 留空回退本地默认，prompt 留空回退内置默认。
            let base_url = find("base_url").unwrap_or("");
            let prompt = find("prompt").unwrap_or("");
            Ok(Box::new(OllamaProvider::new(model, base_url, prompt)))
        }
        "chatglm" => {
            let api_key = find("apiKey")
                .ok_or_else(|| "chatglm 未配置 apiKey，请前往设置填入 API Key".to_string())?;
            let model = find("model")
                .ok_or_else(|| "chatglm 未配置 model，请前往设置填入模型名".to_string())?;
            // base_url/prompt 选填：base_url 留空回退官方端点，prompt 留空回退内置默认。
            let base_url = find("base_url").unwrap_or("");
            let prompt = find("prompt").unwrap_or("");
            Ok(Box::new(ChatGlmProvider::new(api_key, model, base_url, prompt)))
        }
        "gemini" => {
            let api_key = find("apiKey")
                .ok_or_else(|| "gemini 未配置 apiKey，请前往设置填入 API Key".to_string())?;
            let model = find("model")
                .ok_or_else(|| "gemini 未配置 model，请前往设置填入模型名".to_string())?;
            // base_url/prompt 选填：base_url 留空回退官方域名，prompt 留空回退内置默认。
            let base_url = find("base_url").unwrap_or("");
            let prompt = find("prompt").unwrap_or("");
            Ok(Box::new(GeminiProvider::new(api_key, model, base_url, prompt)))
        }
        other => Err(format!("未知翻译 provider：{other}")),
    }
}

/// 返回编译期静态注册表：各 provider 的能力声明列表。
///
/// 调用方可枚举此列表渲染 UI 选择器或构建凭据表单，无需运行时反射。
/// Lingva 置于首位，作为免 key 默认源。
pub fn registry() -> Vec<ProviderCapability> {
    vec![
        LingvaProvider::new().capability(),
        GoogleFreeProvider::new().capability(),
        YandexProvider::new().capability(),
        TransmartProvider::new().capability(),
        BingProvider::new().capability(),
        EcdictProvider::new().capability(),
        BaiduProvider::new("", "").capability(),
        BaiduFieldProvider::new("", "", "").capability(),
        YoudaoProvider::new("", "").capability(),
        YoudaoDictProvider::new("", "").capability(),
        CaiyunProvider::new("").capability(),
        NiutransProvider::new("").capability(),
        TencentProvider::new("", "").capability(),
        AlibabaProvider::new("", "").capability(),
        VolcengineProvider::new("", "", "").capability(),
        DeepLFreeProvider::new("").capability(),
        GoogleProvider::new("").capability(),
        OpenAiProvider::new("", "", "", "").capability(),
        OllamaProvider::new("", "", "").capability(),
        ChatGlmProvider::new("", "", "", "").capability(),
        GeminiProvider::new("", "", "", "").capability(),
    ]
}

// Lingva

/// Lingva provider（免 key 默认源）。
///
/// Lingva 是开源 Google 翻译前端，提供无认证的纯 GET HTTP 接口。
/// 端点（按公开互操作协议事实独立实现，不参考任何第三方源码）：
/// `GET https://lingva.ml/api/v1/{source}/{target}/{url-encoded text}`
/// 响应体形如 `{"translation":"..."}`，译文取 `translation` 字段。
/// 选用 lingva.ml 公共实例（实测返回正常 JSON）：译质等同 Google 引擎；
/// 运营依赖第三方实例可用性的风险已记入设计文档§七。
pub struct LingvaProvider;

impl LingvaProvider {
    /// 构造 Lingva provider（无凭据）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for LingvaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslateProvider for LingvaProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "lingva",
            name: "Lingva",
            needs_key: false,
            is_unofficial: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("lingva", &req.source_lang);
        let tgt = map_lang_for_provider("lingva", &req.target_lang);

        // 路径段：src/tgt 为 ASCII 语言码无需编码；text 走 RFC 3986 percent-encode。
        let url = format!(
            "https://lingva.ml/api/v1/{}/{}/{}",
            src,
            tgt,
            percent_encode(&req.text),
        );

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

        let translated = v["translation"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing translation".to_string()))?
            .to_string();

        Ok(TranslateResponse::plain(translated))
    }
}

// Google 免费（translate_a/single 公开接口协议）

/// Google 免费翻译 provider（免 key，非官方 `gtx` 接口）。
///
/// 端点（按 Google translate_a/single 公开接口协议事实独立实现，不参考任何第三方源码）：
/// `GET https://translate.googleapis.com/translate_a/single?client=gtx&sl={src}&tl={tgt}&dt=t&q={text}`
/// - `client=gtx`、`dt=t` 为该公开接口固定参数（请求纯文本翻译结果）。
/// - 响应体形如 `[[["glacier","冰川",null,null,2]],null,"zh-CN",...]`：
///   `result[0]` 是分句译文对数组，译文 = 拼接各 `result[0][i][0]`（每个分句的第 0 元素）。
///
/// 与官方 `google`（needs_key=true）区分：本源 id=`google_free`、needs_key=false，
/// 是非官方接口、Google 可随时封禁，作为免 key 备选/兜底（设计文档§二.2.1）。
pub struct GoogleFreeProvider;

impl GoogleFreeProvider {
    /// 构造 Google 免费 provider（无凭据）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for GoogleFreeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslateProvider for GoogleFreeProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "google_free",
            name: "Google（免费）",
            needs_key: false,
            is_unofficial: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("google_free", &req.source_lang);
        let tgt = map_lang_for_provider("google_free", &req.target_lang);

        // q 为查询参数值需 percent-encode；sl/tl 为 ASCII 语言码、固定参数无需编码。
        let url = format!(
            "https://translate.googleapis.com/translate_a/single?client=gtx&sl={}&tl={}&dt=t&q={}",
            src,
            tgt,
            percent_encode(&req.text),
        );

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

        // result[0] 是分句译文对数组；缺失或非数组即视为格式错误。
        let segments = v[0]
            .as_array()
            .ok_or_else(|| TranslateError::ParseError("missing result[0] array".to_string()))?;
        if segments.is_empty() {
            return Err(TranslateError::ParseError("empty result[0]".to_string()));
        }

        // 译文 = 各分句第 0 元素拼接（实测 Google 不在分句间补空格）。
        let mut translated = String::new();
        for segment in segments {
            let part = segment[0].as_str().ok_or_else(|| {
                TranslateError::ParseError("missing segment translation text".to_string())
            })?;
            translated.push_str(part);
        }

        Ok(TranslateResponse::plain(translated))
    }
}

// Yandex 免费（伪装 Android 客户端公开协议）

/// Yandex 免费翻译 provider（免 key，非官方伪装客户端接口）。
///
/// 端点（按 Yandex translate v1 tr.json 公开互操作协议事实独立实现，不参考任何第三方源码）：
/// `POST https://translate.yandex.net/api/v1/tr.json/translate?id={hex}-0-0&srv=android`
/// body：`text={url-encoded}&lang={src}-{tgt}`（urlencoded 表单）。
/// - 实测：必须用 `srv=android` 且 `id` 为去连字符 uuid 加 `-0-0` 后缀、body 单参 `lang=src-tgt`
///   连字符对，否则返回 `{"code":405,"message":"Session is invalid"}`（HTTP 403）。
/// - 成功响应 `{"code":200,...,"text":["..."]}`，译文取 `text` 数组拼接。
///
/// 非官方接口、Yandex 可随时封禁，作为免 key 备选（设计文档§二.2.1）。
/// `id` 仅是客户端会话标识、非安全凭据，用随机 uuid 防请求被去重即可。
pub struct YandexProvider;

impl YandexProvider {
    /// 构造 Yandex 免费 provider（无凭据）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for YandexProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslateProvider for YandexProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "yandex",
            name: "Yandex（免费）",
            needs_key: false,
            is_unofficial: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("yandex", &req.source_lang);
        let tgt = map_lang_for_provider("yandex", &req.target_lang);

        // id 为去连字符 uuid 加 -0-0：实测此格式 + srv=android 才返回 200。
        // 仅作客户端会话标识、非安全敏感值，故用普通 v4 uuid。
        let session_id = uuid::Uuid::new_v4().simple().to_string();
        let url = format!(
            "https://translate.yandex.net/api/v1/tr.json/translate?id={session_id}-0-0&srv=android",
        );

        // body：text 走 percent-encode；lang 为 src-tgt 连字符对（ASCII 语言码无需编码）。
        let body = format!("text={}&lang={}-{}", percent_encode(&req.text), src, tgt);

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

        // Yandex 用 body 内 code 字段表达错误（非 HTTP 状态码可在此层拿到）。
        if let Some(code) = v["code"].as_u64() {
            if code != 200 {
                let msg = v["message"].as_str().unwrap_or("unknown");
                return Err(map_yandex_error(code, msg));
            }
        }

        // text 为译文数组（长文本可能分段），按顺序拼接。
        let segments = v["text"]
            .as_array()
            .ok_or_else(|| TranslateError::ParseError("missing text array".to_string()))?;
        let mut translated = String::new();
        for seg in segments {
            if let Some(part) = seg.as_str() {
                translated.push_str(part);
            }
        }
        if translated.is_empty() {
            return Err(TranslateError::ParseError("empty text array".to_string()));
        }

        Ok(TranslateResponse::plain(translated))
    }
}

/// 将 Yandex `code` 字段（非 200）归一为 `TranslateError`。
///
/// 错误码来源：Yandex translate v1 tr.json 公开协议响应观测（实测 405=会话失效）。
fn map_yandex_error(code: u64, msg: &str) -> TranslateError {
    match code {
        401 | 402 | 403 | 405 => TranslateError::Auth(format!("Yandex 会话/鉴权失败 {code}: {msg}")),
        404 | 413 => TranslateError::TooLong(format!("Yandex 文本过长 {code}: {msg}")),
        422 | 501 => TranslateError::Unsupported(format!("Yandex 语言不支持 {code}: {msg}")),
        429 => TranslateError::RateLimit(format!("Yandex 频率超限 {code}: {msg}")),
        _ => TranslateError::ServerError(format!("Yandex 错误 {code}: {msg}")),
    }
}

// Transmart 腾讯交互翻译·匿名（公开协议）

/// Transmart（腾讯交互翻译·匿名）免费翻译 provider（免 key，非官方匿名接口）。
///
/// 端点（按 transmart.qq.com/api/imt 公开互操作协议事实独立实现，不参考任何第三方源码）：
/// `POST https://transmart.qq.com/api/imt`，匿名 JSON body：
/// `{"header":{"fn":"auto_translation","client_key":"..."},"type":"plain","model_category":"normal",
///   "source":{"lang":"{src}","text_list":["{text}"]},"target":{"lang":"{tgt}"}}`。
/// - 成功响应 `{"header":{"ret_code":"succ",..},"auto_translation":["..",".."],..}`，
///   译文取 `auto_translation` 数组拼接（text_list 逐项对应）。
/// - `ret_code != "succ"` 视为错误。
///
/// 非官方匿名接口（可选填 user/token 提限额，本免 key 实现不填），作为免 key 备选（设计文档§二.2.1）。
pub struct TransmartProvider;

impl TransmartProvider {
    /// 构造 Transmart 免费 provider（无凭据）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for TransmartProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TransmartProvider {
    /// 构造匿名 client_key：实测匿名调用只需一个非空标识，用随机 uuid 即可。
    ///
    /// client_key 是浏览器指纹式标识、非安全凭据；用随机值避免请求被去重/限流绑定。
    fn anonymous_client_key() -> String {
        format!("browser-quickquick-{}", uuid::Uuid::new_v4().simple())
    }
}

impl TranslateProvider for TransmartProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "transmart",
            name: "腾讯交互翻译（免费）",
            needs_key: false,
            is_unofficial: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("transmart", &req.source_lang);
        let tgt = map_lang_for_provider("transmart", &req.target_lang);

        // 用 serde_json::json! 构造 body，自动正确转义文本（避免手拼 JSON 注入风险）。
        let body = serde_json::json!({
            "header": {
                "fn": "auto_translation",
                "client_key": Self::anonymous_client_key(),
            },
            "type": "plain",
            "model_category": "normal",
            "source": { "lang": src, "text_list": [req.text] },
            "target": { "lang": tgt },
        })
        .to_string();

        ProviderHttpRequest {
            method: "POST",
            url: "https://transmart.qq.com/api/imt".to_string(),
            body: Some(body),
            headers: vec![(
                "Content-Type".to_string(),
                "application/json".to_string(),
            )],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // header.ret_code != "succ" 表示接口层错误。
        if let Some(ret_code) = v["header"]["ret_code"].as_str() {
            if ret_code != "succ" {
                let msg = v["header"]["message"].as_str().unwrap_or("unknown");
                return Err(TranslateError::ServerError(format!(
                    "Transmart 错误 {ret_code}: {msg}"
                )));
            }
        }

        // auto_translation 为译文数组（与 text_list 逐项对应），按顺序拼接。
        let segments = v["auto_translation"]
            .as_array()
            .ok_or_else(|| TranslateError::ParseError("missing auto_translation array".to_string()))?;
        let mut translated = String::new();
        for seg in segments {
            if let Some(part) = seg.as_str() {
                translated.push_str(part);
            }
        }
        if translated.is_empty() {
            return Err(TranslateError::ParseError(
                "empty auto_translation".to_string(),
            ));
        }

        Ok(TranslateResponse::plain(translated))
    }
}

// Bing 免费（Edge 翻译两步公开协议）

/// Bing edge 翻译端点常量（公开互操作协议事实，非 pot 源码）。
///
/// 注释与端点来源：实测 curl edge.microsoft.com/translate/auth + api-edge cognitive
/// （证据 artifacts/bing-auth-token-head.txt、bing-translate-sample.json）。
const BING_AUTH_URL: &str = "https://edge.microsoft.com/translate/auth";
const BING_TRANSLATE_BASE: &str =
    "https://api-edge.cognitive.microsofttranslator.com/translate?api-version=3.0";

/// Bing 免费翻译 provider（免 key，两步：抓 edge auth token → cognitive translate）。
///
/// 机制（按 Bing edge 翻译公开互操作协议事实独立实现，**不参考任何第三方源码**）：
/// 1. `GET https://edge.microsoft.com/translate/auth` → 响应体为纯文本 JWT（Bearer token）。
/// 2. `POST https://api-edge.cognitive.microsofttranslator.com/translate?api-version=3.0&from={src}&to={tgt}`
///    Header `Authorization: Bearer {token}`、`Content-Type: application/json`，
///    body `[{"Text":"..."}]`，响应 `[{"translations":[{"text":"..."}]}]`，译文取 `[0].translations[0].text`。
///
/// 这是 provider 多步请求架构的验证源：override `translate` 用注入的 `HttpExecutor`
/// 自行编排两次 HTTP，单步源沿用默认 `translate` 不受影响（设计文档§四）。
/// 非官方 Edge 接口、微软可随时变更，作为免 key 备选（设计文档§二.2.1 / §七）。
pub struct BingProvider;

impl BingProvider {
    /// 构造 Bing 免费 provider（无凭据）。
    pub fn new() -> Self {
        Self
    }

    /// 构造取 auth token 的请求（第一步）。
    fn build_auth_request() -> ProviderHttpRequest {
        ProviderHttpRequest {
            method: "GET",
            url: BING_AUTH_URL.to_string(),
            body: None,
            headers: vec![],
        }
    }
}

impl Default for BingProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslateProvider for BingProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "bing",
            name: "Bing（免费）",
            needs_key: false,
            is_unofficial: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        // 默认仅用于「翻译步」；token 由 translate 内部取得后注入 Authorization 头。
        // 单独调 build_request（无 token）会得到无鉴权头的请求——本源只走 translate 两步路径。
        self.build_translate_request(req, "")
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 响应形如 [{"translations":[{"text":"冰川","to":"zh-Hans"}]}]，译文取 [0].translations[0].text。
        let translated = v[0]["translations"][0]["text"]
            .as_str()
            .ok_or_else(|| {
                TranslateError::ParseError("missing [0].translations[0].text".to_string())
            })?
            .to_string();

        Ok(TranslateResponse::plain(translated))
    }

    fn translate(
        &self,
        req: &TranslateRequest,
        executor: &dyn HttpExecutor,
    ) -> Result<TranslateResponse, TranslateError> {
        // 第一步：取 edge auth token（纯文本 JWT）。
        let token = executor.execute(&Self::build_auth_request())?;
        let token = token.trim();
        if token.is_empty() {
            return Err(TranslateError::Auth("Bing edge auth 返回空 token".to_string()));
        }

        // 第二步：带 Bearer token POST 翻译，复用 parse_response 解析译文。
        let translate_req = self.build_translate_request(req, token);
        let raw = executor.execute(&translate_req)?;
        self.parse_response(&raw)
    }
}

impl BingProvider {
    /// 构造翻译步请求（第二步）：注入 from/to 与 Bearer token，body 为 `[{"Text":"..."}]`。
    fn build_translate_request(&self, req: &TranslateRequest, token: &str) -> ProviderHttpRequest {
        let src = map_lang_for_provider("bing", &req.source_lang);
        let tgt = map_lang_for_provider("bing", &req.target_lang);

        // from/to 为 ASCII 语言码无需编码；用 serde_json 构造 body 自动正确转义文本。
        let url = format!("{BING_TRANSLATE_BASE}&from={src}&to={tgt}");
        let body = serde_json::json!([{ "Text": req.text }]).to_string();

        ProviderHttpRequest {
            method: "POST",
            url,
            body: Some(body),
            headers: vec![
                ("Authorization".to_string(), format!("Bearer {token}")),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
        }
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
            is_unofficial: false,
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

        Ok(TranslateResponse::plain(translated))
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

// 百度专业翻译（fieldtranslate）

/// 百度专业（领域）翻译 provider（需要 AppID + SecretKey + 领域 field）。
///
/// 端点（百度翻译开放平台「领域翻译」API 文档）：
/// `POST https://fanyi-api.baidu.com/api/trans/vip/fieldtranslate`
/// 签名算法（官方文档 §领域翻译·签名）：
/// `sign = MD5(appid + q + salt + field + secret_key)`（比基础百度多拼入领域 field）。
/// 请求参数：q/from/to/appid/salt/sign/domain(=field)；
/// 响应取 `trans_result[*].dst`（逐段译文）。
pub struct BaiduFieldProvider {
    app_id: String,
    secret_key: String,
    field: String,
}

impl BaiduFieldProvider {
    /// 构造百度专业翻译 provider。
    pub fn new(app_id: &str, secret_key: &str, field: &str) -> Self {
        Self {
            app_id: app_id.to_string(),
            secret_key: secret_key.to_string(),
            field: field.to_string(),
        }
    }
}

impl TranslateProvider for BaiduFieldProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "baidu_field",
            name: "百度专业翻译",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("baidu_field", &req.source_lang);
        let tgt = map_lang_for_provider("baidu_field", &req.target_lang);

        // 每次请求生成随机 salt 防重放（百度领域翻译 API 文档要求）。
        let salt = uuid::Uuid::new_v4().simple().to_string();
        let sign = baidu_field_sign(&self.app_id, &req.text, &salt, &self.field, &self.secret_key);

        // domain 参数承载领域（field）；q/appid 走 percent-encode。
        let body = format!(
            "q={}&from={}&to={}&appid={}&salt={}&domain={}&sign={}",
            percent_encode(&req.text),
            src,
            tgt,
            percent_encode(&self.app_id),
            salt,
            percent_encode(&self.field),
            sign,
        );

        ProviderHttpRequest {
            method: "POST",
            url: "https://fanyi-api.baidu.com/api/trans/vip/fieldtranslate".to_string(),
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

        // 百度出错时返回 error_code（字符串或数字），复用基础百度错误码归一。
        if let Some(code) = extract_number_or_string(&v["error_code"]) {
            return Err(map_baidu_error(code, &v));
        }

        let translated = concat_baidu_dst(&v)?;
        Ok(TranslateResponse::plain(translated))
    }
}

/// 计算百度专业（领域）翻译请求签名。
///
/// 算法（百度翻译开放平台「领域翻译」API 文档）：
/// `sign = MD5(appid + q + salt + field + secret_key)` 的十六进制小写。
/// 比基础百度签名多拼入领域 field。抽为纯函数以便对任意 salt 直接验证算法正确性。
pub fn baidu_field_sign(appid: &str, q: &str, salt: &str, field: &str, secret_key: &str) -> String {
    let input = format!("{appid}{q}{salt}{field}{secret_key}");
    format!("{:x}", md5::compute(input.as_bytes()))
}

/// 拼接百度响应 `trans_result[*].dst` 各段译文（换行分隔）。
///
/// 百度对多行输入逐段返回译文；空数组或全空译文视为格式错误（ParseError）。
fn concat_baidu_dst(v: &serde_json::Value) -> Result<String, TranslateError> {
    let segments = v["trans_result"]
        .as_array()
        .ok_or_else(|| TranslateError::ParseError("missing trans_result array".to_string()))?;
    let parts: Vec<&str> = segments
        .iter()
        .filter_map(|seg| seg["dst"].as_str())
        .collect();
    if parts.is_empty() {
        return Err(TranslateError::ParseError(
            "empty trans_result dst".to_string(),
        ));
    }
    Ok(parts.join("\n"))
}

// 有道智云翻译（signType=v3）

/// 有道智云翻译 provider（需要应用 ID app_key + 应用密钥 app_secret）。
///
/// 端点（有道智云「自然语言翻译服务」API 文档）：
/// `POST https://openapi.youdao.com/api`，signType=v3。
/// 签名算法（官方文档 §计算签名）：
/// `sign = SHA256(appKey + truncate(q) + salt + curtime + appSecret)` 的十六进制小写，
/// 其中 `truncate(q)`：字符数 ≤ 20 用全文，否则取「前 10 字符 + 字符长度 + 后 10 字符」。
/// 请求参数：q/from/to/appKey/salt/sign/signType=v3/curtime；
/// 响应取 `translation[*]`（逐段译文）。
pub struct YoudaoProvider {
    app_key: String,
    app_secret: String,
}

impl YoudaoProvider {
    /// 构造有道智云翻译 provider。
    pub fn new(app_key: &str, app_secret: &str) -> Self {
        Self {
            app_key: app_key.to_string(),
            app_secret: app_secret.to_string(),
        }
    }
}

impl TranslateProvider for YoudaoProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "youdao",
            name: "有道翻译",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("youdao", &req.source_lang);
        let tgt = map_lang_for_provider("youdao", &req.target_lang);

        // salt 用随机 uuid 防重放；curtime 为当前 Unix 秒（官方文档要求 signType=v3）。
        let salt = uuid::Uuid::new_v4().simple().to_string();
        let curtime = current_unix_secs().to_string();
        let sign = youdao_sign(&self.app_key, &req.text, &salt, &curtime, &self.app_secret);

        let body = format!(
            "q={}&from={}&to={}&appKey={}&salt={}&sign={}&signType=v3&curtime={}",
            percent_encode(&req.text),
            src,
            tgt,
            percent_encode(&self.app_key),
            salt,
            sign,
            curtime,
        );

        ProviderHttpRequest {
            method: "POST",
            url: "https://openapi.youdao.com/api".to_string(),
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

        // 有道用 errorCode 表达错误（"0" 为成功），非 0 归一为 TranslateError。
        if let Some(code) = v["errorCode"].as_str() {
            if code != "0" {
                return Err(map_youdao_error(code));
            }
        }

        let segments = v["translation"]
            .as_array()
            .ok_or_else(|| TranslateError::ParseError("missing translation array".to_string()))?;
        let parts: Vec<&str> = segments.iter().filter_map(|s| s.as_str()).collect();
        if parts.is_empty() {
            return Err(TranslateError::ParseError("empty translation".to_string()));
        }
        Ok(TranslateResponse::plain(parts.join("\n")))
    }
}

/// 计算有道 signType=v3 请求签名。
///
/// 算法（有道智云 API 文档 §计算签名）：
/// `sign = SHA256(appKey + truncate(q) + salt + curtime + appSecret)` 十六进制小写。
/// 抽为纯函数：对固定 salt+curtime 可断言确定 SHA256 值，使签名算法可单测验证。
pub fn youdao_sign(app_key: &str, q: &str, salt: &str, curtime: &str, app_secret: &str) -> String {
    use sha2::{Digest, Sha256};
    let input = format!("{app_key}{}{salt}{curtime}{app_secret}", youdao_truncate(q));
    let digest = Sha256::digest(input.as_bytes());
    format!("{digest:x}")
}

/// 有道签名 `truncate(q)` 规则（官方文档 §计算签名）。
///
/// 按**字符数**判断（非字节）：长度 ≤ 20 返回全文；否则返回「前 10 字符 + 字符长度 + 后 10 字符」。
/// 用 `chars()` 取字符避免中文按字节切分 panic。
pub fn youdao_truncate(q: &str) -> String {
    let chars: Vec<char> = q.chars().collect();
    let len = chars.len();
    if len <= 20 {
        return q.to_string();
    }
    let prefix: String = chars[..10].iter().collect();
    let suffix: String = chars[len - 10..].iter().collect();
    format!("{prefix}{len}{suffix}")
}

/// 返回当前 Unix 时间戳（秒）。
///
/// 隔离真实时间读取，使 `youdao_sign` 保持纯函数、可对固定 curtime 测确定签名。
fn current_unix_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 将有道 errorCode（非 "0"）归一为 `TranslateError`。
///
/// 错误码来源：有道智云 API 文档 §错误代码列表。
fn map_youdao_error(code: &str) -> TranslateError {
    match code {
        "101" | "102" | "103" | "104" | "105" | "106" | "107" => {
            TranslateError::Unsupported(format!("有道翻译请求参数错误 {code}"))
        }
        "108" | "110" | "111" | "202" | "401" => {
            TranslateError::Auth(format!("有道翻译密钥/账户错误 {code}"))
        }
        "207" | "303" => TranslateError::ServerError(format!("有道翻译服务端错误 {code}")),
        "302" | "411" | "412" => TranslateError::RateLimit(format!("有道翻译频率超限 {code}")),
        "402" => TranslateError::Quota(format!("有道翻译余额不足 {code}")),
        "206" => TranslateError::TooLong(format!("有道翻译文本过长 {code}")),
        _ => TranslateError::ServerError(format!("有道翻译错误 {code}")),
    }
}

// 有道词典模式（同有道翻译签名，isWord 模式返回词条）

/// 有道词典模式 provider（需要应用 ID app_key + 应用密钥 app_secret，同有道翻译 key）。
///
/// 端点与签名完全复用有道翻译（signType=v3，`youdao_sign`）；区别在于响应解析：
/// 有道 `/api` 对单词查询会附带 `isWord:true` 与 `basic` 词条块。
/// - `isWord===true` 且含 `basic`：取 basic 的音标（优先 us-phonetic→phonetic→uk-phonetic）、
///   `explains`（按词性前缀分组为释义）、`wfs`（词形变化）→ `Dict(DictEntry)`。
/// - 否则（非词或无 basic）：回退取 `translation[*]` 拼接为 `Plain`。
///
/// 字段来源：有道智云「自然语言翻译服务」API 文档 §词典结果（basic/wfs/explains）。
pub struct YoudaoDictProvider {
    app_key: String,
    app_secret: String,
}

impl YoudaoDictProvider {
    /// 构造有道词典模式 provider。
    pub fn new(app_key: &str, app_secret: &str) -> Self {
        Self {
            app_key: app_key.to_string(),
            app_secret: app_secret.to_string(),
        }
    }
}

impl TranslateProvider for YoudaoDictProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "youdao_dict",
            name: "有道词典",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("youdao", &req.source_lang);
        let tgt = map_lang_for_provider("youdao", &req.target_lang);

        // 完全复用有道翻译签名（signType=v3）：salt 随机、curtime 当前秒、SHA256 签名。
        let salt = uuid::Uuid::new_v4().simple().to_string();
        let curtime = current_unix_secs().to_string();
        let sign = youdao_sign(&self.app_key, &req.text, &salt, &curtime, &self.app_secret);

        let body = format!(
            "q={}&from={}&to={}&appKey={}&salt={}&sign={}&signType=v3&curtime={}",
            percent_encode(&req.text),
            src,
            tgt,
            percent_encode(&self.app_key),
            salt,
            sign,
            curtime,
        );

        ProviderHttpRequest {
            method: "POST",
            url: "https://openapi.youdao.com/api".to_string(),
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

        // 错误码归一复用有道翻译（"0" 成功）。
        if let Some(code) = v["errorCode"].as_str() {
            if code != "0" {
                return Err(map_youdao_error(code));
            }
        }

        // isWord===true 且含 basic 块时解析为词条；否则回退普通译文。
        let is_word = v["isWord"].as_bool().unwrap_or(false);
        if is_word && v["basic"].is_object() {
            return Ok(TranslateResponse::Dict {
                entry: parse_youdao_basic(&v["basic"]),
            });
        }

        // 回退：取 translation 数组拼接为 Plain（与有道翻译解析一致）。
        let segments = v["translation"]
            .as_array()
            .ok_or_else(|| TranslateError::ParseError("missing translation array".to_string()))?;
        let parts: Vec<&str> = segments.iter().filter_map(|s| s.as_str()).collect();
        if parts.is_empty() {
            return Err(TranslateError::ParseError("empty translation".to_string()));
        }
        Ok(TranslateResponse::plain(parts.join("\n")))
    }
}

/// 把有道 `basic` 词条块解析为 `DictEntry`。
///
/// 字段映射（有道智云 API 文档 §词典结果）：
/// - 音标：优先 `us-phonetic`，缺则 `phonetic`，再缺则 `uk-phonetic`。
/// - 释义：`explains` 各项按词性前缀（如 "n."）分组为 `PosDefinition`。
/// - 词形：`wfs[*].wf.value`（如复数 glaciers）。
/// - 发音：有道 basic 不直接给音频 URL，留空。
fn parse_youdao_basic(basic: &serde_json::Value) -> DictEntry {
    let phonetic = ["us-phonetic", "phonetic", "uk-phonetic"]
        .iter()
        .find_map(|key| basic[*key].as_str())
        .map(str::to_string);

    let explains = basic["explains"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|e| e.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    let definitions = group_definitions_by_pos(&explains);

    let inflections = basic["wfs"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let wf = &item["wf"];
                    let name = wf["name"].as_str();
                    let value = wf["value"].as_str()?;
                    Some(match name {
                        Some(n) => format!("{n}: {value}"),
                        None => value.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    DictEntry {
        phonetic,
        definitions,
        examples: vec![],
        audio: None,
        inflections,
    }
}

/// 把一组释义字符串按开头的词性前缀（如 "n."、"vt."）分组为 `PosDefinition`。
///
/// 识别规则：取首个空白前的 token，若以英文字母+点号结尾（如 `n.`/`vt.`/`adj.`）视为词性，
/// 其余作为该词性下的释义；无可识别词性时 pos 留空、整条作为释义。
/// 同一词性的多条释义合并到同一分组，保持出现顺序。
fn group_definitions_by_pos(explains: &[&str]) -> Vec<PosDefinition> {
    let mut groups: Vec<PosDefinition> = Vec::new();
    for explain in explains {
        let (pos, meaning) = split_pos_prefix(explain);
        match groups.iter_mut().find(|g| g.pos.as_deref() == pos.as_deref()) {
            Some(group) => group.meanings.push(meaning),
            None => groups.push(PosDefinition {
                pos,
                meanings: vec![meaning],
            }),
        }
    }
    groups
}

/// 从释义文本切出词性前缀与剩余释义。
///
/// 词性前缀定义：首 token 由 ASCII 字母组成并以 `.` 结尾（如 `n.`/`vt.`/`adj.`）。
/// 命中则返回 `(Some(pos), 余下释义)`；否则 `(None, 原文)`。
fn split_pos_prefix(explain: &str) -> (Option<String>, String) {
    let trimmed = explain.trim();
    if let Some((head, rest)) = trimmed.split_once(char::is_whitespace) {
        if is_pos_token(head) {
            return (Some(head.to_string()), rest.trim().to_string());
        }
    }
    (None, trimmed.to_string())
}

/// 判断 token 是否为词性前缀：以 `.` 结尾、点号前全为 ASCII 字母且非空。
fn is_pos_token(token: &str) -> bool {
    matches!(token.strip_suffix('.'), Some(body) if !body.is_empty() && body.chars().all(|c| c.is_ascii_alphabetic()))
}

// ECDICT 英汉词典（pot-app.com/api/dict POST，免 key，pot 自建公共服务）

/// ECDICT 词典 provider（pot-app.com/api/dict POST，免 key）。
///
/// 端点（pot 自建公共服务，按公开接口形态独立实现，不参考任何第三方源码）：
/// `POST https://pot-app.com/api/dict`，JSON body `{"word":"..."}`，
/// 响应为 ECDICT 数据库行：`{word, phonetic, translation, definition, exchange, ...}`。
/// - 音标取 `phonetic`。
/// - `translation`（英汉释义，多词性按换行分隔）按词性前缀分组为释义。
/// - `exchange`（如 `s:glaciers/p:glacial`）解析为词形列表（取各项 `:` 后的值）。
///
/// pot 自建公共服务、随对方改版即可能失效，标 is_unofficial=true（同 lingva 处理）。
pub struct EcdictProvider;

impl EcdictProvider {
    /// 构造 ECDICT provider（无凭据）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for EcdictProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslateProvider for EcdictProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "ecdict",
            name: "ECDICT 英汉词典",
            needs_key: false,
            is_unofficial: true,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        // ECDICT 只查英文词，待查词置于 JSON body 的 word 字段；
        // 用 serde_json 构造自动正确转义（避免手拼 JSON 注入风险）。
        let body = serde_json::json!({ "word": req.text }).to_string();

        ProviderHttpRequest {
            method: "POST",
            url: "https://pot-app.com/api/dict".to_string(),
            body: Some(body),
            headers: vec![(
                "Content-Type".to_string(),
                "application/json".to_string(),
            )],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 未收录/非词时 word 与 translation 均空，视为无结果（ParseError，不 panic）。
        let translation = v["translation"].as_str().unwrap_or("").trim();
        if translation.is_empty() {
            return Err(TranslateError::ParseError(
                "ECDICT 未收录该词或返回空词条".to_string(),
            ));
        }

        let phonetic = v["phonetic"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        // translation 各行为一条释义，按词性前缀分组。
        let explains: Vec<&str> = translation.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let definitions = group_definitions_by_pos(&explains);

        let inflections = parse_ecdict_exchange(v["exchange"].as_str().unwrap_or(""));

        Ok(TranslateResponse::Dict {
            entry: DictEntry {
                phonetic,
                definitions,
                examples: vec![],
                audio: None,
                inflections,
            },
        })
    }
}

/// 解析 ECDICT `exchange` 字段为词形列表。
///
/// 格式（ECDICT 约定）：`类型:值` 以 `/` 分隔，如 `s:glaciers/p:glacial/3:glaciates`。
/// 取各项 `:` 后的值作为词形；无 `:` 的项原样保留。空串返回空列表。
fn parse_ecdict_exchange(exchange: &str) -> Vec<String> {
    exchange
        .split('/')
        .filter_map(|item| {
            let value = item.split_once(':').map(|(_, v)| v).unwrap_or(item);
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        })
        .collect()
}

// 彩云小译（token 简单鉴权）

/// 彩云小译翻译 provider（需要 token）。
///
/// 端点（彩云小译开放平台 API 文档 https://docs.caiyunapp.com/blog/2018/09/03/translator/）：
/// `POST https://api.interpreter.caiyunai.com/v1/translator`
/// 鉴权（官方文档「请求头」）：`x-authorization: token {token}`、`content-type: application/json`。
/// 请求 body（官方文档「请求参数」）：`{"source":["原文"],"trans_type":"{src}2{tgt}","request_id":"...","detect":true}`，
/// 其中 source 为原文数组，trans_type 形如 `en2zh`/`auto2zh`（源2目标）。
/// 响应（官方文档「返回结果」）取 `target`（与 source 逐项对应的译文数组）首项。
pub struct CaiyunProvider {
    token: String,
}

impl CaiyunProvider {
    /// 构造彩云小译 provider。
    pub fn new(token: &str) -> Self {
        Self {
            token: token.to_string(),
        }
    }
}

impl TranslateProvider for CaiyunProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "caiyun",
            name: "彩云小译",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("caiyun", &req.source_lang);
        let tgt = map_lang_for_provider("caiyun", &req.target_lang);

        // trans_type 为「源2目标」形态（官方文档约定，如 en2zh）；request_id 仅作幂等标识，用随机 uuid。
        // 用 serde_json::json! 构造 body 自动正确转义文本（避免手拼 JSON 注入风险）。
        let body = serde_json::json!({
            "source": [req.text],
            "trans_type": format!("{src}2{tgt}"),
            "request_id": uuid::Uuid::new_v4().simple().to_string(),
            "detect": true,
        })
        .to_string();

        ProviderHttpRequest {
            method: "POST",
            url: "https://api.interpreter.caiyunai.com/v1/translator".to_string(),
            body: Some(body),
            headers: vec![
                (
                    "x-authorization".to_string(),
                    format!("token {}", self.token),
                ),
                ("content-type".to_string(), "application/json".to_string()),
            ],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 错误响应：彩云用 message 字段携带错误文案（无 target）；归一为 Auth/ServerError。
        if v["target"].is_null() {
            if let Some(msg) = v["message"].as_str() {
                return Err(map_caiyun_error(msg));
            }
            return Err(TranslateError::ParseError("missing target".to_string()));
        }

        // target 主形态为数组（与 source 逐项对应），取首项；兼容单字符串形态。
        let translated = match &v["target"] {
            serde_json::Value::Array(arr) => arr
                .first()
                .and_then(|t| t.as_str())
                .ok_or_else(|| TranslateError::ParseError("empty target array".to_string()))?
                .to_string(),
            serde_json::Value::String(s) => s.clone(),
            _ => {
                return Err(TranslateError::ParseError(
                    "target 非数组或字符串".to_string(),
                ))
            }
        };

        Ok(TranslateResponse::plain(translated))
    }
}

/// 将彩云错误响应 message 归一为 `TranslateError`。
///
/// 彩云用 HTTP 状态码 + body message 表达错误；此处按 message 文案归类
/// （token 相关→Auth，其余→ServerError）。文案来源：彩云小译 API 文档错误说明。
fn map_caiyun_error(msg: &str) -> TranslateError {
    let lower = msg.to_ascii_lowercase();
    if lower.contains("token") || lower.contains("auth") || lower.contains("unauthorized") {
        TranslateError::Auth(format!("彩云小译鉴权失败: {msg}"))
    } else {
        TranslateError::ServerError(format!("彩云小译错误: {msg}"))
    }
}

// 小牛翻译（body apikey 鉴权）

/// 小牛翻译 provider（需要 apikey）。
///
/// 端点（小牛翻译开放平台 API 文档 https://niutrans.com/documents/contents/trans_text）：
/// `POST https://api.niutrans.com/NiuTransServer/translation`，`content-type: application/json`。
/// 请求 body（官方文档「请求参数」）：`{"from":"{src}","to":"{tgt}","apikey":"{apikey}","src_text":"原文"}`，
/// apikey 直接置于请求体内（非请求头）。
/// 响应（官方文档「返回结果」）成功取 `tgt_text`；失败返回 `error_code`/`error_msg`。
pub struct NiutransProvider {
    apikey: String,
}

impl NiutransProvider {
    /// 构造小牛翻译 provider。
    pub fn new(apikey: &str) -> Self {
        Self {
            apikey: apikey.to_string(),
        }
    }
}

impl TranslateProvider for NiutransProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "niutrans",
            name: "小牛翻译",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("niutrans", &req.source_lang);
        let tgt = map_lang_for_provider("niutrans", &req.target_lang);

        // apikey 置于 body（官方文档约定，非请求头）；用 serde_json 构造自动正确转义文本。
        let body = serde_json::json!({
            "from": src,
            "to": tgt,
            "apikey": self.apikey,
            "src_text": req.text,
        })
        .to_string();

        ProviderHttpRequest {
            method: "POST",
            url: "https://api.niutrans.com/NiuTransServer/translation".to_string(),
            body: Some(body),
            headers: vec![("content-type".to_string(), "application/json".to_string())],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 错误响应：小牛用 error_code（字符串或数字）携带错误码（无 tgt_text）。
        if let Some(code) = extract_number_or_string(&v["error_code"]) {
            let msg = v["error_msg"].as_str().unwrap_or("unknown");
            return Err(map_niutrans_error(code, msg));
        }

        let translated = v["tgt_text"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing tgt_text".to_string()))?
            .to_string();

        Ok(TranslateResponse::plain(translated))
    }
}

/// 将小牛 error_code 归一为 `TranslateError`。
///
/// 错误码来源：小牛翻译 API 文档「错误码」列表（如 13001 apikey 错误、19001 余额不足）。
fn map_niutrans_error(code: u64, msg: &str) -> TranslateError {
    match code {
        13001 | 13002 | 13003 | 13007 => {
            TranslateError::Auth(format!("小牛翻译鉴权失败 {code}: {msg}"))
        }
        14001 | 14002 => TranslateError::TooLong(format!("小牛翻译文本过长 {code}: {msg}")),
        19001 | 19002 => TranslateError::Quota(format!("小牛翻译余额不足 {code}: {msg}")),
        17001 => TranslateError::RateLimit(format!("小牛翻译频率超限 {code}: {msg}")),
        _ => TranslateError::ServerError(format!("小牛翻译错误 {code}: {msg}")),
    }
}

// 腾讯云 TMT（TC3-HMAC-SHA256 三层密钥派生）

/// 腾讯云机器翻译（TMT）端点与签名常量（按腾讯云官方文档，非 pot 源码）。
///
/// 文档来源：
/// - 接口：cloud.tencent.com/document/api/551/15619（TextTranslate）
/// - 签名 v3：cloud.tencent.com/document/api/551/30637（TC3-HMAC-SHA256）
const TENCENT_HOST: &str = "tmt.tencentcloudapi.com";
const TENCENT_SERVICE: &str = "tmt";
const TENCENT_ACTION: &str = "TextTranslate";
const TENCENT_VERSION: &str = "2018-03-21";
const TENCENT_REGION: &str = "ap-guangzhou";
/// 腾讯云 TMT 请求 Content-Type（签名 CanonicalHeaders 与实际头须逐字一致）。
const TENCENT_CONTENT_TYPE: &str = "application/json; charset=utf-8";

/// 腾讯云机器翻译 provider（需要 SecretId + SecretKey）。
///
/// 端点：`POST https://tmt.tencentcloudapi.com`，签名 TC3-HMAC-SHA256。
/// Action=TextTranslate、Version=2018-03-21、Region=ap-guangzhou。
/// 响应取 `Response.TargetText`；`Response.Error` 表错误。
pub struct TencentProvider {
    secret_id: String,
    secret_key: String,
}

impl TencentProvider {
    /// 构造腾讯云翻译 provider。
    pub fn new(secret_id: &str, secret_key: &str) -> Self {
        Self {
            secret_id: secret_id.to_string(),
            secret_key: secret_key.to_string(),
        }
    }

    /// 按腾讯云约定构造请求 body（SourceText/Source/Target/ProjectId）。
    ///
    /// 用 serde_json 构造，自动正确转义文本，并保证字段顺序稳定
    /// （签名对 body 求 SHA256，body 文本须与实际发送逐字一致）。
    fn build_body(req: &TranslateRequest) -> String {
        let src = map_lang_for_provider("tencent", &req.source_lang);
        let tgt = map_lang_for_provider("tencent", &req.target_lang);
        serde_json::json!({
            "SourceText": req.text,
            "Source": src,
            "Target": tgt,
            "ProjectId": 0,
        })
        .to_string()
    }
}

impl TranslateProvider for TencentProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "tencent",
            name: "腾讯云翻译",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let body = Self::build_body(req);
        let timestamp = current_unix_secs() as i64;
        let authorization = tencent_tc3_sign(&self.secret_id, &self.secret_key, &body, timestamp);

        ProviderHttpRequest {
            method: "POST",
            url: format!("https://{TENCENT_HOST}"),
            body: Some(body),
            headers: vec![
                ("Authorization".to_string(), authorization),
                ("Content-Type".to_string(), TENCENT_CONTENT_TYPE.to_string()),
                ("Host".to_string(), TENCENT_HOST.to_string()),
                ("X-TC-Action".to_string(), TENCENT_ACTION.to_string()),
                ("X-TC-Version".to_string(), TENCENT_VERSION.to_string()),
                ("X-TC-Region".to_string(), TENCENT_REGION.to_string()),
                ("X-TC-Timestamp".to_string(), timestamp.to_string()),
            ],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 腾讯云用 Response.Error 表错误（Code 形如 "AuthFailure.SignatureFailure"）。
        let error = &v["Response"]["Error"];
        if !error.is_null() {
            let code = error["Code"].as_str().unwrap_or("Unknown");
            let msg = error["Message"].as_str().unwrap_or("unknown");
            return Err(map_tencent_error(code, msg));
        }

        let translated = v["Response"]["TargetText"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing Response.TargetText".to_string()))?
            .to_string();

        Ok(TranslateResponse::plain(translated))
    }
}

/// 计算腾讯云 TC3-HMAC-SHA256 签名，返回完整 `Authorization` 头值。
///
/// 算法（腾讯云签名 v3 官方文档 cloud.tencent.com/document/api/551/30637，非 pot 源码）：
/// 1. 拼 CanonicalRequest = method\n uri\n query\n canonicalHeaders\n signedHeaders\n SHA256(payload)
/// 2. 拼 StringToSign = "TC3-HMAC-SHA256"\n timestamp\n credentialScope\n SHA256(CanonicalRequest)
/// 3. 三层密钥派生：HMAC(HMAC(HMAC("TC3"+secretKey, date), service), "tc3_request")
/// 4. Signature = hex(HMAC(signingKey, StringToSign))，组装 Authorization 头
///
/// 抽为纯函数：对固定 secret/timestamp/payload 可断言确定 Authorization，使签名可单测验证。
/// `timestamp` 为 Unix 秒；CanonicalHeaders 固定含 content-type/host/x-tc-action（与实际发送头一致）。
pub fn tencent_tc3_sign(secret_id: &str, secret_key: &str, payload: &str, timestamp: i64) -> String {
    use sha2::{Digest, Sha256};

    let date = unix_secs_to_utc_date(timestamp);

    // 步骤 1：CanonicalRequest（POST、根路径、空 query、固定签名头集）。
    let canonical_headers = format!(
        "content-type:{}\nhost:{}\nx-tc-action:{}\n",
        TENCENT_CONTENT_TYPE,
        TENCENT_HOST,
        TENCENT_ACTION.to_ascii_lowercase(),
    );
    let signed_headers = "content-type;host;x-tc-action";
    let hashed_payload = format!("{:x}", Sha256::digest(payload.as_bytes()));
    let canonical_request = format!(
        "POST\n/\n\n{canonical_headers}\n{signed_headers}\n{hashed_payload}"
    );

    // 步骤 2：StringToSign。
    let credential_scope = format!("{date}/{TENCENT_SERVICE}/tc3_request");
    let hashed_canonical = format!("{:x}", Sha256::digest(canonical_request.as_bytes()));
    let string_to_sign = format!(
        "TC3-HMAC-SHA256\n{timestamp}\n{credential_scope}\n{hashed_canonical}"
    );

    // 步骤 3：三层密钥派生。
    let secret_date = hmac_sha256(format!("TC3{secret_key}").as_bytes(), date.as_bytes());
    let secret_service = hmac_sha256(&secret_date, TENCENT_SERVICE.as_bytes());
    let secret_signing = hmac_sha256(&secret_service, b"tc3_request");

    // 步骤 4：Signature 与 Authorization 头。
    let signature = hmac_sha256(&secret_signing, string_to_sign.as_bytes());
    let signature_hex = to_hex_lower(&signature);

    format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, \
SignedHeaders={signed_headers}, Signature={signature_hex}"
    )
}

/// 将腾讯云 `Response.Error.Code` 归一为 `TranslateError`。
///
/// 错误码来源：腾讯云公共错误码 + TMT 业务错误码官方文档（cloud.tencent.com/document/api/551/30640）。
/// 按 Code 前缀/关键词归类（鉴权→Auth、限流→RateLimit、配额→Quota）。
fn map_tencent_error(code: &str, msg: &str) -> TranslateError {
    if code.starts_with("AuthFailure") || code == "UnauthorizedOperation" {
        TranslateError::Auth(format!("腾讯云翻译鉴权失败 {code}: {msg}"))
    } else if code == "RequestLimitExceeded" || code.starts_with("RequestLimitExceeded") {
        TranslateError::RateLimit(format!("腾讯云翻译频率超限 {code}: {msg}"))
    } else if code.contains("Resource") && code.contains("Insufficient") {
        TranslateError::Quota(format!("腾讯云翻译资源不足 {code}: {msg}"))
    } else if code.starts_with("UnsupportedOperation") {
        TranslateError::Unsupported(format!("腾讯云翻译不支持 {code}: {msg}"))
    } else {
        TranslateError::ServerError(format!("腾讯云翻译错误 {code}: {msg}"))
    }
}

// 阿里翻译（HMAC-SHA1 + Base64，RPC 风格签名）

/// 阿里云机器翻译端点与签名常量（按阿里云官方文档，非 pot 源码）。
///
/// 文档来源：
/// - 接口：help.aliyun.com/document_detail/158244（机器翻译·获取翻译·通用版）
/// - RPC 签名：help.aliyun.com/document_detail/30563（签名机制 HMAC-SHA1 + Base64）
const ALIBABA_HOST: &str = "mt.cn-hangzhou.aliyuncs.com";
const ALIBABA_ACTION: &str = "TranslateGeneral";
const ALIBABA_VERSION: &str = "2018-10-12";

/// 阿里云机器翻译 provider（需要 AccessKeyId + AccessKeySecret）。
///
/// 端点：`GET http://mt.cn-hangzhou.aliyuncs.com/`，RPC 风格 HMAC-SHA1 + Base64 签名。
/// Action=TranslateGeneral、Version=2018-10-12。响应取 `Data.Translated`；`Code != 200` 表错误。
pub struct AlibabaProvider {
    access_key_id: String,
    access_key_secret: String,
}

impl AlibabaProvider {
    /// 构造阿里云翻译 provider。
    pub fn new(access_key_id: &str, access_key_secret: &str) -> Self {
        Self {
            access_key_id: access_key_id.to_string(),
            access_key_secret: access_key_secret.to_string(),
        }
    }
}

impl TranslateProvider for AlibabaProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "alibaba",
            name: "阿里翻译",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let src = map_lang_for_provider("alibaba", &req.source_lang);
        let tgt = map_lang_for_provider("alibaba", &req.target_lang);

        // SignatureNonce 防重放，用随机 uuid；Timestamp 为 ISO8601 UTC（官方要求）。
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let timestamp = unix_secs_to_iso8601_utc(current_unix_secs() as i64);

        // RPC 公共参数 + 业务参数（不含 Signature；签名后再追加）。
        let params: Vec<(&str, &str)> = vec![
            ("AccessKeyId", &self.access_key_id),
            ("Action", ALIBABA_ACTION),
            ("Format", "JSON"),
            ("FormatType", "text"),
            ("Scene", "general"),
            ("SignatureMethod", "HMAC-SHA1"),
            ("SignatureNonce", &nonce),
            ("SignatureVersion", "1.0"),
            ("SourceLanguage", &src),
            ("SourceText", &req.text),
            ("TargetLanguage", &tgt),
            ("Timestamp", &timestamp),
            ("Version", ALIBABA_VERSION),
        ];

        let signature = alibaba_hmac_sign("GET", &params, &self.access_key_secret);

        // 拼 query：所有签名参数 + Signature，值统一 RFC3986 编码。
        let mut query = params
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        query.push_str(&format!("&Signature={}", percent_encode(&signature)));

        ProviderHttpRequest {
            method: "GET",
            url: format!("http://{ALIBABA_HOST}/?{query}"),
            body: None,
            headers: vec![],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 阿里用 Code 表状态："200" 成功；非 200（数字或错误码字符串）表错误。
        let code = v["Code"].as_str().unwrap_or("");
        if code != "200" {
            let msg = v["Message"].as_str().unwrap_or("unknown");
            return Err(map_alibaba_error(code, msg));
        }

        let translated = v["Data"]["Translated"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing Data.Translated".to_string()))?
            .to_string();

        Ok(TranslateResponse::plain(translated))
    }
}

/// 计算阿里云 RPC 风格 HMAC-SHA1 签名，返回 Base64 字符串。
///
/// 算法（阿里云签名机制官方文档 help.aliyun.com/document_detail/30563，非 pot 源码）：
/// 1. 按参数名字典序排序，RFC3986 编码每个 key/value，拼 `k1=v1&k2=v2...` 规范化查询串
/// 2. StringToSign = `METHOD + "&" + encode("/") + "&" + encode(规范化查询串)`
/// 3. Signature = `Base64(HMAC-SHA1(accessKeySecret + "&", StringToSign))`
///
/// 抽为纯函数：对固定参数集可断言确定 Base64 签名，使签名可单测验证。
/// `params` 不含 Signature 本身（签名结果由调用方追加进 query）。
pub fn alibaba_hmac_sign(method: &str, params: &[(&str, &str)], access_key_secret: &str) -> String {
    use base64::Engine;

    // 步骤 1：按 key 字典序排序后 RFC3986 编码拼规范化查询串。
    let mut sorted: Vec<&(&str, &str)> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let canonical = sorted
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    // 步骤 2：StringToSign（method & encode("/") & encode(canonical)）。
    let string_to_sign = format!(
        "{}&{}&{}",
        method,
        percent_encode("/"),
        percent_encode(&canonical),
    );

    // 步骤 3：HMAC-SHA1(secret + "&", stringToSign) 后 Base64。
    let signature = hmac_sha1(
        format!("{access_key_secret}&").as_bytes(),
        string_to_sign.as_bytes(),
    );
    base64::engine::general_purpose::STANDARD.encode(signature)
}

/// 将阿里云错误 `Code` 归一为 `TranslateError`。
///
/// 错误码来源：阿里云机器翻译 API 错误码官方文档（help.aliyun.com/document_detail/158244）。
/// 按 Code 关键词归类（鉴权→Auth、限流→RateLimit、配额→Quota）。
fn map_alibaba_error(code: &str, msg: &str) -> TranslateError {
    if code.contains("AccessKey") || code.contains("Signature") || code.contains("Forbidden") {
        TranslateError::Auth(format!("阿里翻译鉴权失败 {code}: {msg}"))
    } else if code.starts_with("Throttling") {
        TranslateError::RateLimit(format!("阿里翻译频率超限 {code}: {msg}"))
    } else if code.contains("Quota") || code.contains("Arrearage") {
        TranslateError::Quota(format!("阿里翻译配额不足 {code}: {msg}"))
    } else {
        TranslateError::ServerError(format!("阿里翻译错误 {code}: {msg}"))
    }
}

// 火山引擎翻译（AWS SigV4 风格四层 HMAC-SHA256）

/// 火山引擎机器翻译端点与签名常量（按火山引擎官方文档，非 pot 源码）。
///
/// 文档来源：
/// - 接口：volcengine.com/docs/4640/65067（机器翻译 TranslateText）
/// - 签名 V4：volcengine.com/docs/6369/67269（签名方法 V4，AWS SigV4 风格）
const VOLCENGINE_HOST: &str = "open.volcengineapi.com";
const VOLCENGINE_SERVICE: &str = "translate";
const VOLCENGINE_ACTION: &str = "TranslateText";
const VOLCENGINE_VERSION: &str = "2020-06-01";
/// 默认地域（用户未在凭据 region 字段填写时采用）。
const VOLCENGINE_DEFAULT_REGION: &str = "cn-north-1";
/// 火山请求 Content-Type（签名 CanonicalHeaders 与实际头须逐字一致）。
const VOLCENGINE_CONTENT_TYPE: &str = "application/json";

/// 火山引擎机器翻译 provider（需要 AccessKeyId + SecretAccessKey + Region）。
///
/// 端点：`POST https://open.volcengineapi.com/?Action=TranslateText&Version=2020-06-01`，
/// 签名 AWS SigV4 风格四层 HMAC-SHA256。响应取 `TranslationList[].Translation` 拼接；
/// `ResponseMetadata.Error` 表错误。
pub struct VolcengineProvider {
    access_key_id: String,
    secret_access_key: String,
    region: String,
}

impl VolcengineProvider {
    /// 构造火山引擎翻译 provider。
    pub fn new(access_key_id: &str, secret_access_key: &str, region: &str) -> Self {
        Self {
            access_key_id: access_key_id.to_string(),
            secret_access_key: secret_access_key.to_string(),
            region: region.to_string(),
        }
    }

    /// 按火山约定构造请求 body（SourceLanguage/TargetLanguage/TextList）。
    ///
    /// 用 serde_json 构造，自动正确转义文本；body 对 SHA256 求 payloadHash，
    /// 须与实际发送逐字一致。
    fn build_body(req: &TranslateRequest) -> String {
        let src = map_lang_for_provider("volcengine", &req.source_lang);
        let tgt = map_lang_for_provider("volcengine", &req.target_lang);
        serde_json::json!({
            "SourceLanguage": src,
            "TargetLanguage": tgt,
            "TextList": [req.text],
        })
        .to_string()
    }
}

impl TranslateProvider for VolcengineProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "volcengine",
            name: "火山翻译",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let body = Self::build_body(req);
        let timestamp = current_unix_secs() as i64;
        let x_date = unix_secs_to_compact_utc(timestamp);
        let payload_hash = to_hex_lower(&sha256_digest(body.as_bytes()));
        let authorization = volcengine_sigv4_sign(
            &self.access_key_id,
            &self.secret_access_key,
            &self.region,
            &body,
            timestamp,
        );

        ProviderHttpRequest {
            method: "POST",
            url: format!(
                "https://{VOLCENGINE_HOST}/?Action={VOLCENGINE_ACTION}&Version={VOLCENGINE_VERSION}"
            ),
            body: Some(body),
            headers: vec![
                ("Authorization".to_string(), authorization),
                ("Content-Type".to_string(), VOLCENGINE_CONTENT_TYPE.to_string()),
                ("Host".to_string(), VOLCENGINE_HOST.to_string()),
                ("X-Date".to_string(), x_date),
                ("X-Content-Sha256".to_string(), payload_hash),
            ],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 火山用 ResponseMetadata.Error 表错误（Code 形如 "SignatureDoesNotMatch"）。
        let error = &v["ResponseMetadata"]["Error"];
        if !error.is_null() {
            let code = error["Code"].as_str().unwrap_or("Unknown");
            let msg = error["Message"].as_str().unwrap_or("unknown");
            return Err(map_volcengine_error(code, msg));
        }

        let list = v["TranslationList"]
            .as_array()
            .ok_or_else(|| TranslateError::ParseError("missing TranslationList".to_string()))?;
        let translated = list
            .iter()
            .filter_map(|item| item["Translation"].as_str())
            .collect::<Vec<_>>()
            .join("");
        if translated.is_empty() {
            return Err(TranslateError::ParseError(
                "TranslationList 无 Translation 文本".to_string(),
            ));
        }

        Ok(TranslateResponse::plain(translated))
    }
}

/// 计算火山引擎 SigV4 风格签名，返回完整 `Authorization` 头值。
///
/// 算法（火山引擎签名方法 V4 官方文档 volcengine.com/docs/6369/67269，非 pot 源码）：
/// 1. 拼 CanonicalRequest = method\n uri\n query\n canonicalHeaders\n signedHeaders\n SHA256(payload)
/// 2. 拼 StringToSign = "HMAC-SHA256"\n xDate\n credentialScope\n SHA256(CanonicalRequest)
/// 3. 四层密钥派生：HMAC(HMAC(HMAC(HMAC(secretAccessKey, date), region), service), "request")
/// 4. Signature = hex(HMAC(signingKey, StringToSign))，组装 Authorization 头
///
/// 与 AWS SigV4 区别（火山特有）：算法标识 `HMAC-SHA256`、credentialScope 以 `request` 结尾、
/// 签名密钥首层直接用 secretAccessKey（不加 "AWS4" 前缀）、X-Date 为紧凑 `YYYYMMDDThhmmssZ`。
///
/// 抽为纯函数：对固定 secret/region/payload/timestamp 可断言确定 Authorization，使签名可单测验证。
/// `timestamp` 为 Unix 秒；CanonicalHeaders 固定含 content-type/host/x-content-sha256/x-date
/// （与实际发送头一致、按字母序）。
pub fn volcengine_sigv4_sign(
    access_key_id: &str,
    secret_access_key: &str,
    region: &str,
    payload: &str,
    timestamp: i64,
) -> String {
    let x_date = unix_secs_to_compact_utc(timestamp);
    let short_date = unix_secs_to_compact_date(timestamp);
    let payload_hash = to_hex_lower(&sha256_digest(payload.as_bytes()));

    // 步骤 1：CanonicalRequest（POST、根路径、固定 query、签名头集按字母序）。
    let canonical_query =
        format!("Action={VOLCENGINE_ACTION}&Version={VOLCENGINE_VERSION}");
    let canonical_headers = format!(
        "content-type:{VOLCENGINE_CONTENT_TYPE}\nhost:{VOLCENGINE_HOST}\nx-content-sha256:{payload_hash}\nx-date:{x_date}\n"
    );
    let signed_headers = "content-type;host;x-content-sha256;x-date";
    let canonical_request = format!(
        "POST\n/\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );

    // 步骤 2：StringToSign。
    let credential_scope = format!("{short_date}/{region}/{VOLCENGINE_SERVICE}/request");
    let hashed_canonical = to_hex_lower(&sha256_digest(canonical_request.as_bytes()));
    let string_to_sign =
        format!("HMAC-SHA256\n{x_date}\n{credential_scope}\n{hashed_canonical}");

    // 步骤 3：四层密钥派生（首层直接用 secretAccessKey，无 "AWS4" 前缀，火山特有）。
    let k_date = hmac_sha256(secret_access_key.as_bytes(), short_date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, VOLCENGINE_SERVICE.as_bytes());
    let k_signing = hmac_sha256(&k_service, b"request");

    // 步骤 4：Signature 与 Authorization 头。
    let signature = to_hex_lower(&hmac_sha256(&k_signing, string_to_sign.as_bytes()));

    format!(
        "HMAC-SHA256 Credential={access_key_id}/{credential_scope}, \
SignedHeaders={signed_headers}, Signature={signature}"
    )
}

/// 将火山 `ResponseMetadata.Error.Code` 归一为 `TranslateError`。
///
/// 错误码来源：火山引擎公共错误码官方文档（volcengine.com/docs/6369/67270）。
/// 按 Code 关键词归类（签名/鉴权→Auth、限流→RateLimit、配额→Quota）。
fn map_volcengine_error(code: &str, msg: &str) -> TranslateError {
    if code.contains("Signature")
        || code.contains("Auth")
        || code.contains("AccessKey")
        || code == "AccessDenied"
    {
        TranslateError::Auth(format!("火山翻译鉴权失败 {code}: {msg}"))
    } else if code.contains("FlowLimit") || code.contains("Throttling") {
        TranslateError::RateLimit(format!("火山翻译频率超限 {code}: {msg}"))
    } else if code.contains("Quota") || code.contains("Arrears") {
        TranslateError::Quota(format!("火山翻译配额不足 {code}: {msg}"))
    } else {
        TranslateError::ServerError(format!("火山翻译错误 {code}: {msg}"))
    }
}

// 云厂商签名共用工具（HMAC / hex / 时间格式化）

/// 计算 HMAC-SHA256，返回 32 字节摘要。
///
/// 供腾讯云 TC3 三层密钥派生与最终签名复用。
fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    use hmac::{Mac, SimpleHmac};
    use sha2::Sha256;
    // SimpleHmac::new_from_slice 对任意长度 key 不会失败（HMAC 定义允许任意长度 key）。
    let mut mac = SimpleHmac::<Sha256>::new_from_slice(key).expect("HMAC 接受任意长度 key");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

/// 计算 SHA-256，返回 32 字节摘要。
///
/// 供火山 SigV4 的 payloadHash 与 CanonicalRequest 哈希复用。
fn sha256_digest(msg: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(msg).to_vec()
}

/// 计算 HMAC-SHA1，返回 20 字节摘要。
///
/// 供阿里云 RPC 风格签名复用（官方要求 HMAC-SHA1）。
fn hmac_sha1(key: &[u8], msg: &[u8]) -> Vec<u8> {
    use hmac::{Mac, SimpleHmac};
    use sha1::Sha1;
    let mut mac = SimpleHmac::<Sha1>::new_from_slice(key).expect("HMAC 接受任意长度 key");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

/// 将字节切片转为小写十六进制字符串。
fn to_hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(hex_lower(b >> 4));
        out.push(hex_lower(b & 0x0F));
    }
    out
}

/// 将 4 位 nibble 转为小写十六进制字符。
fn hex_lower(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + nibble - 10) as char,
    }
}

/// 将 Unix 秒转为 UTC 日期字符串 `YYYY-MM-DD`（腾讯云 credential scope 用）。
///
/// 自实现公历换算（不引入 chrono），对 1970 起的正时间戳精确。
fn unix_secs_to_utc_date(timestamp: i64) -> String {
    let (year, month, day, _, _, _) = unix_secs_to_utc_parts(timestamp);
    format!("{year:04}-{month:02}-{day:02}")
}

/// 将 Unix 秒转为 ISO8601 UTC 字符串 `YYYY-MM-DDThh:mm:ssZ`（阿里云 Timestamp 用）。
fn unix_secs_to_iso8601_utc(timestamp: i64) -> String {
    let (year, month, day, hour, min, sec) = unix_secs_to_utc_parts(timestamp);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// 将 Unix 秒转为火山 SigV4 紧凑 UTC 时间戳 `YYYYMMDDThhmmssZ`（X-Date 头用）。
fn unix_secs_to_compact_utc(timestamp: i64) -> String {
    let (year, month, day, hour, min, sec) = unix_secs_to_utc_parts(timestamp);
    format!("{year:04}{month:02}{day:02}T{hour:02}{min:02}{sec:02}Z")
}

/// 将 Unix 秒转为紧凑 UTC 短日期 `YYYYMMDD`（火山 credential scope 用）。
fn unix_secs_to_compact_date(timestamp: i64) -> String {
    let (year, month, day, _, _, _) = unix_secs_to_utc_parts(timestamp);
    format!("{year:04}{month:02}{day:02}")
}

/// 把 Unix 秒拆为 UTC `(年, 月, 日, 时, 分, 秒)`。
///
/// 自实现公历日期换算（避免引入 chrono），用「以 0000-03-01 为纪元的天数公式」处理闰年。
/// 仅用于签名时间戳格式化，对 1970 起的合法时间戳精确。
fn unix_secs_to_utc_parts(timestamp: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = timestamp.div_euclid(86_400);
    let secs_of_day = timestamp.rem_euclid(86_400);
    let hour = (secs_of_day / 3600) as u32;
    let min = ((secs_of_day % 3600) / 60) as u32;
    let sec = (secs_of_day % 60) as u32;

    // 民用历算法（Howard Hinnant civil_from_days）：以 0000-03-01 为基准。
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = if month <= 2 { year + 1 } else { year };

    (year, month, day, hour, min, sec)
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
            is_unofficial: false,
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

        Ok(TranslateResponse::plain(translated))
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
            is_unofficial: false,
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

        Ok(TranslateResponse::plain(translated))
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
/// 换源必须用户显式操作；此处只负责告知用户需要做什么（填 key）。
///
/// 调用方负责把返回的 `UserPrompt` 展示给用户，而非自动执行任何切换。
pub fn on_quota_or_failure(provider_id: &str, err: &TranslateError) -> Option<UserPrompt> {
    match err {
        TranslateError::Quota(_) => Some(UserPrompt {
            kind: UserPromptKind::NeedKey,
            message: format!(
                "「{provider_id}」配额已耗尽，请检查账户余额或升级套餐，或填入有效 API Key。"
            ),
        }),
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

/// 对查询参数值或路径段做 RFC 3986 percent-encoding（不保留任何额外字符）。
fn percent_encode(s: &str) -> String {
    percent_encode_with_extra(s, &[])
}

/// 将 4 位 nibble 转为大写十六进制字符。
fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + nibble - 10) as char,
    }
}

// LLM 对话翻译（chat/completions 形态）：OpenAI + Ollama 共用 Prompt 模板引擎与 messages 构造

/// chat-completion 协议的一条对话消息。
///
/// `role` 取 "system" / "user" / "assistant"；`content` 为消息正文。
/// OpenAI 与 Ollama 的请求体均用 `messages: [{role, content}, ...]` 数组承载，
/// 故抽为共用结构体，由 `render_prompt` 产出、各 provider 拼进自家 body。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChatMessage {
    /// 消息角色（"system" / "user" / "assistant"）。
    pub role: String,
    /// 消息正文。
    pub content: String,
}

impl ChatMessage {
    /// 构造一条对话消息。
    fn new(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.to_string(),
            content: content.into(),
        }
    }
}

/// 内置默认翻译 Prompt 的 system 指示模板。
///
/// `$from`/`$to` 占位在 `render_prompt` 中替换为请求的源/目标语言。
/// 措辞约束模型「只输出译文」以避免寒暄/解释污染译文（设计文档§五.V3）。
const DEFAULT_TRANSLATE_PROMPT: &str =
    "你是专业翻译引擎。把用户消息中的文本从 $from 翻译到 $to，只输出译文本身，不要任何解释、前后缀或引号。";

/// 把可编辑 Prompt 模板渲染为 chat-completion 的 messages 数组（纯函数，可单测）。
///
/// 变量替换：`$text`→原文、`$from`→源语言、`$to`→目标语言。
/// - `template` 为 `Some(t)`：t 作为单条模板渲染后作为 **user** 消息内容（用户自定义形态），
///   不再附带默认 system，使「可编辑 Prompt」对最终请求有完全控制权。
/// - `template` 为 `None`：回退内置默认翻译 Prompt——system 用 `DEFAULT_TRANSLATE_PROMPT`
///   （替换 $from/$to），user 为原文（$text）。
///
/// 两分支均产出含 user 消息的非空数组，供 OpenAI/Ollama 直接拼进请求体。
pub fn render_prompt(template: Option<&str>, req: &TranslateRequest) -> Vec<ChatMessage> {
    let substitute = |s: &str| -> String {
        s.replace("$text", &req.text)
            .replace("$from", req.source_lang.as_str())
            .replace("$to", req.target_lang.as_str())
    };

    match template {
        Some(t) if !t.trim().is_empty() => vec![ChatMessage::new("user", substitute(t))],
        _ => vec![
            ChatMessage::new("system", substitute(DEFAULT_TRANSLATE_PROMPT)),
            ChatMessage::new("user", req.text.clone()),
        ],
    }
}

/// 构造 chat-completion 非流式请求体（OpenAI 与 Ollama 字段同构）。
///
/// body 形如 `{"model":..,"messages":[..],"stream":false}`；用 serde_json 构造自动正确转义文本。
/// 仅做非流式（`stream=false`，一次性解析），流式为 YAGNI（设计文档§四）。
fn build_chat_body(model: &str, messages: &[ChatMessage]) -> String {
    serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false,
    })
    .to_string()
}

/// OpenAI chat-completions provider（需要 apiKey；兼容自定义 base_url 网关）。
///
/// 端点（OpenAI 官方 API 文档，按协议事实独立实现，不参考任何第三方源码）：
/// `POST {base_url}/v1/chat/completions`，base_url 默认 `https://api.openai.com`。
/// 鉴权：`Authorization: Bearer {apiKey}`。
/// body：`{"model":..,"messages":[{role,content}..],"stream":false}`。
/// 响应取 `choices[0].message.content`；错误体 `{"error":{type,code,message}}` → TranslateError。
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    base_url: String,
    prompt: String,
}

impl OpenAiProvider {
    /// 构造 OpenAI provider。
    ///
    /// `base_url` 为空回退官方 `https://api.openai.com`；`prompt` 为空回退内置默认 Prompt。
    pub fn new(api_key: &str, model: &str, base_url: &str, prompt: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: base_url.to_string(),
            prompt: prompt.to_string(),
        }
    }

    /// 解析 base_url：空则回退官方端点，去尾部斜杠避免拼出双斜杠。
    fn resolved_base(&self) -> &str {
        let b = self.base_url.trim().trim_end_matches('/');
        if b.is_empty() {
            "https://api.openai.com"
        } else {
            b
        }
    }
}

impl TranslateProvider for OpenAiProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "openai",
            name: "OpenAI",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let prompt = optional_prompt(&self.prompt);
        let messages = render_prompt(prompt, req);
        let body = build_chat_body(&self.model, &messages);

        ProviderHttpRequest {
            method: "POST",
            url: format!("{}/v1/chat/completions", self.resolved_base()),
            body: Some(body),
            headers: vec![
                (
                    "Authorization".to_string(),
                    format!("Bearer {}", self.api_key),
                ),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 错误体 {"error":{type,code,message}}：按 type/code 归类，错误消息不回显 apiKey。
        let error = &v["error"];
        if !error.is_null() {
            let code = error["code"].as_str().unwrap_or("");
            let typ = error["type"].as_str().unwrap_or("");
            let msg = error["message"].as_str().unwrap_or("unknown");
            return Err(map_openai_error(typ, code, msg));
        }

        let translated = v["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                TranslateError::ParseError("missing choices[0].message.content".to_string())
            })?
            .trim()
            .to_string();

        Ok(TranslateResponse::plain(translated))
    }
}

/// 将 OpenAI 错误体的 type/code 归一为 `TranslateError`。
///
/// 分类参照 OpenAI 官方错误码（platform.openai.com/docs/guides/error-codes）：
/// 鉴权（invalid_api_key / 401）、限流（rate_limit_exceeded / 429）、配额（insufficient_quota）、
/// 上下文过长（context_length_exceeded）；其余归服务端错误。msg 不含 apiKey。
fn map_openai_error(typ: &str, code: &str, msg: &str) -> TranslateError {
    match (typ, code) {
        (_, "invalid_api_key" | "invalid_authentication") => {
            TranslateError::Auth(format!("OpenAI 鉴权失败: {msg}"))
        }
        (_, "rate_limit_exceeded") => TranslateError::RateLimit(format!("OpenAI 频率超限: {msg}")),
        (_, "insufficient_quota") => TranslateError::Quota(format!("OpenAI 配额不足: {msg}")),
        (_, "context_length_exceeded") => {
            TranslateError::TooLong(format!("OpenAI 上下文过长: {msg}"))
        }
        ("authentication_error", _) => TranslateError::Auth(format!("OpenAI 鉴权失败: {msg}")),
        _ => TranslateError::ServerError(format!("OpenAI 错误: {msg}")),
    }
}

/// Ollama 本地 chat provider（本地自部署，无鉴权）。
///
/// 端点（Ollama 官方 API 文档 github.com/ollama/ollama/blob/main/docs/api.md，独立实现不抄源码）：
/// `POST {base_url}/api/chat`，base_url 默认 `http://localhost:11434`。
/// 本地自部署无鉴权——绝不发 Authorization 头。
/// body：`{"model":..,"messages":[{role,content}..],"stream":false}`。
/// 响应取 `message.content`（非流式一次性返回整段）。
pub struct OllamaProvider {
    model: String,
    base_url: String,
    prompt: String,
}

impl OllamaProvider {
    /// 构造 Ollama provider。
    ///
    /// `base_url` 为空回退本地 `http://localhost:11434`；`prompt` 为空回退内置默认 Prompt。
    pub fn new(model: &str, base_url: &str, prompt: &str) -> Self {
        Self {
            model: model.to_string(),
            base_url: base_url.to_string(),
            prompt: prompt.to_string(),
        }
    }

    /// 解析 base_url：空则回退本地端点，去尾部斜杠避免拼出双斜杠。
    fn resolved_base(&self) -> &str {
        let b = self.base_url.trim().trim_end_matches('/');
        if b.is_empty() {
            "http://localhost:11434"
        } else {
            b
        }
    }
}

impl TranslateProvider for OllamaProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "ollama",
            name: "Ollama（本地）",
            needs_key: false,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let prompt = optional_prompt(&self.prompt);
        let messages = render_prompt(prompt, req);
        let body = build_chat_body(&self.model, &messages);

        // 本地自部署无鉴权：只发 Content-Type，绝不发 Authorization 头。
        ProviderHttpRequest {
            method: "POST",
            url: format!("{}/api/chat", self.resolved_base()),
            body: Some(body),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // Ollama 出错时返回 {"error":"..."}（无 message 字段）。
        if let Some(err) = v["error"].as_str() {
            return Err(TranslateError::ServerError(format!("Ollama 错误: {err}")));
        }

        let translated = v["message"]["content"]
            .as_str()
            .ok_or_else(|| TranslateError::ParseError("missing message.content".to_string()))?
            .trim()
            .to_string();

        Ok(TranslateResponse::plain(translated))
    }
}

/// 把可编辑 Prompt 字段（可能为空串）转为 `render_prompt` 的 `Option`：空串视同未配置。
fn optional_prompt(prompt: &str) -> Option<&str> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(prompt)
    }
}

/// 把字节切片编码为 base64url（无填充）字符串。
///
/// 供 ChatGLM 手搓 JWT 的 header / payload / signature 三段编码复用。
/// JWT（RFC 7519）规定用 base64url 且去除 `=` 填充。
fn base64url_no_pad(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// 按智谱（ChatGLM）官方文档手搓 JWT HS256，返回 `header.payload.signature` 形态 token。
///
/// 算法（智谱 open.bigmodel.cn 鉴权文档 / 设计文档§四）：
/// - header：`{"alg":"HS256","sign_type":"SIGN"}`
/// - payload：`{"api_key":{id},"exp":{exp_ms},"timestamp":{timestamp_ms}}`（毫秒时间戳）
/// - signature：`HMAC-SHA256(secret, base64url(header) + "." + base64url(payload))`
/// - 三段均 base64url 无填充，以 `.` 连接。
///
/// `exp_ms`/`timestamp_ms` 作参数注入，使签名对固定输入确定、可独立复算锚定单测
/// （不在内部读 SystemTime，避免签名不确定无法测）。
/// 抽为纯函数：不持有/不打印 id 或 secret，调用方传入、用后即弃。
fn chatglm_jwt(id: &str, secret: &str, exp_ms: i64, timestamp_ms: i64) -> String {
    // header/payload 用 serde_json 构造保证转义正确；字段顺序与官方文档及参照向量一致。
    let header = serde_json::json!({ "alg": "HS256", "sign_type": "SIGN" }).to_string();
    let payload = serde_json::json!({
        "api_key": id,
        "exp": exp_ms,
        "timestamp": timestamp_ms,
    })
    .to_string();

    let header_b64 = base64url_no_pad(header.as_bytes());
    let payload_b64 = base64url_no_pad(payload.as_bytes());
    let signing_input = format!("{header_b64}.{payload_b64}");
    let signature = hmac_sha256(secret.as_bytes(), signing_input.as_bytes());
    let signature_b64 = base64url_no_pad(&signature);

    format!("{signing_input}.{signature_b64}")
}

/// ChatGLM（智谱）端点常量（智谱开放平台 API 文档，独立实现不抄源码）。
const CHATGLM_ENDPOINT: &str = "https://open.bigmodel.cn/api/paas/v4/chat/completions";

/// JWT 有效期（毫秒）：签发后 1 小时。智谱要求 exp = 签发时刻 + 有效期。
const CHATGLM_JWT_TTL_MS: i64 = 3_600_000;

/// ChatGLM（智谱）chat-completions provider（需要 apiKey，形如 `{id}.{secret}`）。
///
/// 端点（智谱开放平台文档）：`POST https://open.bigmodel.cn/api/paas/v4/chat/completions`
/// （OpenAI 兼容 chat/completions 形态），base_url 可覆盖。
/// 鉴权：apiKey 拆 `{id}.{secret}`，手搓 JWT HS256（见 `chatglm_jwt`）置于 `Authorization` 头。
/// body 与 OpenAI 同构（model/messages/stream），复用 `build_chat_body` + `render_prompt`。
/// 响应取 `choices[0].message.content`；错误体 `{"error":{code,message}}` → TranslateError。
pub struct ChatGlmProvider {
    api_key: String,
    model: String,
    base_url: String,
    prompt: String,
}

impl ChatGlmProvider {
    /// 构造 ChatGLM provider。
    ///
    /// `api_key` 形如 `{id}.{secret}`；`base_url` 为空回退官方端点；`prompt` 为空回退内置默认。
    pub fn new(api_key: &str, model: &str, base_url: &str, prompt: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: base_url.to_string(),
            prompt: prompt.to_string(),
        }
    }

    /// 解析 base_url：空则回退官方端点，去尾部斜杠避免拼出双斜杠。
    fn resolved_endpoint(&self) -> String {
        let b = self.base_url.trim().trim_end_matches('/');
        if b.is_empty() {
            CHATGLM_ENDPOINT.to_string()
        } else {
            format!("{b}/api/paas/v4/chat/completions")
        }
    }

    /// 基于 apiKey `{id}.{secret}` 与当前时间签发 JWT 鉴权头值。
    ///
    /// apiKey 缺 `.` 分隔时整体作 id、secret 为空（让服务端鉴权失败而非本地 panic）。
    /// exp/timestamp 在此处用当前时间生成（请求时刻才需真实时间），
    /// 签名逻辑本身在纯函数 `chatglm_jwt` 内、可独立测。
    fn authorization(&self) -> String {
        let (id, secret) = self
            .api_key
            .split_once('.')
            .unwrap_or((self.api_key.as_str(), ""));
        let now_ms = current_unix_secs() as i64 * 1000;
        chatglm_jwt(id, secret, now_ms + CHATGLM_JWT_TTL_MS, now_ms)
    }
}

impl TranslateProvider for ChatGlmProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "chatglm",
            name: "ChatGLM（智谱）",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        let prompt = optional_prompt(&self.prompt);
        let messages = render_prompt(prompt, req);
        let body = build_chat_body(&self.model, &messages);

        ProviderHttpRequest {
            method: "POST",
            url: self.resolved_endpoint(),
            body: Some(body),
            headers: vec![
                // 智谱官方文档要求 `Authorization: Bearer <JWT>`（与 OpenAI 一致），JWT 非裸值。
                (
                    "Authorization".to_string(),
                    format!("Bearer {}", self.authorization()),
                ),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 错误体 {"error":{code,message}}：按 code 归类，错误消息不回显 apiKey。
        let error = &v["error"];
        if !error.is_null() {
            let code = error["code"].as_str().unwrap_or("");
            let msg = error["message"].as_str().unwrap_or("unknown");
            return Err(map_chatglm_error(code, msg));
        }

        let translated = v["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                TranslateError::ParseError("missing choices[0].message.content".to_string())
            })?
            .trim()
            .to_string();

        Ok(TranslateResponse::plain(translated))
    }
}

/// 将 ChatGLM 错误体 code 归一为 `TranslateError`。
///
/// 错误码来源：智谱开放平台 API 文档错误码列表（1002/1003 鉴权、1302 限流、1112 余额）。
/// msg 不含 apiKey。
fn map_chatglm_error(code: &str, msg: &str) -> TranslateError {
    match code {
        "1001" | "1002" | "1003" | "1004" => {
            TranslateError::Auth(format!("ChatGLM 鉴权失败 {code}: {msg}"))
        }
        "1302" | "1303" | "1305" => TranslateError::RateLimit(format!("ChatGLM 频率超限 {code}: {msg}")),
        "1112" | "1113" => TranslateError::Quota(format!("ChatGLM 余额不足 {code}: {msg}")),
        _ => TranslateError::ServerError(format!("ChatGLM 错误 {code}: {msg}")),
    }
}

/// Gemini 端点基址常量（Google Generative Language API 文档，独立实现不抄源码）。
const GEMINI_BASE: &str = "https://generativelanguage.googleapis.com";

/// Gemini（Google）generateContent provider（需要 apiKey，key 作 URL query 参）。
///
/// 端点（Google Generative Language API 文档）：
/// `POST {base}/v1beta/models/{model}:generateContent?key={apiKey}`，base 默认官方域名。
/// 鉴权：apiKey 作 URL query 参（**不进请求头**）。
/// body：`{"contents":[{"parts":[{"text":...}]}]}`，可含 `systemInstruction`。
/// 响应取 `candidates[0].content.parts[0].text`；错误体 `{"error":{code,message,status}}` → TranslateError。
pub struct GeminiProvider {
    api_key: String,
    model: String,
    base_url: String,
    prompt: String,
}

impl GeminiProvider {
    /// 构造 Gemini provider。
    ///
    /// `base_url` 为空回退官方域名；`model` 为空回退 `gemini-pro`；`prompt` 为空回退内置默认。
    pub fn new(api_key: &str, model: &str, base_url: &str, prompt: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: base_url.to_string(),
            prompt: prompt.to_string(),
        }
    }

    /// 解析 base：空则回退官方域名，去尾部斜杠避免拼出双斜杠。
    fn resolved_base(&self) -> &str {
        let b = self.base_url.trim().trim_end_matches('/');
        if b.is_empty() {
            GEMINI_BASE
        } else {
            b
        }
    }

    /// 解析 model：空则回退 `gemini-pro`。
    fn resolved_model(&self) -> &str {
        let m = self.model.trim();
        if m.is_empty() {
            "gemini-pro"
        } else {
            m
        }
    }
}

impl TranslateProvider for GeminiProvider {
    fn capability(&self) -> ProviderCapability {
        ProviderCapability {
            id: "gemini",
            name: "Gemini（Google）",
            needs_key: true,
            is_unofficial: false,
        }
    }

    fn build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest {
        // 复用 render_prompt 得到 messages，再转为 Gemini 的 contents/parts 结构：
        // system 消息映射为 systemInstruction，user 消息合并进 contents 的 parts.text。
        let prompt = optional_prompt(&self.prompt);
        let messages = render_prompt(prompt, req);
        let body = build_gemini_body(&messages);

        // key 作 URL query 参（percent-encode 防特殊字符破坏 URL），绝不进请求头。
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.resolved_base(),
            self.resolved_model(),
            percent_encode(&self.api_key),
        );

        ProviderHttpRequest {
            method: "POST",
            url,
            body: Some(body),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        }
    }

    fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
        let v: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| TranslateError::ParseError(e.to_string()))?;

        // 错误体 {"error":{code,message,status}}：按 status/code 归类，消息不回显 apiKey。
        let error = &v["error"];
        if !error.is_null() {
            let code = error["code"].as_u64().unwrap_or(0);
            let status = error["status"].as_str().unwrap_or("");
            let msg = error["message"].as_str().unwrap_or("unknown");
            return Err(map_gemini_error(code, status, msg));
        }

        let translated = v["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| {
                TranslateError::ParseError(
                    "missing candidates[0].content.parts[0].text".to_string(),
                )
            })?
            .trim()
            .to_string();

        Ok(TranslateResponse::plain(translated))
    }
}

/// 把 chat messages 转为 Gemini generateContent 请求体。
///
/// 映射规则（Google Generative Language API 文档）：
/// - role=="system" 的消息合并为顶层 `systemInstruction.parts[].text`。
/// - 其余消息（user/assistant）作为 `contents[].parts[].text`（role 归一为 user/model）。
///
/// 用 serde_json 构造自动正确转义文本。
fn build_gemini_body(messages: &[ChatMessage]) -> String {
    let system_text: String = messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let contents: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| {
            // Gemini 的 content role 取 user/model；assistant 归一为 model。
            let role = if m.role == "assistant" { "model" } else { "user" };
            serde_json::json!({ "role": role, "parts": [{ "text": m.content }] })
        })
        .collect();

    let mut body = serde_json::json!({ "contents": contents });
    if !system_text.is_empty() {
        body["systemInstruction"] = serde_json::json!({ "parts": [{ "text": system_text }] });
    }
    body.to_string()
}

/// 将 Gemini 错误体的 status/code 归一为 `TranslateError`。
///
/// 分类参照 Google API 标准错误码（google.golang.org/grpc/codes / HTTP 映射）：
/// UNAUTHENTICATED/PERMISSION_DENIED/无效 key→鉴权，RESOURCE_EXHAUSTED/429→限流，
/// INVALID_ARGUMENT→不支持，5xx→服务端。msg 不含 apiKey。
fn map_gemini_error(code: u64, status: &str, msg: &str) -> TranslateError {
    if msg.to_ascii_lowercase().contains("api key not valid") {
        return TranslateError::Auth(format!("Gemini 鉴权失败: {msg}"));
    }
    match (status, code) {
        ("UNAUTHENTICATED" | "PERMISSION_DENIED", _) | (_, 401 | 403) => {
            TranslateError::Auth(format!("Gemini 鉴权失败 {status}: {msg}"))
        }
        ("RESOURCE_EXHAUSTED", _) | (_, 429) => {
            TranslateError::RateLimit(format!("Gemini 频率超限 {status}: {msg}"))
        }
        ("INVALID_ARGUMENT", _) | (_, 400) => {
            TranslateError::Unsupported(format!("Gemini 请求无效 {status}: {msg}"))
        }
        _ => TranslateError::ServerError(format!("Gemini 错误 {status}: {msg}")),
    }
}

#[cfg(test)]
mod tests {
    use super::super::Lang;
    use super::*;

    /// 从翻译响应取出 Plain 变体的译文文本；非 Plain（如 Dict）即测试失败。
    ///
    /// 既有机翻/LLM 源全部产出 Plain，本 helper 让断言聚焦译文值、不在每处重复 match。
    fn plain_text(resp: &TranslateResponse) -> &str {
        match resp {
            TranslateResponse::Plain { translated } => translated,
            TranslateResponse::Dict { .. } => panic!("既有源应返回 Plain，实际返回 Dict"),
        }
    }

    // 对齐 acceptance TV1-F1-A01：注册表含 lingva（免 key）、不含 mymemory；
    // build_provider 对 lingva 成功、对 mymemory 返回未知源错误。
    #[test]
    fn providers_registry_has_lingva_no_mymemory() {
        let reg = registry();
        let lingva = reg
            .iter()
            .find(|c| c.id == "lingva")
            .expect("注册表应含 lingva");
        assert!(!lingva.needs_key, "lingva 应为免 key 源");
        assert!(
            reg.iter().all(|c| c.id != "mymemory"),
            "注册表不应再含 mymemory"
        );

        assert!(
            build_provider("lingva", &[]).is_ok(),
            "build_provider(\"lingva\") 应成功"
        );
        assert!(
            build_provider("mymemory", &[]).is_err(),
            "build_provider(\"mymemory\") 应返回未知源错误"
        );
    }

    // 对齐 acceptance TV1-F1-A02：build_request 生成 Lingva GET 端点 + 编码 text + 语言码；
    // parse_response 从 {"translation":"X"} 取 X，缺字段返回 ParseError。
    #[test]
    fn lingva_build_request_url_and_parse_translation() {
        let provider = LingvaProvider::new();
        let req = TranslateRequest {
            text: "hello world".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "GET");
        assert!(
            http_req
                .url
                .starts_with("https://lingva.ml/api/v1/"),
            "URL 应为 Lingva 端点，实际：{}",
            http_req.url
        );
        // 实测语言码：en→en、zh→zh；text percent-encode 后空格为 %20。
        assert!(
            http_req.url.ends_with("/en/zh/hello%20world"),
            "URL 应含编码后的语言码与 text，实际：{}",
            http_req.url
        );

        let ok = provider
            .parse_response(r#"{"translation":"glacier"}"#)
            .expect("含 translation 字段应解析成功");
        assert_eq!(plain_text(&ok), "glacier");

        let err = provider.parse_response(r#"{"foo":"bar"}"#);
        assert!(
            matches!(err, Err(TranslateError::ParseError(_))),
            "缺 translation 字段应返回 ParseError，实际：{err:?}"
        );
    }

    #[test]
    fn lingva_provider_is_keyless_and_built_without_credentials() {
        // 安全约定 TV1-A-SEC：免 key 源 needs_key=false，且空凭据可构造。
        let cap = LingvaProvider::new().capability();
        assert_eq!(cap.id, "lingva");
        assert!(!cap.needs_key);
        assert!(build_provider("lingva", &[]).is_ok());
    }

    #[test]
    fn registry_is_idempotent() {
        let first = registry();
        let second = registry();
        assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.id, b.id);
        }
    }

    #[test]
    fn build_provider_baidu_missing_required_fields_returns_err() {
        let result = build_provider("baidu", &[]);
        assert!(result.is_err(), "百度缺凭据应返回 Err");
        if let Err(err) = result {
            assert!(
                err.contains("未配置") || err.contains("AppID"),
                "错误应提示未配置：{err}"
            );
        }
    }

    #[test]
    fn build_provider_baidu_with_all_fields_succeeds() {
        let creds = vec![
            ("app_id".to_string(), "test_app_id".to_string()),
            ("secret_key".to_string(), "test_secret".to_string()),
        ];
        let result = build_provider("baidu", &creds);
        assert!(result.is_ok(), "百度全字段应成功");
        assert_eq!(result.unwrap().capability().id, "baidu");
    }

    #[test]
    fn build_provider_google_with_api_key_succeeds() {
        let creds = vec![("api_key".to_string(), "test_key".to_string())];
        let result = build_provider("google", &creds);
        assert!(result.is_ok(), "google 有 key 应成功");
        assert_eq!(result.unwrap().capability().id, "google");
    }

    #[test]
    fn build_provider_deepl_free_missing_auth_key_returns_err() {
        let result = build_provider("deepl_free", &[]);
        assert!(result.is_err(), "deepl_free 缺 auth_key 应返回 Err");
        if let Err(err) = result {
            assert!(
                err.contains("未配置") || err.contains("auth_key"),
                "错误应提示未配置：{err}"
            );
        }
    }

    #[test]
    fn build_provider_deepl_free_with_auth_key_succeeds() {
        let creds = vec![("auth_key".to_string(), "test_auth_key".to_string())];
        let result = build_provider("deepl_free", &creds);
        assert!(result.is_ok(), "deepl_free 有 auth_key 应成功");
        assert_eq!(result.unwrap().capability().id, "deepl_free");
    }

    #[test]
    fn build_provider_unknown_id_returns_err() {
        let result = build_provider("nonexistent_provider", &[]);
        assert!(result.is_err(), "未知 id 应返回 Err");
        if let Err(err) = result {
            assert!(!err.is_empty(), "Err 消息不应为空");
        }
    }

    // TDD RED: 带首尾空格的百度凭据应被 trim，build_request body 中 appid 是干净值
    #[test]
    fn build_provider_baidu_trims_whitespace_in_credentials() {
        let creds = vec![
            ("app_id".to_string(), " 12345 ".to_string()),
            ("secret_key".to_string(), " mysecret ".to_string()),
        ];
        let provider = build_provider("baidu", &creds).expect("带空格凭据应成功构造");

        let req = super::super::TranslateRequest {
            text: "hello".to_string(),
            source_lang: super::super::Lang("auto".to_string()),
            target_lang: super::super::Lang("zh".to_string()),
        };
        let http_req = provider.build_request(&req);
        let body = http_req.body.expect("百度请求应有 body");

        assert!(
            body.contains("appid=12345"),
            "trim 后 appid 应为 12345，body 实际为：{body}"
        );
        assert!(
            !body.contains("%2012345") && !body.contains("12345%20"),
            "appid 不应含空格编码 %20，body 实际为：{body}"
        );
    }

    // 对齐 acceptance TV1-F2-A01：Google 免费源 build_request 生成 translate_a/single GET 端点，
    // 含 client=gtx、sl/tl 语言码、dt=t 与 percent-encode 的 q。
    #[test]
    fn google_free_build_request_url() {
        let provider = GoogleFreeProvider::new();
        let req = TranslateRequest {
            text: "hello world".to_string(),
            source_lang: Lang::new("auto"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "GET");
        assert!(http_req.body.is_none(), "GET 请求不应有 body");
        assert!(
            http_req
                .url
                .starts_with("https://translate.googleapis.com/translate_a/single"),
            "URL 应为 Google translate_a/single 端点，实际：{}",
            http_req.url
        );
        assert!(
            http_req.url.contains("client=gtx"),
            "URL 应含 client=gtx，实际：{}",
            http_req.url
        );
        assert!(
            http_req.url.contains("dt=t"),
            "URL 应含 dt=t，实际：{}",
            http_req.url
        );
        // auto 透传作 sl；zh 经 google_free 映射为 zh-CN 作 tl。
        assert!(
            http_req.url.contains("sl=auto"),
            "URL 应含 sl=auto，实际：{}",
            http_req.url
        );
        assert!(
            http_req.url.contains("tl=zh-CN"),
            "URL 应含 tl=zh-CN，实际：{}",
            http_req.url
        );
        // text percent-encode 后空格为 %20。
        assert!(
            http_req.url.contains("q=hello%20world"),
            "URL 应含编码后的 q，实际：{}",
            http_req.url
        );
    }

    // 对齐 acceptance TV1-F2-A01：parse_response 拼接 result[0][i][0] 各分句译文；
    // 空/格式错返回 ParseError。
    #[test]
    fn google_free_parse_concatenates_segments() {
        let provider = GoogleFreeProvider::new();

        // 单分句（实测 sl=zh-CN&tl=en&q=冰川 的真实响应结构）。
        let single = provider
            .parse_response(r#"[[["glacier","冰川",null,null,2]],null,"zh-CN"]"#)
            .expect("单分句应解析成功");
        assert_eq!(plain_text(&single), "glacier");

        // 多分句：拼接各分句 result[0][i][0]（实测 Google 不在分句间补空格，原样拼接）。
        let multi = provider
            .parse_response(r#"[[["Hello","你好",null,null,2],["World","世界",null,null,2]],null,"zh-CN"]"#)
            .expect("多分句应解析成功");
        assert_eq!(plain_text(&multi), "HelloWorld");

        // result[0] 为空数组 → ParseError。
        let empty = provider.parse_response(r#"[[],null,"en"]"#);
        assert!(
            matches!(empty, Err(TranslateError::ParseError(_))),
            "空 result[0] 应返回 ParseError，实际：{empty:?}"
        );

        // 顶层非数组（格式错）→ ParseError。
        let malformed = provider.parse_response(r#"{"foo":"bar"}"#);
        assert!(
            matches!(malformed, Err(TranslateError::ParseError(_))),
            "格式错应返回 ParseError，实际：{malformed:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn google_free_is_keyless_and_built_without_credentials() {
        // 安全约定 TV1-A-SEC：免 key 源 needs_key=false，且空凭据可构造、不读凭据存储。
        let cap = GoogleFreeProvider::new().capability();
        assert_eq!(cap.id, "google_free");
        assert_eq!(cap.name, "Google（免费）");
        assert!(!cap.needs_key);
        assert!(build_provider("google_free", &[]).is_ok());
    }

    #[test]
    fn registry_contains_google_free_keyless() {
        let reg = registry();
        let gf = reg
            .iter()
            .find(|c| c.id == "google_free")
            .expect("注册表应含 google_free");
        assert!(!gf.needs_key, "google_free 应为免 key 源");
        // 既有 google（官方、需 key）不应被覆盖或移除。
        let g = reg
            .iter()
            .find(|c| c.id == "google")
            .expect("注册表仍应含官方 google");
        assert!(g.needs_key, "官方 google 仍需 key");
    }

    // Yandex 测试（免 key，伪装 Android 客户端公开协议）

    // 对齐 acceptance TV1-F2-A01：Yandex build_request 生成 translate v1 端点，
    // query 含 srv=android 与 id=...-0-0；body 为 text=...&lang=src-tgt。
    #[test]
    fn yandex_build_request_endpoint_and_body() {
        let provider = YandexProvider::new();
        let req = TranslateRequest {
            text: "hello world".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "POST");
        assert!(
            http_req
                .url
                .starts_with("https://translate.yandex.net/api/v1/tr.json/translate"),
            "URL 应为 Yandex translate v1 端点，实际：{}",
            http_req.url
        );
        // 实测必须 srv=android 且 id 形如 {hex}-0-0 才返回 200（否则 405 Session invalid）。
        assert!(
            http_req.url.contains("srv=android"),
            "URL 应含 srv=android，实际：{}",
            http_req.url
        );
        assert!(
            http_req.url.contains("id=") && http_req.url.contains("-0-0"),
            "URL 应含 id=...-0-0，实际：{}",
            http_req.url
        );

        let body = http_req.body.expect("Yandex 为 POST，应有 body");
        // 实测取译文靠 body 单参 lang=src-tgt（连字符对），非分参 source_lang/target_lang。
        assert!(
            body.contains("lang=en-zh"),
            "body 应含 lang=en-zh（连字符对），实际：{body}"
        );
        assert!(
            body.contains("text=hello%20world"),
            "body 应含 percent-encode 后的 text，实际：{body}"
        );
        assert_eq!(
            http_req
                .headers
                .iter()
                .find(|(k, _)| k == "Content-Type")
                .map(|(_, v)| v.as_str()),
            Some("application/x-www-form-urlencoded"),
            "Yandex 表单 POST 应声明 urlencoded Content-Type"
        );
    }

    // 对齐 acceptance TV1-F2-A01：parse_response 从录制样例 {"code":200,..,"text":["冰川"]}
    // 取 text 数组拼接为译文；code!=200 / 缺字段 / 非法 JSON 返回 TranslateError。
    #[test]
    fn yandex_parse_extracts_text_array() {
        let provider = YandexProvider::new();

        // 录制真实样例（artifacts/yandex-sample.json）：单元素 text 数组。
        let ok = provider
            .parse_response(r#"{"code":200,"lang":"en-zh","nmt_code":200,"text":["冰川"]}"#)
            .expect("含 text 数组应解析成功");
        assert_eq!(plain_text(&ok), "冰川");

        // 多元素 text 数组拼接（Yandex 偶将长文本分段返回）。
        let multi = provider
            .parse_response(r#"{"code":200,"text":["你好世界。","你好吗?"]}"#)
            .expect("多元素 text 数组应解析成功");
        assert_eq!(plain_text(&multi), "你好世界。你好吗?");

        // 录制错误样例（artifacts/yandex-error-sample.json）：code!=200 → 错误。
        let err = provider.parse_response(r#"{"code":405,"message":"Session is invalid"}"#);
        assert!(err.is_err(), "code!=200 应返回错误，实际：{err:?}");
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "405 会话失效应归一为 Auth，实际：{err:?}"
        );

        // text 为空数组 → ParseError。
        let empty = provider.parse_response(r#"{"code":200,"text":[]}"#);
        assert!(
            matches!(empty, Err(TranslateError::ParseError(_))),
            "空 text 数组应返回 ParseError，实际：{empty:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn yandex_is_keyless_and_built_without_credentials() {
        // 安全约定 TV1-A-SEC：免 key 源 needs_key=false，空凭据可构造、不读凭据存储。
        let cap = YandexProvider::new().capability();
        assert_eq!(cap.id, "yandex");
        assert!(!cap.needs_key);
        assert!(build_provider("yandex", &[]).is_ok());
    }

    #[test]
    fn registry_contains_yandex_keyless() {
        let reg = registry();
        let y = reg
            .iter()
            .find(|c| c.id == "yandex")
            .expect("注册表应含 yandex");
        assert!(!y.needs_key, "yandex 应为免 key 源");
    }

    // Transmart 测试（腾讯交互翻译·匿名接口公开协议）

    // 对齐 acceptance TV1-F2-A01：Transmart build_request 生成 imt 端点 JSON body，
    // header.fn=auto_translation、source/target.lang 与 text_list 正确。
    #[test]
    fn transmart_build_request_endpoint_and_json_body() {
        let provider = TransmartProvider::new();
        let req = TranslateRequest {
            text: "glacier".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "POST");
        assert_eq!(http_req.url, "https://transmart.qq.com/api/imt");
        assert_eq!(
            http_req
                .headers
                .iter()
                .find(|(k, _)| k == "Content-Type")
                .map(|(_, v)| v.as_str()),
            Some("application/json"),
            "Transmart 为 JSON POST，应声明 application/json"
        );

        let body = http_req.body.expect("Transmart 为 POST，应有 body");
        // body 必须是合法 JSON，且字段结构符合实测协议。
        let v: serde_json::Value =
            serde_json::from_str(&body).expect("Transmart body 应为合法 JSON");
        assert_eq!(
            v["header"]["fn"].as_str(),
            Some("auto_translation"),
            "header.fn 应为 auto_translation，实际 body：{body}"
        );
        assert_eq!(v["source"]["lang"].as_str(), Some("en"));
        assert_eq!(v["target"]["lang"].as_str(), Some("zh"));
        // 单段文本放入 text_list 首元素。
        assert_eq!(
            v["source"]["text_list"][0].as_str(),
            Some("glacier"),
            "source.text_list[0] 应为待译文本，实际 body：{body}"
        );
    }

    // 对齐 acceptance TV1-F2-A01：parse_response 从录制样例取 auto_translation 拼接；
    // ret_code!=succ / 缺字段 / 非法 JSON 返回 TranslateError。
    #[test]
    fn transmart_parse_concatenates_auto_translation() {
        let provider = TransmartProvider::new();

        // 录制真实样例（artifacts/transmart-sample.json）：含前后空串，拼接得译文。
        let ok = provider
            .parse_response(
                r#"{"header":{"ret_code":"succ"},"auto_translation":["","冰川",""],"src_lang":"en","tgt_lang":"zh"}"#,
            )
            .expect("含 auto_translation 应解析成功");
        assert_eq!(plain_text(&ok), "冰川");

        // 多段拼接（实测多 text_list 逐项对应）。
        let multi = provider
            .parse_response(
                r#"{"header":{"ret_code":"succ"},"auto_translation":["Glaciers are melting.","Hello World"]}"#,
            )
            .expect("多段 auto_translation 应解析成功");
        assert_eq!(plain_text(&multi), "Glaciers are melting.Hello World");

        // ret_code != succ → 错误。
        let err = provider.parse_response(r#"{"header":{"ret_code":"fail","message":"bad"}}"#);
        assert!(err.is_err(), "ret_code!=succ 应返回错误，实际：{err:?}");
        assert!(
            matches!(err, Err(TranslateError::ServerError(_))),
            "ret_code!=succ 应归一为 ServerError，实际：{err:?}"
        );

        // auto_translation 全空串拼接后为空 → ParseError。
        let empty = provider
            .parse_response(r#"{"header":{"ret_code":"succ"},"auto_translation":["",""]}"#);
        assert!(
            matches!(empty, Err(TranslateError::ParseError(_))),
            "全空译文应返回 ParseError，实际：{empty:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn transmart_is_keyless_and_built_without_credentials() {
        // 安全约定 TV1-A-SEC：免 key 源 needs_key=false，空凭据可构造、不读凭据存储。
        let cap = TransmartProvider::new().capability();
        assert_eq!(cap.id, "transmart");
        assert!(!cap.needs_key);
        assert!(build_provider("transmart", &[]).is_ok());
    }

    #[test]
    fn registry_contains_transmart_keyless() {
        let reg = registry();
        let t = reg
            .iter()
            .find(|c| c.id == "transmart")
            .expect("注册表应含 transmart");
        assert!(!t.needs_key, "transmart 应为免 key 源");
    }

    // TDD RED: 全空白的必填字段 trim 后为空，build_provider 应返回 Err
    #[test]
    fn build_provider_baidu_whitespace_only_app_id_returns_err() {
        let creds = vec![
            ("app_id".to_string(), "   ".to_string()),
            ("secret_key".to_string(), "valid_key".to_string()),
        ];
        let result = build_provider("baidu", &creds);
        assert!(
            result.is_err(),
            "全空白 app_id trim 后为空应视同缺失，返回 Err"
        );
    }

    // Bing 测试（免 key，两步：edge auth token → cognitive translate）

    use super::super::HttpExecutor;
    use std::cell::RefCell;

    /// 按 URL 子串路由返回不同 canned 响应的测试执行器（多步源专用）。
    ///
    /// 现有 `FakeExecutor`（ipc::translate）只返回固定串，无法区分 Bing 两步；
    /// 此处按 URL 命中关键字 take 对应响应（`TranslateError` 不实现 Clone，故用
    /// `RefCell<Option<…>>` 一次性取出，保留原错误变体不失真），并记录每次请求的
    /// URL 与 Authorization 头供断言两步顺序与 token 传递。
    struct RoutingFakeExecutor {
        auth_response: RefCell<Option<Result<String, TranslateError>>>,
        translate_response: RefCell<Option<Result<String, TranslateError>>>,
        seen_urls: RefCell<Vec<String>>,
        seen_auth_headers: RefCell<Vec<String>>,
    }

    impl RoutingFakeExecutor {
        fn new(
            auth: Result<String, TranslateError>,
            translate: Result<String, TranslateError>,
        ) -> Self {
            Self {
                auth_response: RefCell::new(Some(auth)),
                translate_response: RefCell::new(Some(translate)),
                seen_urls: RefCell::new(Vec::new()),
                seen_auth_headers: RefCell::new(Vec::new()),
            }
        }
    }

    // RefCell 非 Sync，但单测单线程使用、不跨线程共享，故为测试桩显式实现。
    unsafe impl Sync for RoutingFakeExecutor {}

    impl HttpExecutor for RoutingFakeExecutor {
        fn execute(&self, req: &ProviderHttpRequest) -> Result<String, TranslateError> {
            self.seen_urls.borrow_mut().push(req.url.clone());
            let auth = req
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("Authorization"))
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            self.seen_auth_headers.borrow_mut().push(auth);

            let slot = if req.url.contains("translate/auth") {
                &self.auth_response
            } else {
                &self.translate_response
            };
            slot.borrow_mut()
                .take()
                .expect("测试桩同一步骤被调用多次（超出预期）")
        }
    }

    fn bing_req() -> TranslateRequest {
        TranslateRequest {
            text: "glacier".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        }
    }

    // 对齐 acceptance TV1-F3-A01：Bing 两步流程——先取 edge auth token，
    // 再用 Bearer token POST 翻译，对录制样例正确解析出译文。
    #[test]
    fn bing_two_step_translate_with_mock_executor() {
        let provider = BingProvider::new();
        // 录制真实样例（artifacts/bing-translate-sample.json）。
        let exec = RoutingFakeExecutor::new(
            Ok("fake.jwt.token".to_string()),
            Ok(r#"[{"translations":[{"text":"冰川","to":"zh-Hans"}]}]"#.to_string()),
        );

        let resp = provider
            .translate(&bing_req(), &exec)
            .expect("Bing 两步应成功解析出译文");
        assert_eq!(plain_text(&resp), "冰川");

        // 验证两步顺序：第一步打 auth、第二步打 translate 端点。
        let urls = exec.seen_urls.borrow();
        assert_eq!(urls.len(), 2, "应恰好两次 HTTP 调用");
        assert!(
            urls[0].contains("edge.microsoft.com/translate/auth"),
            "第一步应为 edge auth，实际：{}",
            urls[0]
        );
        assert!(
            urls[1].contains("api-edge.cognitive.microsofttranslator.com/translate"),
            "第二步应为 cognitive translate，实际：{}",
            urls[1]
        );
        assert!(
            urls[1].contains("from=en") && urls[1].contains("to=zh-Hans"),
            "翻译 URL 应含 from=en&to=zh-Hans，实际：{}",
            urls[1]
        );
        // 第二步必须带上第一步取到的 Bearer token。
        let auths = exec.seen_auth_headers.borrow();
        assert_eq!(
            auths[1], "Bearer fake.jwt.token",
            "翻译步应携带第一步 token 的 Bearer 头，实际：{}",
            auths[1]
        );
    }

    #[test]
    fn bing_translate_token_step_failure_returns_error_not_panic() {
        let provider = BingProvider::new();
        let exec = RoutingFakeExecutor::new(
            Err(TranslateError::Network("auth 503".to_string())),
            Ok(r#"[{"translations":[{"text":"冰川"}]}]"#.to_string()),
        );
        let err = provider.translate(&bing_req(), &exec);
        assert!(
            matches!(err, Err(TranslateError::Network(_))),
            "token 步失败应返回 Network 错误，实际：{err:?}"
        );
        // token 步失败后不应再发翻译请求。
        assert_eq!(exec.seen_urls.borrow().len(), 1, "token 步失败应短路、不发翻译请求");
    }

    #[test]
    fn bing_translate_empty_token_returns_auth_error() {
        let provider = BingProvider::new();
        let exec = RoutingFakeExecutor::new(
            Ok("   ".to_string()),
            Ok(r#"[{"translations":[{"text":"冰川"}]}]"#.to_string()),
        );
        let err = provider.translate(&bing_req(), &exec);
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "空 token 应返回 Auth 错误，实际：{err:?}"
        );
    }

    #[test]
    fn bing_translate_step_failure_returns_error() {
        let provider = BingProvider::new();
        let exec = RoutingFakeExecutor::new(
            Ok("fake.jwt.token".to_string()),
            Err(TranslateError::RateLimit("429".to_string())),
        );
        let err = provider.translate(&bing_req(), &exec);
        assert!(
            matches!(err, Err(TranslateError::RateLimit(_))),
            "翻译步失败应透传错误，实际：{err:?}"
        );
    }

    #[test]
    fn bing_parse_response_extracts_translation_text() {
        let provider = BingProvider::new();
        // 录制真实样例（artifacts/bing-translate-sample.json）。
        let ok = provider
            .parse_response(r#"[{"translations":[{"text":"冰川","to":"zh-Hans"}]}]"#)
            .expect("含 translations[0].text 应解析成功");
        assert_eq!(plain_text(&ok), "冰川");

        // 缺 translations 字段 → ParseError。
        let missing = provider.parse_response(r#"[{"detectedLanguage":{"language":"en"}}]"#);
        assert!(
            matches!(missing, Err(TranslateError::ParseError(_))),
            "缺 translations 应返回 ParseError，实际：{missing:?}"
        );

        // 空数组 → ParseError。
        let empty = provider.parse_response("[]");
        assert!(
            matches!(empty, Err(TranslateError::ParseError(_))),
            "空数组应返回 ParseError，实际：{empty:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn bing_is_keyless_and_built_without_credentials() {
        // 安全约定 TV1-A-SEC：免 key 源 needs_key=false，空凭据可构造、不读凭据存储。
        let cap = BingProvider::new().capability();
        assert_eq!(cap.id, "bing");
        assert!(!cap.needs_key);
        assert!(build_provider("bing", &[]).is_ok());
    }

    #[test]
    fn registry_contains_bing_keyless() {
        let reg = registry();
        let b = reg.iter().find(|c| c.id == "bing").expect("注册表应含 bing");
        assert!(!b.needs_key, "bing 应为免 key 源");
    }

    // 对齐 acceptance TV1-F4-A01：非官方免 key 源 capability.is_unofficial=true，
    // 官方 keyed 源 is_unofficial=false（设计文档§二：免 key 源均为非官方/自建接口）。
    #[test]
    fn capability_is_unofficial_flags_free_sources_only() {
        let reg = registry();
        // 期望值：5 个免 key 非官方源 true、3 个官方 keyed 源 false。
        let expected: &[(&str, bool)] = &[
            ("lingva", true),
            ("google_free", true),
            ("yandex", true),
            ("transmart", true),
            ("bing", true),
            ("baidu", false),
            ("deepl_free", false),
            ("google", false),
        ];
        for (id, want) in expected {
            let cap = reg
                .iter()
                .find(|c| c.id == *id)
                .unwrap_or_else(|| panic!("注册表应含 {id}"));
            assert_eq!(
                cap.is_unofficial, *want,
                "{id} 的 is_unofficial 应为 {want}，实际 {}",
                cap.is_unofficial
            );
        }
    }

    // 百度专业（fieldtranslate）测试（需 key，官方 API 文档协议）

    // 对齐 acceptance TV2-F1-A01：baidu_field_sign 对固定输入产出确定 MD5；
    // build_request 用 fieldtranslate 端点、body 含 domain(field) 与正确签名。
    #[test]
    fn baidu_field_sign_and_build() {
        // 签名串 MD5(appid + text + salt + field + secret) 的确定值（printf|md5 手算核对）。
        let sign = baidu_field_sign("appid123", "hello", "12345", "it", "secret");
        assert_eq!(
            sign, "0ddfb12f98655a716cc509c2538a4386",
            "baidu_field 签名应为 MD5(appid+q+salt+field+secret) 的确定值，实际：{sign}"
        );

        let provider = BaiduFieldProvider::new("appid123", "secret", "it");
        let req = TranslateRequest {
            text: "hello".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url, "https://fanyi-api.baidu.com/api/trans/vip/fieldtranslate",
            "URL 应为百度专业 fieldtranslate 端点，实际：{}",
            http_req.url
        );
        let body = http_req.body.expect("百度专业为 POST，应有 body");
        assert!(body.contains("q=hello"), "body 应含编码后 q，实际：{body}");
        assert!(body.contains("from=en"), "body 应含 from=en，实际：{body}");
        assert!(body.contains("to=zh"), "body 应含 to=zh，实际：{body}");
        assert!(
            body.contains("appid=appid123"),
            "body 应含 appid，实际：{body}"
        );
        assert!(
            body.contains("domain=it"),
            "body 应含 domain=it（领域 field 经 domain 参数传递），实际：{body}"
        );
        // 签名由 build 内随机 salt 计算，故重算同 salt 比对。
        let salt = body
            .split("salt=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .expect("body 应含 salt 参数");
        let expected_sign = baidu_field_sign("appid123", "hello", salt, "it", "secret");
        assert!(
            body.contains(&format!("sign={expected_sign}")),
            "body 的 sign 应等于 baidu_field_sign(appid,q,salt,field,secret)，实际 body：{body}"
        );
    }

    // 对齐 acceptance TV2-F1-A01：parse_response 拼接 trans_result[*].dst；错误响应→TranslateError。
    #[test]
    fn baidu_field_parse() {
        let provider = BaiduFieldProvider::new("appid123", "secret", "it");

        // 成功响应：trans_result 各 dst 拼接。
        let ok = provider
            .parse_response(r#"{"from":"en","to":"zh","trans_result":[{"src":"hello","dst":"你好"}]}"#)
            .expect("含 trans_result.dst 应解析成功");
        assert_eq!(plain_text(&ok), "你好");

        // 多段 dst 换行拼接（百度逐段返回）。
        let multi = provider
            .parse_response(r#"{"trans_result":[{"src":"a","dst":"甲"},{"src":"b","dst":"乙"}]}"#)
            .expect("多段 trans_result 应解析成功");
        assert_eq!(plain_text(&multi), "甲\n乙");

        // 错误响应（error_code）→ TranslateError（54001 签名错误归一为 Auth）。
        let err = provider.parse_response(r#"{"error_code":"54001","error_msg":"Invalid Sign"}"#);
        assert!(err.is_err(), "error_code 应返回错误，实际：{err:?}");
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "54001 签名错误应归一为 Auth，实际：{err:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn build_provider_baidu_field_missing_required_fields_returns_err() {
        // 缺 field（领域）必填字段应报错，且错误消息不含任何字段值（安全约定）。
        let creds = vec![
            ("app_id".to_string(), "id1".to_string()),
            (
                "secret_key".to_string(),
                "sk_secret_must_not_leak".to_string(),
            ),
        ];
        let result = build_provider("baidu_field", &creds);
        assert!(result.is_err(), "缺 field 应返回 Err");
        if let Err(err) = result {
            assert!(
                !err.contains("sk_secret_must_not_leak"),
                "错误消息不应含 secret 值：{err}"
            );
        }
    }

    #[test]
    fn build_provider_baidu_field_with_all_fields_succeeds() {
        let creds = vec![
            ("app_id".to_string(), "id1".to_string()),
            ("secret_key".to_string(), "sk1".to_string()),
            ("field".to_string(), "it".to_string()),
        ];
        let result = build_provider("baidu_field", &creds);
        assert!(result.is_ok(), "百度专业全字段应成功");
        assert_eq!(result.unwrap().capability().id, "baidu_field");
    }

    #[test]
    fn registry_contains_baidu_field_keyed_official() {
        let reg = registry();
        let bf = reg
            .iter()
            .find(|c| c.id == "baidu_field")
            .expect("注册表应含 baidu_field");
        assert!(bf.needs_key, "baidu_field 应为需 key 源");
        assert!(
            !bf.is_unofficial,
            "baidu_field 为官方 API，is_unofficial 应为 false"
        );
    }

    // 有道（signType=v3）测试（需 key，官方 API 文档协议）

    // 对齐 acceptance TV2-F1-A01：youdao_sign 纯函数对固定输入产出确定 SHA256；
    // truncate 规则边界（短文本用全文、长文本前10+len+后10）；build_request 端点/参数正确。
    #[test]
    fn youdao_sign_v3_and_build() {
        // 短文本（len<=20）：truncate 用全文。SHA256(appKey+input+salt+curtime+appSecret) 确定值。
        let short_sign = youdao_sign("app123", "hello", "saltX", "1700000000", "sec456");
        assert_eq!(
            short_sign, "de9f455414aeb5c0057ad78813f9be70ff0ef07f9ea70cf53ee90169860871a2",
            "短文本 youdao 签名应为确定 SHA256，实际：{short_sign}"
        );

        // 长文本（len>20）：truncate = 前10字符 + 字符长度 + 后10字符。
        let long_text = "this is a very long text over twenty chars";
        assert_eq!(long_text.chars().count(), 42, "样例长文本应为 42 字符");
        let long_sign = youdao_sign("app123", long_text, "saltX", "1700000000", "sec456");
        assert_eq!(
            long_sign, "b61bfa28f19cdff1f69386cb076d25c13902bc855a97e91364dbad6fe76c3c3b",
            "长文本 truncate(前10+len+后10) 后签名应为确定 SHA256，实际：{long_sign}"
        );

        // 边界：恰好 20 字符仍用全文（<=20）。
        let exactly20 = "12345678901234567890";
        assert_eq!(exactly20.chars().count(), 20);
        assert_eq!(youdao_truncate(exactly20), exactly20, "恰好 20 字符应用全文");

        // 21 字符触发截断：前10 + "21" + 后10。
        assert_eq!(
            youdao_truncate("123456789012345678901"),
            "1234567890212345678901",
            "21 字符应截断为 前10+21+后10"
        );

        let provider = YoudaoProvider::new("app123", "sec456");
        let req = TranslateRequest {
            text: "hello".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url, "https://openapi.youdao.com/api",
            "URL 应为有道 openapi 端点，实际：{}",
            http_req.url
        );
        let body = http_req.body.expect("有道为 POST，应有 body");
        assert!(body.contains("q=hello"), "body 应含编码后 q，实际：{body}");
        assert!(body.contains("from=en"), "body 应含 from=en，实际：{body}");
        assert!(
            body.contains("to=zh-CHS"),
            "body 应含 to=zh-CHS（有道简中码），实际：{body}"
        );
        assert!(
            body.contains("appKey=app123"),
            "body 应含 appKey，实际：{body}"
        );
        assert!(
            body.contains("signType=v3"),
            "body 应含 signType=v3，实际：{body}"
        );
        assert!(body.contains("salt="), "body 应含 salt，实际：{body}");
        assert!(body.contains("curtime="), "body 应含 curtime，实际：{body}");
        // 签名应等于对 build 内实际 salt/curtime 重算的 youdao_sign。
        let salt = body
            .split("salt=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .expect("body 应含 salt");
        let curtime = body
            .split("curtime=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .expect("body 应含 curtime");
        let expected = youdao_sign("app123", "hello", salt, curtime, "sec456");
        assert!(
            body.contains(&format!("sign={expected}")),
            "body 的 sign 应等于 youdao_sign(appKey,truncate(q),salt,curtime,appSecret)，实际 body：{body}"
        );
    }

    // 对齐 acceptance TV2-F1-A01：parse_response 拼接 translation[*]；错误响应→TranslateError。
    #[test]
    fn youdao_parse() {
        let provider = YoudaoProvider::new("app123", "sec456");

        // 成功响应：translation 数组拼接（errorCode=0）。
        let ok = provider
            .parse_response(r#"{"errorCode":"0","query":"hello","translation":["你好"],"l":"en2zh-CHS"}"#)
            .expect("含 translation 应解析成功");
        assert_eq!(plain_text(&ok), "你好");

        // 多段 translation 换行拼接。
        let multi = provider
            .parse_response(r#"{"errorCode":"0","translation":["你好","世界"]}"#)
            .expect("多段 translation 应解析成功");
        assert_eq!(plain_text(&multi), "你好\n世界");

        // errorCode != 0 → TranslateError（108 应用密钥无效归一为 Auth）。
        let err = provider.parse_response(r#"{"errorCode":"108","translation":null}"#);
        assert!(err.is_err(), "errorCode!=0 应返回错误，实际：{err:?}");
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "108 应用密钥无效应归一为 Auth，实际：{err:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn build_provider_youdao_missing_required_fields_returns_err() {
        // 缺 app_secret 应报错。
        let creds = vec![("app_key".to_string(), "ak1".to_string())];
        let result = build_provider("youdao", &creds);
        assert!(result.is_err(), "缺 app_secret 应返回 Err");
    }

    #[test]
    fn build_provider_youdao_with_all_fields_succeeds() {
        let creds = vec![
            ("app_key".to_string(), "ak1".to_string()),
            ("app_secret".to_string(), "as1".to_string()),
        ];
        let result = build_provider("youdao", &creds);
        assert!(result.is_ok(), "有道全字段应成功");
        assert_eq!(result.unwrap().capability().id, "youdao");
    }

    #[test]
    fn registry_contains_youdao_keyed_official() {
        let reg = registry();
        let y = reg
            .iter()
            .find(|c| c.id == "youdao")
            .expect("注册表应含 youdao");
        assert!(y.needs_key, "youdao 应为需 key 源");
        assert!(
            !y.is_unofficial,
            "youdao 为官方 API，is_unofficial 应为 false"
        );
    }

    // 彩云小译（token 简单鉴权，需 key，官方 API 文档协议）测试

    // 对齐 acceptance TV2-F2-A01：build_request 端点/x-authorization 头/JSON body 结构正确；
    // parse_response 取 target[0]；错误响应→TranslateError。
    #[test]
    fn caiyun_build_and_parse() {
        let provider = CaiyunProvider::new("tok123");
        let req = TranslateRequest {
            text: "hello".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url, "https://api.interpreter.caiyunai.com/v1/translator",
            "URL 应为彩云 translator 端点，实际：{}",
            http_req.url
        );
        // 鉴权头：x-authorization: token {token}（彩云官方文档约定）。
        let auth = http_req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("x-authorization"))
            .expect("应含 x-authorization 头");
        assert_eq!(
            auth.1, "token tok123",
            "x-authorization 应为 'token {{token}}'，实际：{}",
            auth.1
        );
        assert!(
            http_req
                .headers
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case("content-type")
                    && v.contains("application/json")),
            "应含 content-type: application/json 头，实际：{:?}",
            http_req.headers
        );

        // body 为合法 JSON，含 source（数组）、trans_type=en2zh、request_id。
        let body = http_req.body.expect("彩云为 POST，应有 body");
        let parsed: serde_json::Value =
            serde_json::from_str(&body).expect("body 应为合法 JSON");
        assert_eq!(
            parsed["source"][0], "hello",
            "source 应为含原文的数组，实际：{body}"
        );
        assert_eq!(
            parsed["trans_type"], "en2zh",
            "trans_type 应按 en→zh 映射为 en2zh，实际：{body}"
        );
        assert!(
            parsed["request_id"].is_string(),
            "应含 request_id 字符串，实际：{body}"
        );

        // 成功响应：target 为译文数组，取首项。
        let ok = provider
            .parse_response(r#"{"target":["你好"],"confidence":0.9}"#)
            .expect("含 target 应解析成功");
        assert_eq!(plain_text(&ok), "你好");

        // 兼容 target 为字符串形态（部分接口返回单串）。
        let ok_str = provider
            .parse_response(r#"{"target":"你好世界"}"#)
            .expect("target 为字符串也应解析成功");
        assert_eq!(plain_text(&ok_str), "你好世界");

        // 错误响应（message 字段，无 target）→ TranslateError。
        // 错误响应（message 含 token）→ map_caiyun_error 归为 Auth。
        let err = provider.parse_response(r#"{"message":"token is invalid"}"#);
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "token 无效的错误消息应归一为 Auth，实际：{err:?}"
        );

        // 错误响应（message 不含 token 关键词）→ 归为 ServerError（非 token 类错误兜底）。
        let server_err = provider.parse_response(r#"{"message":"internal error"}"#);
        assert!(
            matches!(server_err, Err(TranslateError::ServerError(_))),
            "非鉴权类错误消息应归一为 ServerError，实际：{server_err:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn build_provider_caiyun_missing_token_returns_err() {
        // 缺 token 必填字段应报错，且错误消息不含任何字段值（安全约定 TV2-F5-A01）。
        // 传一个错拼 key 的脏密钥值：token 仍缺失走缺字段路径，坐实错误消息不泄露该脏值。
        let creds = vec![("toke".to_string(), "tok_secret_must_not_leak".to_string())];
        let result = build_provider("caiyun", &creds);
        assert!(result.is_err(), "缺 token 应返回 Err");
        if let Err(err) = result {
            assert!(
                !err.contains("tok_secret_must_not_leak"),
                "错误消息不应含 token 值：{err}"
            );
        }
    }

    #[test]
    fn build_provider_caiyun_with_token_succeeds() {
        let creds = vec![("token".to_string(), "tok123".to_string())];
        let result = build_provider("caiyun", &creds);
        assert!(result.is_ok(), "彩云有 token 应成功");
        assert_eq!(result.unwrap().capability().id, "caiyun");
    }

    #[test]
    fn registry_contains_caiyun_keyed_official() {
        let reg = registry();
        let c = reg
            .iter()
            .find(|c| c.id == "caiyun")
            .expect("注册表应含 caiyun");
        assert!(c.needs_key, "caiyun 应为需 key 源");
        assert!(
            !c.is_unofficial,
            "caiyun 为官方 API，is_unofficial 应为 false"
        );
    }

    // 小牛翻译（body apikey，需 key，官方 API 文档协议）测试

    // 对齐 acceptance TV2-F2-A01：build_request 端点/JSON body（from/to/apikey/src_text）正确；
    // parse_response 取 tgt_text；错误响应→TranslateError。
    #[test]
    fn niutrans_build_and_parse() {
        let provider = NiutransProvider::new("apikey789");
        let req = TranslateRequest {
            text: "hello".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url, "https://api.niutrans.com/NiuTransServer/translation",
            "URL 应为小牛 translation 端点，实际：{}",
            http_req.url
        );
        assert!(
            http_req
                .headers
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case("content-type")
                    && v.contains("application/json")),
            "应含 content-type: application/json 头，实际：{:?}",
            http_req.headers
        );

        // body 为合法 JSON，含 from/to/apikey/src_text。
        let body = http_req.body.expect("小牛为 POST，应有 body");
        let parsed: serde_json::Value =
            serde_json::from_str(&body).expect("body 应为合法 JSON");
        assert_eq!(parsed["from"], "en", "from 应为 en，实际：{body}");
        assert_eq!(parsed["to"], "zh", "to 应为 zh，实际：{body}");
        assert_eq!(
            parsed["apikey"], "apikey789",
            "body 应含 apikey，实际：{body}"
        );
        assert_eq!(
            parsed["src_text"], "hello",
            "src_text 应为原文，实际：{body}"
        );

        // 成功响应：取 tgt_text。
        let ok = provider
            .parse_response(r#"{"from":"en","to":"zh","tgt_text":"你好","src_text":"hello"}"#)
            .expect("含 tgt_text 应解析成功");
        assert_eq!(plain_text(&ok), "你好");

        // 错误响应（error_code 13001 apikey 错误）→ map_niutrans_error 归为 Auth。
        let err =
            provider.parse_response(r#"{"error_code":"13001","error_msg":"apikey error"}"#);
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "13001 apikey 错误应归一为 Auth，实际：{err:?}"
        );

        // 错误响应（error_code 19001 余额不足）→ 归为 Quota。
        let quota_err =
            provider.parse_response(r#"{"error_code":"19001","error_msg":"no balance"}"#);
        assert!(
            matches!(quota_err, Err(TranslateError::Quota(_))),
            "19001 余额不足应归一为 Quota，实际：{quota_err:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn build_provider_niutrans_missing_apikey_returns_err() {
        // 缺 apikey 必填字段应报错，且错误消息不含任何字段值（安全约定 TV2-F5-A01）。
        // 传一个错拼 key 的脏密钥值：apikey 仍缺失走缺字段路径，坐实错误消息不泄露该脏值。
        let creds = vec![("apike".to_string(), "key_secret_must_not_leak".to_string())];
        let result = build_provider("niutrans", &creds);
        assert!(result.is_err(), "缺 apikey 应返回 Err");
        if let Err(err) = result {
            assert!(
                !err.contains("key_secret_must_not_leak"),
                "错误消息不应含 apikey 值：{err}"
            );
        }
    }

    #[test]
    fn build_provider_niutrans_with_apikey_succeeds() {
        let creds = vec![("apikey".to_string(), "apikey789".to_string())];
        let result = build_provider("niutrans", &creds);
        assert!(result.is_ok(), "小牛有 apikey 应成功");
        assert_eq!(result.unwrap().capability().id, "niutrans");
    }

    #[test]
    fn registry_contains_niutrans_keyed_official() {
        let reg = registry();
        let n = reg
            .iter()
            .find(|c| c.id == "niutrans")
            .expect("注册表应含 niutrans");
        assert!(n.needs_key, "niutrans 应为需 key 源");
        assert!(
            !n.is_unofficial,
            "niutrans 为官方 API，is_unofficial 应为 false"
        );
    }

    // 腾讯云 TMT（TC3-HMAC-SHA256，需 key，官方 API 文档协议）测试

    // 对齐 acceptance TV2-F3-A01：tencent_tc3_sign 纯函数对固定输入产出确定 Authorization。
    // 参照向量由独立 Python 实现按腾讯云签名 v3 官方文档（cloud.tencent.com/document/api/551/30637）
    // 计算（非 pot 源码），固定 secret/timestamp/payload 下手算核对。
    #[test]
    fn tencent_tc3_signature_deterministic() {
        let payload = r#"{"SourceText":"hello","Source":"en","Target":"zh","ProjectId":0}"#;
        let auth = tencent_tc3_sign(
            "AKIDtest_secret_id_123",
            "test_secret_key_abc",
            payload,
            1_700_000_000,
        );

        // timestamp=1700000000 对应 UTC 日期 2023-11-14；credential scope 用该 date。
        let expected = "TC3-HMAC-SHA256 Credential=AKIDtest_secret_id_123/2023-11-14/tmt/tc3_request, \
SignedHeaders=content-type;host;x-tc-action, \
Signature=cc913306276069356aef21567e4670d036b69e1fd30eb24e17d7c536ed7decaf";
        assert_eq!(
            auth, expected,
            "tencent TC3 Authorization 应为按官方文档计算的确定值，实际：{auth}"
        );

        // 同输入二次调用应稳定一致（纯函数确定性）。
        let auth2 = tencent_tc3_sign(
            "AKIDtest_secret_id_123",
            "test_secret_key_abc",
            payload,
            1_700_000_000,
        );
        assert_eq!(auth, auth2, "同输入两次签名应一致（确定性）");
    }

    // 对齐 acceptance TV2-F3-A01：build_request 端点/头/JSON body 正确；
    // parse_response 取 Response.TargetText；错误响应→TranslateError。
    #[test]
    fn tencent_build_and_parse() {
        let provider = TencentProvider::new("secret_id_1", "secret_key_1");
        let req = TranslateRequest {
            text: "hello".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url, "https://tmt.tencentcloudapi.com",
            "URL 应为腾讯云 TMT 端点，实际：{}",
            http_req.url
        );
        // 必需头：Authorization、X-TC-Action、X-TC-Version、X-TC-Timestamp、X-TC-Region、Host、Content-Type。
        let header = |name: &str| {
            http_req
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(
            header("X-TC-Action"),
            Some("TextTranslate"),
            "应含 X-TC-Action: TextTranslate 头，实际：{:?}",
            http_req.headers
        );
        assert_eq!(
            header("X-TC-Version"),
            Some("2018-03-21"),
            "应含 X-TC-Version: 2018-03-21 头"
        );
        assert!(
            header("X-TC-Region").is_some(),
            "应含 X-TC-Region 头，实际：{:?}",
            http_req.headers
        );
        assert!(
            header("X-TC-Timestamp").is_some(),
            "应含 X-TC-Timestamp 头"
        );
        let auth = header("Authorization").expect("应含 Authorization 头");
        assert!(
            auth.starts_with("TC3-HMAC-SHA256 Credential=secret_id_1/"),
            "Authorization 应为 TC3 签名头，实际：{auth}"
        );

        // body 为合法 JSON，含 SourceText/Source/Target。
        let body = http_req.body.expect("腾讯为 POST，应有 body");
        let parsed: serde_json::Value =
            serde_json::from_str(&body).expect("body 应为合法 JSON");
        assert_eq!(parsed["SourceText"], "hello", "body 应含 SourceText");
        assert_eq!(parsed["Source"], "en", "body 应含 Source=en");
        assert_eq!(parsed["Target"], "zh", "body 应含 Target=zh");

        // Authorization 应等于对 build 内实际 timestamp 重算的 tencent_tc3_sign。
        let timestamp: i64 = header("X-TC-Timestamp")
            .expect("应含 X-TC-Timestamp")
            .parse()
            .expect("timestamp 应为整数");
        let expected_auth = tencent_tc3_sign("secret_id_1", "secret_key_1", &body, timestamp);
        assert_eq!(
            auth, expected_auth,
            "Authorization 应等于 tencent_tc3_sign(secret_id,secret_key,body,timestamp)"
        );

        // 成功响应：取 Response.TargetText。
        let ok = provider
            .parse_response(
                r#"{"Response":{"TargetText":"你好","Source":"en","Target":"zh","RequestId":"r1"}}"#,
            )
            .expect("含 Response.TargetText 应解析成功");
        assert_eq!(plain_text(&ok), "你好");

        // 错误响应（Response.Error）→ TranslateError。
        // AuthFailure.SignatureFailure 应归一为 Auth。
        let err = provider.parse_response(
            r#"{"Response":{"Error":{"Code":"AuthFailure.SignatureFailure","Message":"signature error"},"RequestId":"r2"}}"#,
        );
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "AuthFailure.* 应归一为 Auth，实际：{err:?}"
        );

        // 限流错误归一为 RateLimit。
        let rate = provider.parse_response(
            r#"{"Response":{"Error":{"Code":"RequestLimitExceeded","Message":"too many requests"},"RequestId":"r3"}}"#,
        );
        assert!(
            matches!(rate, Err(TranslateError::RateLimit(_))),
            "RequestLimitExceeded 应归一为 RateLimit，实际：{rate:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn build_provider_tencent_missing_secret_key_returns_err() {
        // 缺 secret_key 必填字段应报错，且错误消息不含任何字段值（安全约定 TV2-F5-A01）。
        // 传一个错拼 key 的脏密钥值：secret_key 仍缺失走缺字段路径，坐实错误消息不泄露该脏值。
        let creds = vec![
            ("secret_id".to_string(), "sid1".to_string()),
            (
                "secret_ke".to_string(),
                "sk_secret_must_not_leak".to_string(),
            ),
        ];
        let result = build_provider("tencent", &creds);
        assert!(result.is_err(), "缺 secret_key 应返回 Err");
        if let Err(err) = result {
            assert!(
                !err.contains("sk_secret_must_not_leak"),
                "错误消息不应含 secret_key 值：{err}"
            );
        }
    }

    #[test]
    fn build_provider_tencent_with_all_fields_succeeds() {
        let creds = vec![
            ("secret_id".to_string(), "sid1".to_string()),
            ("secret_key".to_string(), "sk1".to_string()),
        ];
        let result = build_provider("tencent", &creds);
        assert!(result.is_ok(), "腾讯全字段应成功");
        assert_eq!(result.unwrap().capability().id, "tencent");
    }

    #[test]
    fn registry_contains_tencent_keyed_official() {
        let reg = registry();
        let t = reg
            .iter()
            .find(|c| c.id == "tencent")
            .expect("注册表应含 tencent");
        assert!(t.needs_key, "tencent 应为需 key 源");
        assert!(
            !t.is_unofficial,
            "tencent 为官方 API，is_unofficial 应为 false"
        );
    }

    // 阿里翻译（HMAC-SHA1 + Base64，需 key，官方 API 文档协议）测试

    // 对齐 acceptance TV2-F3-A01：alibaba_hmac_sign 纯函数对固定输入产出确定 Base64 签名。
    // 参照向量由独立 Python 实现按阿里云 RPC 签名官方文档（help.aliyun.com/document_detail/30563）
    // 计算（非 pot 源码），固定参数下手算核对。
    #[test]
    fn alibaba_hmac_signature_deterministic() {
        // 固定全套规范化参数（公共 + 业务），按官方 RPC 签名机制计算。
        let params: Vec<(&str, &str)> = vec![
            ("AccessKeyId", "LTAItest_access_id"),
            ("Action", "TranslateGeneral"),
            ("Format", "JSON"),
            ("FormatType", "text"),
            ("Scene", "general"),
            ("SignatureMethod", "HMAC-SHA1"),
            ("SignatureNonce", "fixed-nonce-123"),
            ("SignatureVersion", "1.0"),
            ("SourceLanguage", "en"),
            ("SourceText", "hello"),
            ("TargetLanguage", "zh"),
            ("Timestamp", "2023-11-14T00:00:00Z"),
            ("Version", "2018-10-12"),
        ];
        let sig = alibaba_hmac_sign("GET", &params, "test_access_secret");
        assert_eq!(
            sig, "+uwyBbn3LNXWPJOuNcXCiWB/32k=",
            "alibaba HMAC-SHA1 签名应为按官方文档计算的确定 Base64 值，实际：{sig}"
        );

        // 同输入二次调用应稳定一致（纯函数确定性）。
        let sig2 = alibaba_hmac_sign("GET", &params, "test_access_secret");
        assert_eq!(sig, sig2, "同输入两次签名应一致（确定性）");
    }

    // 对齐 acceptance TV2-F3-A01：build_request 端点/签名参数正确；
    // parse_response 取 Data.Translated；错误响应→TranslateError。
    #[test]
    fn alibaba_build_and_parse() {
        let provider = AlibabaProvider::new("access_id_1", "access_secret_1");
        let req = TranslateRequest {
            text: "hello".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "GET");
        assert!(
            http_req
                .url
                .starts_with("http://mt.cn-hangzhou.aliyuncs.com/"),
            "URL 应为阿里翻译端点，实际：{}",
            http_req.url
        );
        // URL query 应含签名机制参数与签名本身。
        assert!(
            http_req.url.contains("Action=TranslateGeneral"),
            "URL 应含 Action=TranslateGeneral，实际：{}",
            http_req.url
        );
        assert!(
            http_req.url.contains("SignatureMethod=HMAC-SHA1"),
            "URL 应含 SignatureMethod=HMAC-SHA1"
        );
        assert!(
            http_req.url.contains("AccessKeyId=access_id_1"),
            "URL 应含 AccessKeyId"
        );
        assert!(
            http_req.url.contains("Signature="),
            "URL 应含 Signature 参数，实际：{}",
            http_req.url
        );
        assert!(
            http_req.url.contains("SourceText=hello"),
            "URL 应含 SourceText=hello"
        );

        // 成功响应：取 Data.Translated。
        let ok = provider
            .parse_response(r#"{"Code":"200","Data":{"WordCount":"5","Translated":"你好"},"RequestId":"r1"}"#)
            .expect("含 Data.Translated 应解析成功");
        assert_eq!(plain_text(&ok), "你好");

        // 错误响应（Code 非 200 + 鉴权类 Message）→ Auth。
        let err = provider.parse_response(
            r#"{"Code":"InvalidAccessKeyId.NotFound","Message":"Specified access key is not found.","RequestId":"r2"}"#,
        );
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "鉴权类错误应归一为 Auth，实际：{err:?}"
        );

        // 限流错误归一为 RateLimit。
        let rate = provider.parse_response(
            r#"{"Code":"Throttling.User","Message":"Request was denied due to user flow control.","RequestId":"r3"}"#,
        );
        assert!(
            matches!(rate, Err(TranslateError::RateLimit(_))),
            "Throttling.* 应归一为 RateLimit，实际：{rate:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn build_provider_alibaba_missing_secret_returns_err() {
        // 缺 accesskey_secret 必填字段应报错，错误消息不含任何字段值（安全约定 TV2-F5-A01）。
        let creds = vec![
            ("accesskey_id".to_string(), "aid1".to_string()),
            (
                "accesskey_secre".to_string(),
                "as_secret_must_not_leak".to_string(),
            ),
        ];
        let result = build_provider("alibaba", &creds);
        assert!(result.is_err(), "缺 accesskey_secret 应返回 Err");
        if let Err(err) = result {
            assert!(
                !err.contains("as_secret_must_not_leak"),
                "错误消息不应含 accesskey_secret 值：{err}"
            );
        }
    }

    #[test]
    fn build_provider_alibaba_with_all_fields_succeeds() {
        let creds = vec![
            ("accesskey_id".to_string(), "aid1".to_string()),
            ("accesskey_secret".to_string(), "as1".to_string()),
        ];
        let result = build_provider("alibaba", &creds);
        assert!(result.is_ok(), "阿里全字段应成功");
        assert_eq!(result.unwrap().capability().id, "alibaba");
    }

    #[test]
    fn registry_contains_alibaba_keyed_official() {
        let reg = registry();
        let a = reg
            .iter()
            .find(|c| c.id == "alibaba")
            .expect("注册表应含 alibaba");
        assert!(a.needs_key, "alibaba 应为需 key 源");
        assert!(
            !a.is_unofficial,
            "alibaba 为官方 API，is_unofficial 应为 false"
        );
    }

    // 火山引擎翻译（AWS SigV4 风格四层 HMAC-SHA256，需 key，官方 API 文档协议）测试

    // 对齐 acceptance TV2-F4-A01：volcengine_sigv4_sign 纯函数对固定输入产出确定 Authorization。
    // 参照向量由独立 Python 实现按火山引擎签名 V4 官方文档（volcengine.com/docs/6369/67269）
    // 计算（非 pot 源码），固定 access_key/secret/region/payload/timestamp 下手算核对。
    #[test]
    fn volcengine_sigv4_signature_deterministic() {
        let payload =
            r#"{"SourceLanguage":"en","TargetLanguage":"zh","TextList":["hello"]}"#;
        let auth = volcengine_sigv4_sign(
            "AKLTtest_access_key_id_123",
            "test_secret_access_key_abc",
            "cn-north-1",
            payload,
            1_700_000_000,
        );

        // timestamp=1700000000 对应 UTC X-Date 20231114T221320Z；credential scope 用短日期 20231114。
        let expected = "HMAC-SHA256 Credential=AKLTtest_access_key_id_123/20231114/cn-north-1/translate/request, \
SignedHeaders=content-type;host;x-content-sha256;x-date, \
Signature=dac06f9e1be8667102fc1dfe025834cc9da68f2359b28084891b8e03ee332a61";
        assert_eq!(
            auth, expected,
            "volcengine SigV4 Authorization 应为按官方文档计算的确定值，实际：{auth}"
        );

        // 同输入二次调用应稳定一致（纯函数确定性）。
        let auth2 = volcengine_sigv4_sign(
            "AKLTtest_access_key_id_123",
            "test_secret_access_key_abc",
            "cn-north-1",
            payload,
            1_700_000_000,
        );
        assert_eq!(auth, auth2, "同输入两次签名应一致（确定性）");
    }

    // 对齐 acceptance TV2-F4-A01：build_request 端点/头/JSON body 正确；
    // parse_response 取 TranslationList[].Translation；错误响应→TranslateError。
    #[test]
    fn volcengine_build_and_parse() {
        let provider =
            VolcengineProvider::new("access_id_1", "access_secret_1", "cn-north-1");
        let req = TranslateRequest {
            text: "hello".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url,
            "https://open.volcengineapi.com/?Action=TranslateText&Version=2020-06-01",
            "URL 应为火山翻译端点，实际：{}",
            http_req.url
        );

        let header = |name: &str| {
            http_req
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(
            header("Content-Type"),
            Some("application/json"),
            "应含 Content-Type: application/json 头，实际：{:?}",
            http_req.headers
        );
        assert!(
            header("Host").is_some(),
            "应含 Host 头，实际：{:?}",
            http_req.headers
        );
        assert!(header("X-Date").is_some(), "应含 X-Date 头");
        assert!(
            header("X-Content-Sha256").is_some(),
            "应含 X-Content-Sha256 头"
        );
        let auth = header("Authorization").expect("应含 Authorization 头");
        assert!(
            auth.starts_with("HMAC-SHA256 Credential=access_id_1/"),
            "Authorization 应为 SigV4 签名头，实际：{auth}"
        );

        // body 为合法 JSON，含 SourceLanguage/TargetLanguage/TextList。
        let body = http_req.body.expect("火山为 POST，应有 body");
        let parsed: serde_json::Value =
            serde_json::from_str(&body).expect("body 应为合法 JSON");
        assert_eq!(parsed["SourceLanguage"], "en", "body 应含 SourceLanguage=en");
        assert_eq!(parsed["TargetLanguage"], "zh", "body 应含 TargetLanguage=zh");
        assert_eq!(
            parsed["TextList"][0], "hello",
            "body TextList[0] 应为待译文本"
        );

        // 成功响应：取 TranslationList[].Translation 拼接。
        let ok = provider
            .parse_response(
                r#"{"TranslationList":[{"Translation":"你好","DetectedSourceLanguage":"en"}]}"#,
            )
            .expect("含 TranslationList[].Translation 应解析成功");
        assert_eq!(plain_text(&ok), "你好");

        // 多段译文应按序拼接。
        let multi = provider
            .parse_response(
                r#"{"TranslationList":[{"Translation":"你好"},{"Translation":"世界"}]}"#,
            )
            .expect("多段译文应解析成功");
        assert_eq!(plain_text(&multi), "你好世界", "多段译文应拼接");

        // 错误响应（ResponseMetadata.Error）→ TranslateError。
        // SignatureDoesNotMatch 应归一为 Auth。
        let err = provider.parse_response(
            r#"{"ResponseMetadata":{"Error":{"Code":"SignatureDoesNotMatch","Message":"signature mismatch"},"RequestId":"r1"}}"#,
        );
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "SignatureDoesNotMatch 应归一为 Auth，实际：{err:?}"
        );

        // 限流错误归一为 RateLimit。
        let rate = provider.parse_response(
            r#"{"ResponseMetadata":{"Error":{"Code":"FlowLimitExceeded","Message":"too many requests"},"RequestId":"r2"}}"#,
        );
        assert!(
            matches!(rate, Err(TranslateError::RateLimit(_))),
            "FlowLimitExceeded 应归一为 RateLimit，实际：{rate:?}"
        );

        // 非法 JSON → ParseError。
        let invalid = provider.parse_response("not json");
        assert!(
            matches!(invalid, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{invalid:?}"
        );
    }

    #[test]
    fn build_provider_volcengine_missing_secret_returns_err() {
        // 缺 secret_access_key 必填字段应报错，错误消息不含任何字段值（安全约定 TV2-F5-A01）。
        // 传错拼 key 的脏密钥：secret_access_key 仍缺失走缺字段路径，坐实错误消息不泄露脏值。
        let creds = vec![
            ("access_key_id".to_string(), "akid1".to_string()),
            (
                "secret_access_ke".to_string(),
                "sak_secret_must_not_leak".to_string(),
            ),
        ];
        let result = build_provider("volcengine", &creds);
        assert!(result.is_err(), "缺 secret_access_key 应返回 Err");
        if let Err(err) = result {
            assert!(
                !err.contains("sak_secret_must_not_leak"),
                "错误消息不应含 secret_access_key 值：{err}"
            );
        }
    }

    #[test]
    fn build_provider_volcengine_with_all_fields_succeeds() {
        // region 非必填（有默认 cn-north-1），仅 access_key_id + secret_access_key 即可构造。
        let creds = vec![
            ("access_key_id".to_string(), "akid1".to_string()),
            ("secret_access_key".to_string(), "sak1".to_string()),
        ];
        let result = build_provider("volcengine", &creds);
        assert!(result.is_ok(), "火山全必填字段应成功");
        assert_eq!(result.unwrap().capability().id, "volcengine");
    }

    #[test]
    fn registry_contains_volcengine_keyed_official() {
        let reg = registry();
        let v = reg
            .iter()
            .find(|c| c.id == "volcengine")
            .expect("注册表应含 volcengine");
        assert!(v.needs_key, "volcengine 应为需 key 源");
        assert!(
            !v.is_unofficial,
            "volcengine 为官方 API，is_unofficial 应为 false"
        );
    }

    // TV3-F1 OpenAI + Ollama（chat/completions）+ Prompt 模板引擎

    fn llm_req() -> TranslateRequest {
        TranslateRequest {
            text: "hello".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        }
    }

    // 对齐 acceptance TV3-F1-A01 / TV3-F3-A01（Prompt 部分）：
    // 自定义模板下 render_prompt 的 $text/$from/$to 占位被替换为实际请求值。
    #[test]
    fn prompt_template_substitutes_text_from_to() {
        let template = "translate from $from to $to: $text";
        let messages = render_prompt(Some(template), &llm_req());

        let user = messages
            .iter()
            .find(|m| m.role == "user")
            .expect("应含 user 消息");
        assert_eq!(
            user.content, "translate from en to zh: hello",
            "$from/$to/$text 应分别替换为 en/zh/hello，实际：{}",
            user.content
        );
    }

    // 对齐 acceptance TV3-F3-A01（Prompt 缺省回退）：
    // 模板为 None 时回退内置默认翻译 Prompt，仍产出含目标语言与原文的 system+user。
    #[test]
    fn prompt_template_falls_back_to_default() {
        let messages = render_prompt(None, &llm_req());

        let system = messages
            .iter()
            .find(|m| m.role == "system")
            .expect("默认 Prompt 应含 system 指示");
        assert!(
            system.content.contains("zh"),
            "默认 system 应含目标语言 zh，实际：{}",
            system.content
        );
        let user = messages
            .iter()
            .find(|m| m.role == "user")
            .expect("默认 Prompt 应含 user 消息");
        assert_eq!(user.content, "hello", "默认 user 消息应为原文 hello");
    }

    // 对齐 acceptance TV3-F1-A01：OpenAI build_request 端点/Bearer 头/messages，
    // parse_response 取 choices[0].message.content。
    #[test]
    fn openai_build_request_and_parse() {
        let provider = OpenAiProvider::new("SENTINEL_DEADBEEF", "gpt-4o-mini", "", "");
        let http_req = provider.build_request(&llm_req());

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url, "https://api.openai.com/v1/chat/completions",
            "URL 应为 OpenAI chat/completions 端点，实际：{}",
            http_req.url
        );
        let header = |name: &str| {
            http_req
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(
            header("Authorization"),
            Some("Bearer SENTINEL_DEADBEEF"),
            "应含 Bearer 鉴权头"
        );

        let body = http_req.body.expect("OpenAI 为 POST，应有 body");
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("body 应为合法 JSON");
        assert_eq!(parsed["model"], "gpt-4o-mini", "body 应含 model");
        assert_eq!(parsed["stream"], false, "应为非流式 stream=false");
        let messages = parsed["messages"]
            .as_array()
            .expect("body 应含 messages 数组");
        assert_eq!(messages.len(), 2, "默认 Prompt 应产出 system+user 两条");
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "hello", "user 内容应为原文");

        let ok = provider
            .parse_response(r#"{"choices":[{"message":{"role":"assistant","content":"你好"}}]}"#)
            .expect("含 choices[0].message.content 应解析成功");
        assert_eq!(plain_text(&ok), "你好");
    }

    // 对齐 acceptance TV3-F1-A01：OpenAI 错误响应体 → TranslateError（按 type/code 分类）。
    #[test]
    fn openai_parse_error_response() {
        let provider = OpenAiProvider::new("SENTINEL_DEADBEEF", "gpt-4o-mini", "", "");

        let auth_err = provider.parse_response(
            r#"{"error":{"message":"Incorrect API key","type":"invalid_request_error","code":"invalid_api_key"}}"#,
        );
        assert!(
            matches!(auth_err, Err(TranslateError::Auth(_))),
            "invalid_api_key 应归类为 Auth，实际：{auth_err:?}"
        );

        let rate_err = provider.parse_response(
            r#"{"error":{"message":"Rate limit reached","type":"requests","code":"rate_limit_exceeded"}}"#,
        );
        assert!(
            matches!(rate_err, Err(TranslateError::RateLimit(_))),
            "rate_limit_exceeded 应归类为 RateLimit，实际：{rate_err:?}"
        );

        // 错误消息不得泄露 apiKey 脏值（sentinel，非空可识别值）。
        if let Err(e) = auth_err {
            assert!(
                !e.to_string().contains("SENTINEL_DEADBEEF"),
                "错误消息不得含 apiKey"
            );
        }
    }

    // 对齐 acceptance TV3-F1-A01：Ollama build_request 端点/无鉴权头/messages，
    // parse_response 取 message.content。
    #[test]
    fn ollama_build_request_and_parse() {
        let provider = OllamaProvider::new("llama3", "", "");
        let http_req = provider.build_request(&llm_req());

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url, "http://localhost:11434/api/chat",
            "URL 应为 Ollama /api/chat 端点，实际：{}",
            http_req.url
        );

        let body = http_req.body.expect("Ollama 为 POST，应有 body");
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("body 应为合法 JSON");
        assert_eq!(parsed["model"], "llama3", "body 应含 model");
        assert_eq!(parsed["stream"], false, "应为非流式 stream=false");
        let messages = parsed["messages"]
            .as_array()
            .expect("body 应含 messages 数组");
        assert_eq!(messages.len(), 2, "默认 Prompt 应产出 system+user 两条");

        let ok = provider
            .parse_response(r#"{"message":{"role":"assistant","content":"你好"},"done":true}"#)
            .expect("含 message.content 应解析成功");
        assert_eq!(plain_text(&ok), "你好");
    }

    // 对齐 acceptance TV3-F1-A01：Ollama 本地自部署无鉴权，绝不发 Authorization 头。
    #[test]
    fn ollama_local_no_auth_header() {
        let provider = OllamaProvider::new("llama3", "", "");
        let http_req = provider.build_request(&llm_req());

        assert!(
            http_req
                .headers
                .iter()
                .all(|(k, _)| !k.eq_ignore_ascii_case("Authorization")),
            "Ollama 本地无鉴权，不应发 Authorization 头，实际：{:?}",
            http_req.headers
        );
        let cap = provider.capability();
        assert!(!cap.needs_key, "Ollama 本地自部署，needs_key 应为 false");
    }

    // 自定义 base_url 覆盖默认端点（OpenAI 兼容网关场景）。
    #[test]
    fn openai_custom_base_url_overrides_default() {
        let provider = OpenAiProvider::new("k", "gpt-4o-mini", "https://gw.example.com", "");
        let http_req = provider.build_request(&llm_req());
        assert_eq!(
            http_req.url, "https://gw.example.com/v1/chat/completions",
            "自定义 base_url 应覆盖默认端点"
        );
    }

    // build_provider 缺必填字段（OpenAI apiKey）应明确报错，不含字段值。
    #[test]
    fn build_provider_openai_missing_fields_returns_err() {
        let result = build_provider("openai", &[]);
        assert!(result.is_err(), "openai 缺 apiKey 应返回 Err");
        if let Err(err) = result {
            assert!(err.contains("未配置"), "错误应提示未配置：{err}");
            assert!(!err.contains("SENTINEL_DEADBEEF"), "错误不应含字段值");
        }
    }

    // 对齐 acceptance TV3-F3-A01（缺必填字段错误路径）：
    // 4 个 LLM 源缺必填字段时 build_provider 返回明确错误，且错误消息不含任何字段值。
    //
    // 防泄露用非空 sentinel 脏值（hints TV2-RETRO-1）：仅填部分字段、留一必填项缺失，
    // 已填字段用可识别脏值，断言错误消息 !contains 该脏值（空值占位是恒真假绿）。
    #[test]
    fn build_provider_llm_missing_field_errors() {
        const DIRTY: &str = "SENTINEL_DEADBEEF";

        // build_provider 返回的 Ok 内层 Box<dyn TranslateProvider> 不实现 Debug，
        // 故不能用 unwrap_err；提取错误字符串（应为 Err）。
        let err_of = |provider_id: &str, creds: &[(String, String)]| -> String {
            match build_provider(provider_id, creds) {
                Ok(_) => panic!("{provider_id} 缺必填字段应返回 Err，却得到 Ok"),
                Err(e) => e,
            }
        };

        // 用例表：(源, 已填字段)。每例只缺一个必填字段，其余填脏值。
        let cases: &[(&str, Vec<(String, String)>)] = &[
            // OpenAI：缺 apiKey（model/base_url 填脏值）。
            (
                "openai",
                vec![
                    ("model".to_string(), DIRTY.to_string()),
                    ("base_url".to_string(), DIRTY.to_string()),
                ],
            ),
            // Ollama：本地无 key，唯一必填是 model；缺 model（仅填 base_url）。
            ("ollama", vec![("base_url".to_string(), DIRTY.to_string())]),
            // ChatGLM：缺 apiKey（model 填脏值）。
            ("chatglm", vec![("model".to_string(), DIRTY.to_string())]),
            // Gemini：缺 model（apiKey 填脏值——apiKey 是 secret，尤须不泄露）。
            ("gemini", vec![("apiKey".to_string(), DIRTY.to_string())]),
        ];

        for (provider_id, creds) in cases {
            let err = err_of(provider_id, creds);
            assert!(
                err.contains("未配置"),
                "{provider_id} 缺必填字段错误应提示未配置：{err}"
            );
            assert!(
                !err.contains(DIRTY),
                "{provider_id} 错误不应含任何字段值（防 secret 泄露）：{err}"
            );
        }
    }

    #[test]
    fn build_provider_ollama_with_model_succeeds() {
        let creds = vec![("model".to_string(), "llama3".to_string())];
        let result = build_provider("ollama", &creds);
        assert!(result.is_ok(), "ollama 有 model 应成功");
        assert_eq!(result.unwrap().capability().id, "ollama");
    }

    #[test]
    fn registry_contains_openai_and_ollama() {
        let reg = registry();
        let openai = reg
            .iter()
            .find(|c| c.id == "openai")
            .expect("注册表应含 openai");
        assert!(openai.needs_key, "openai 应为需 key 源");
        assert!(!openai.is_unofficial, "openai 为官方 API");
        let ollama = reg
            .iter()
            .find(|c| c.id == "ollama")
            .expect("注册表应含 ollama");
        assert!(!ollama.needs_key, "ollama 本地无 key");
        assert!(!ollama.is_unofficial, "ollama 为本地自部署官方运行时");
    }

    // 对齐 acceptance TV3-F2-A01：ChatGLM 手搓 JWT HS256 的签名确定性。
    //
    // 参照 token 由独立 Python（hmac+hashlib+base64）按 JWT HS256 手算
    // （固定 id/secret/exp/timestamp，header {"alg":"HS256","sign_type":"SIGN"}、
    //  payload {"api_key","exp","timestamp"}，base64url 无填充），断言本实现产出与之逐字相等，
    //  非「等于本实现自己的输出」的循环论证（见 hints TV2 复杂签名独立复算锚定）。
    #[test]
    fn chatglm_jwt_hs256_deterministic() {
        // 固定输入：与 Python 参照向量同。exp/timestamp 注入固定值使签名确定。
        let token = chatglm_jwt(
            "test_id_12345",
            "test_secret_67890",
            1_717_632_000_000,
            1_717_631_700_000,
        );

        // 三段 base64url 参照常量（Python 独立复算）。
        let expected = "eyJhbGciOiJIUzI1NiIsInNpZ25fdHlwZSI6IlNJR04ifQ.eyJhcGlfa2V5IjoidGVzdF9pZF8xMjM0NSIsImV4cCI6MTcxNzYzMjAwMDAwMCwidGltZXN0YW1wIjoxNzE3NjMxNzAwMDAwfQ.p-yF6cb9lFXduM5xA4qbBQkjTckbRU9tTFfO2IIIf4M";
        assert_eq!(
            token, expected,
            "JWT token 应与独立 Python 复算的参照向量逐字相等"
        );
    }

    // 对齐 acceptance TV3-F2-A01：ChatGLM build_request 端点/JWT 鉴权头/messages，
    // parse_response 取 choices[0].message.content（OpenAI 兼容 chat/completions 形态）。
    #[test]
    fn chatglm_build_request_and_parse() {
        // apiKey 形如 {id}.{secret}；用 sentinel 脏值证否泄露。
        let provider = ChatGlmProvider::new("SENTINELID.SENTINEL_DEADBEEF", "glm-4", "", "");
        let http_req = provider.build_request(&llm_req());

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url, "https://open.bigmodel.cn/api/paas/v4/chat/completions",
            "URL 应为智谱 chat/completions 端点，实际：{}",
            http_req.url
        );
        let header = |name: &str| {
            http_req
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        };
        let auth = header("Authorization").expect("应含 Authorization 头");
        // 智谱官方要求 `Authorization: Bearer <JWT>`——独立守卫前缀，防裸 JWT 头（真请求 401）bug 类复发。
        assert!(
            auth.starts_with("Bearer "),
            "Authorization 应带 Bearer 前缀，实际：{auth}"
        );
        // Authorization 头承载 JWT（三段 base64url，header.payload.signature），不是裸 secret。
        assert_eq!(
            auth.matches('.').count(),
            2,
            "Authorization 应为三段 JWT，实际：{auth}"
        );
        assert!(
            !auth.contains("SENTINEL_DEADBEEF"),
            "Authorization 头不得明文含 secret 脏值"
        );
        assert!(
            !auth.contains("SENTINELID"),
            "Authorization 头不得明文含 id 脏值（应为 base64url 编码后的 payload）"
        );

        let body = http_req.body.expect("ChatGLM 为 POST，应有 body");
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("body 应为合法 JSON");
        assert_eq!(parsed["model"], "glm-4", "body 应含 model");
        assert_eq!(parsed["stream"], false, "应为非流式 stream=false");
        let messages = parsed["messages"]
            .as_array()
            .expect("body 应含 messages 数组");
        assert_eq!(messages.len(), 2, "默认 Prompt 应产出 system+user 两条");
        assert_eq!(messages[1]["content"], "hello", "user 内容应为原文");

        let ok = provider
            .parse_response(r#"{"choices":[{"message":{"role":"assistant","content":"你好"}}]}"#)
            .expect("含 choices[0].message.content 应解析成功");
        assert_eq!(plain_text(&ok), "你好");
    }

    // 对齐 acceptance TV3-F2-A01：Gemini key 作 URL query 参（不进头），body contents/parts，
    // parse_response 取 candidates[0].content.parts[0].text。
    #[test]
    fn gemini_build_request_url_key_and_parse() {
        let provider = GeminiProvider::new("SENTINEL_DEADBEEF", "gemini-pro", "", "");
        let http_req = provider.build_request(&llm_req());

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent?key=SENTINEL_DEADBEEF",
            "URL 应为 Gemini generateContent 端点且 key 作 query 参，实际：{}",
            http_req.url
        );
        // 独立守卫：URL 必须以 ?key={apiKey} 携带 key（防 format! 漏占位符致 URL 不带 key 的 bug 类复发）。
        assert!(
            http_req.url.contains("?key=SENTINEL_DEADBEEF"),
            "URL 必须以 ?key= 携带 apiKey，实际：{}",
            http_req.url
        );
        // key 在 URL query，绝不进 Authorization 头。
        assert!(
            http_req
                .headers
                .iter()
                .all(|(k, _)| !k.eq_ignore_ascii_case("Authorization")),
            "Gemini key 走 URL query，不应发 Authorization 头，实际：{:?}",
            http_req.headers
        );

        let body = http_req.body.expect("Gemini 为 POST，应有 body");
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("body 应为合法 JSON");
        // body 形如 {"contents":[{"parts":[{"text":...}]}]}，原文进 parts[].text。
        let text = parsed["contents"][0]["parts"][0]["text"]
            .as_str()
            .expect("body 应含 contents[0].parts[0].text");
        assert_eq!(text, "hello", "contents 的 text 应为原文");

        let ok = provider
            .parse_response(
                r#"{"candidates":[{"content":{"parts":[{"text":"你好"}],"role":"model"}}]}"#,
            )
            .expect("含 candidates[0].content.parts[0].text 应解析成功");
        assert_eq!(plain_text(&ok), "你好");
    }

    // 对齐 acceptance TV3-F2-A01：Gemini 错误响应体 → TranslateError，错误消息不泄露 key。
    #[test]
    fn gemini_parse_error_response() {
        let provider = GeminiProvider::new("SENTINEL_DEADBEEF", "gemini-pro", "", "");

        let err = provider.parse_response(
            r#"{"error":{"code":400,"message":"API key not valid","status":"INVALID_ARGUMENT"}}"#,
        );
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "API key not valid 应归类为 Auth，实际：{err:?}"
        );
        if let Err(e) = err {
            assert!(
                !e.to_string().contains("SENTINEL_DEADBEEF"),
                "错误消息不得含 apiKey"
            );
        }

        let server_err = provider
            .parse_response(r#"{"error":{"code":500,"message":"internal","status":"INTERNAL"}}"#);
        assert!(
            matches!(server_err, Err(TranslateError::ServerError(_))),
            "500 INTERNAL 应归类为 ServerError，实际：{server_err:?}"
        );
    }

    #[test]
    fn registry_contains_chatglm_and_gemini() {
        let reg = registry();
        let chatglm = reg
            .iter()
            .find(|c| c.id == "chatglm")
            .expect("注册表应含 chatglm");
        assert!(chatglm.needs_key, "chatglm 应为需 key 源");
        assert!(!chatglm.is_unofficial, "chatglm 为官方 API");
        let gemini = reg
            .iter()
            .find(|c| c.id == "gemini")
            .expect("注册表应含 gemini");
        assert!(gemini.needs_key, "gemini 应为需 key 源");
        assert!(!gemini.is_unofficial, "gemini 为官方 API");
    }

    // ECDICT 词典源（pot-app.com/api/dict POST，免 key，pot 自建）测试

    /// 取出 Dict 变体的词条；非 Dict 即测试失败，让词典断言聚焦结构化字段。
    fn dict_entry(resp: &TranslateResponse) -> &super::super::DictEntry {
        match resp {
            TranslateResponse::Dict { entry } => entry,
            TranslateResponse::Plain { .. } => panic!("应返回 Dict，实际返回 Plain"),
        }
    }

    // 对齐 acceptance TV4-F2-A01：build_request 端点/方法/body 正确；
    // parse_response 把 ECDICT 英汉词条解析为 Dict（音标/按词性分组释义/词形）。
    #[test]
    fn ecdict_build_and_parse_dict() {
        let provider = EcdictProvider::new();
        let req = TranslateRequest {
            text: "glacier".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url, "https://pot-app.com/api/dict",
            "URL 应为 pot-app dict 端点，实际：{}",
            http_req.url
        );
        let body = http_req.body.as_deref().expect("ECDICT POST 应带 body");
        assert!(
            body.contains("glacier"),
            "body 应携带待查词 glacier，实际：{body}"
        );

        // 录制响应：ECDICT 行结构（word/phonetic/translation 按词性分行/exchange 词形）。
        let raw = r#"{
            "word": "glacier",
            "phonetic": "ˈɡleɪʃər",
            "translation": "n. 冰川，冰河\nvt. 测试动词义",
            "exchange": "s:glaciers/p:glacial"
        }"#;
        let resp = provider.parse_response(raw).expect("ECDICT 词条应解析为 Dict");
        let entry = dict_entry(&resp);

        assert_eq!(entry.phonetic.as_deref(), Some("ˈɡleɪʃər"), "应取 phonetic");
        // translation 按词性前缀（n./vt.）分组。
        let noun = entry
            .definitions
            .iter()
            .find(|d| d.pos.as_deref() == Some("n."))
            .expect("应含名词词性分组");
        assert!(
            noun.meanings.iter().any(|m| m.contains("冰川")),
            "名词释义应含「冰川」，实际：{:?}",
            noun.meanings
        );
        let verb = entry
            .definitions
            .iter()
            .find(|d| d.pos.as_deref() == Some("vt."))
            .expect("应含及物动词词性分组");
        assert!(
            verb.meanings.iter().any(|m| m.contains("测试动词义")),
            "动词释义应含「测试动词义」，实际：{:?}",
            verb.meanings
        );
        // exchange 解析为词形列表。
        assert!(
            entry.inflections.iter().any(|i| i == "glaciers"),
            "词形应含复数 glaciers，实际：{:?}",
            entry.inflections
        );
    }

    #[test]
    fn ecdict_parse_invalid_json_returns_parse_error() {
        let provider = EcdictProvider::new();
        let err = provider.parse_response("not json");
        assert!(
            matches!(err, Err(TranslateError::ParseError(_))),
            "非法 JSON 应返回 ParseError，实际：{err:?}"
        );
    }

    #[test]
    fn ecdict_parse_empty_word_returns_parse_error() {
        let provider = EcdictProvider::new();
        // 无 word/translation 的空响应（非词或未收录）应报 ParseError，不 panic。
        let err = provider.parse_response(r#"{"word":"","translation":""}"#);
        assert!(
            matches!(err, Err(TranslateError::ParseError(_))),
            "空词条应返回 ParseError，实际：{err:?}"
        );
    }

    #[test]
    fn registry_contains_ecdict_free_unofficial() {
        let reg = registry();
        let e = reg
            .iter()
            .find(|c| c.id == "ecdict")
            .expect("注册表应含 ecdict");
        assert!(!e.needs_key, "ecdict 应为免 key 源");
        assert!(
            e.is_unofficial,
            "ecdict 为 pot 自建公共服务，is_unofficial 应为 true"
        );
        assert!(
            build_provider("ecdict", &[]).is_ok(),
            "build_provider(\"ecdict\") 应免 key 成功"
        );
    }

    // 有道词典模式（同有道签名，isWord 模式，需 key）测试

    // 对齐 acceptance TV4-F2-A01：isWord===true 且含 basic 时，
    // 取 basic 音标/explains/词形为 Dict(DictEntry)。
    #[test]
    fn youdao_dict_parses_basic_to_dict() {
        let provider = YoudaoDictProvider::new("app123", "sec456");
        // 录制有道 isWord 响应：basic 含 phonetic/us-phonetic/explains/wfs。
        let raw = r#"{
            "errorCode": "0",
            "query": "glacier",
            "isWord": true,
            "translation": ["冰川"],
            "basic": {
                "phonetic": "ˈɡleɪʃər",
                "us-phonetic": "ˈɡleɪʃər",
                "uk-phonetic": "ˈɡlasɪə",
                "explains": ["n. 冰川，冰河"],
                "wfs": [{"wf": {"name": "复数", "value": "glaciers"}}]
            }
        }"#;
        let resp = provider
            .parse_response(raw)
            .expect("isWord 响应应解析为 Dict");
        let entry = dict_entry(&resp);

        assert_eq!(
            entry.phonetic.as_deref(),
            Some("ˈɡleɪʃər"),
            "应优先取 us-phonetic/phonetic"
        );
        assert!(
            entry
                .definitions
                .iter()
                .flat_map(|d| &d.meanings)
                .any(|m| m.contains("冰川")),
            "释义应含 explains 的「冰川」，实际：{:?}",
            entry.definitions
        );
        assert!(
            entry.inflections.iter().any(|i| i.contains("glaciers")),
            "词形应含 wfs 的复数 glaciers，实际：{:?}",
            entry.inflections
        );
    }

    // 对齐 acceptance TV4-F2-A01：非词（isWord!=true 或无 basic）回退 Plain（取 translation）。
    #[test]
    fn youdao_dict_falls_back_to_plain_when_not_word() {
        let provider = YoudaoDictProvider::new("app123", "sec456");
        // 非词响应：isWord=false、无 basic，应回退 Plain 取 translation 拼接。
        let raw = r#"{
            "errorCode": "0",
            "query": "hello world this is a sentence",
            "isWord": false,
            "translation": ["你好世界这是一个句子"]
        }"#;
        let resp = provider.parse_response(raw).expect("非词响应应回退 Plain");
        assert_eq!(plain_text(&resp), "你好世界这是一个句子");
    }

    #[test]
    fn youdao_dict_error_code_maps_to_error() {
        let provider = YoudaoDictProvider::new("app123", "sec456");
        let err = provider.parse_response(r#"{"errorCode":"108","translation":null}"#);
        assert!(
            matches!(err, Err(TranslateError::Auth(_))),
            "108 应用密钥无效应归一为 Auth，实际：{err:?}"
        );
    }

    // 安全：有道词典复用有道签名，请求 body 不得泄露 app_secret（用 sentinel 脏值证否）。
    #[test]
    fn youdao_dict_build_request_does_not_leak_secret() {
        let provider = YoudaoDictProvider::new("app123", "SENTINEL_DEADBEEF");
        let req = TranslateRequest {
            text: "glacier".to_string(),
            source_lang: Lang::new("en"),
            target_lang: Lang::new("zh"),
        };
        let http_req = provider.build_request(&req);

        assert_eq!(http_req.method, "POST");
        assert_eq!(
            http_req.url, "https://openapi.youdao.com/api",
            "应复用有道翻译端点，实际：{}",
            http_req.url
        );
        let body = http_req.body.as_deref().expect("应带 body");
        assert!(
            !body.contains("SENTINEL_DEADBEEF"),
            "请求 body 不得明文包含 app_secret"
        );
        // 签名应等于复用 youdao_sign 对 build 内实际 salt/curtime 重算的值（不另起算法）。
        let salt = extract_form_field(body, "salt").expect("body 应含 salt");
        let curtime = extract_form_field(body, "curtime").expect("body 应含 curtime");
        let actual_sign = extract_form_field(body, "sign").expect("body 应含 sign");
        let expected = youdao_sign("app123", "glacier", &salt, &curtime, "SENTINEL_DEADBEEF");
        assert_eq!(
            actual_sign, expected,
            "有道词典应复用 youdao_sign 签名，不另起算法"
        );
    }

    #[test]
    fn build_provider_youdao_dict_missing_required_fields_returns_err() {
        let creds = vec![("app_key".to_string(), "ak1".to_string())];
        let result = build_provider("youdao_dict", &creds);
        assert!(result.is_err(), "缺 app_secret 应返回 Err");
    }

    #[test]
    fn registry_contains_youdao_dict_keyed() {
        let reg = registry();
        let y = reg
            .iter()
            .find(|c| c.id == "youdao_dict")
            .expect("注册表应含 youdao_dict");
        assert!(y.needs_key, "youdao_dict 应为需 key 源");
    }

    /// 从 `application/x-www-form-urlencoded` body 中取出指定字段值（测试辅助）。
    fn extract_form_field(body: &str, key: &str) -> Option<String> {
        body.split('&')
            .find_map(|pair| pair.strip_prefix(&format!("{key}=")))
            .map(|v| v.to_string())
    }

    // bing_dict / cambridge 已下架（Commit 1）——断言两源彻底移除：
    // build_provider 返 Err、registry 不含其 id、回退到新默认 google_free。

    #[test]
    fn build_provider_rejects_delisted_bing_dict_and_cambridge() {
        let bing_err = build_provider("bing_dict", &[]).err();
        assert_eq!(
            bing_err.as_deref(),
            Some("未知翻译 provider：bing_dict"),
            "bing_dict 已下架，build_provider 应返回未知源 Err"
        );
        let cambridge_err = build_provider("cambridge", &[]).err();
        assert_eq!(
            cambridge_err.as_deref(),
            Some("未知翻译 provider：cambridge"),
            "cambridge 已下架，build_provider 应返回未知源 Err"
        );
    }

    #[test]
    fn registry_excludes_delisted_bing_dict_and_cambridge() {
        let reg = registry();
        assert!(
            reg.iter().all(|c| c.id != "bing_dict"),
            "注册表不应再含已下架的 bing_dict"
        );
        assert!(
            reg.iter().all(|c| c.id != "cambridge"),
            "注册表不应再含已下架的 cambridge"
        );
    }
}
