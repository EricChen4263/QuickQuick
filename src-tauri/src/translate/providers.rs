//! 编译期静态注册表与各 provider 的完整实现。
//!
//! 本文件实现薄 provider 三件职责（capability / build_request / parse_response）；
//! HTTP 执行、重试、超时、凭据读取等横切关注点由核心框架层统一处理。

use super::{
    lang::map_lang_for_provider, HttpExecutor, ProviderCapability, ProviderHttpRequest,
    TranslateError, TranslateProvider, TranslateRequest, TranslateResponse,
};

/// 按 provider_id 与凭据切片动态构造对应的 `TranslateProvider`。
///
/// `credentials` 为 `(field_key, value)` 键值对切片，与 `load_credentials` 返回类型一致。
/// 字段名必须与 `credential_schema` 声明的 key 逐字对齐：
/// - `lingva`、`google_free`、`yandex`、`transmart`：无凭据（免 key）
/// - `baidu`：`app_id`（必填）、`secret_key`（必填）
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
        "baidu" => {
            let app_id = find("app_id")
                .ok_or_else(|| "baidu 未配置 AppID，请前往设置填入 API Key".to_string())?;
            let secret_key = find("secret_key")
                .ok_or_else(|| "baidu 未配置 SecretKey，请前往设置填入 API Key".to_string())?;
            Ok(Box::new(BaiduProvider::new(app_id, secret_key)))
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
        BaiduProvider::new("", "").capability(),
        DeepLFreeProvider::new("").capability(),
        GoogleProvider::new("").capability(),
    ]
}

// Lingva

/// Lingva provider（免 key 默认源）。
///
/// Lingva 是开源 Google 翻译前端，提供无认证的纯 GET HTTP 接口。
/// 端点（按公开互操作协议事实独立实现，不参考任何第三方源码）：
/// `GET https://lingva.pot-app.com/api/v1/{source}/{target}/{url-encoded text}`
/// 响应体形如 `{"translation":"..."}`，译文取 `translation` 字段。
/// 选用 pot-app 托管实例：稳定优先、译质等同 Google 引擎；
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
            "https://lingva.pot-app.com/api/v1/{}/{}/{}",
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

        Ok(TranslateResponse { translated })
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

        Ok(TranslateResponse { translated })
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

        Ok(TranslateResponse { translated })
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

        Ok(TranslateResponse { translated })
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

        Ok(TranslateResponse { translated })
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

#[cfg(test)]
mod tests {
    use super::super::Lang;
    use super::*;

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
                .starts_with("https://lingva.pot-app.com/api/v1/"),
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
        assert_eq!(ok.translated, "glacier");

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
        assert_eq!(single.translated, "glacier");

        // 多分句：拼接各分句 result[0][i][0]（实测 Google 不在分句间补空格，原样拼接）。
        let multi = provider
            .parse_response(r#"[[["Hello","你好",null,null,2],["World","世界",null,null,2]],null,"zh-CN"]"#)
            .expect("多分句应解析成功");
        assert_eq!(multi.translated, "HelloWorld");

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
        assert_eq!(ok.translated, "冰川");

        // 多元素 text 数组拼接（Yandex 偶将长文本分段返回）。
        let multi = provider
            .parse_response(r#"{"code":200,"text":["你好世界。","你好吗?"]}"#)
            .expect("多元素 text 数组应解析成功");
        assert_eq!(multi.translated, "你好世界。你好吗?");

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
        assert_eq!(ok.translated, "冰川");

        // 多段拼接（实测多 text_list 逐项对应）。
        let multi = provider
            .parse_response(
                r#"{"header":{"ret_code":"succ"},"auto_translation":["Glaciers are melting.","Hello World"]}"#,
            )
            .expect("多段 auto_translation 应解析成功");
        assert_eq!(multi.translated, "Glaciers are melting.Hello World");

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
        assert_eq!(resp.translated, "冰川");

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
        assert_eq!(ok.translated, "冰川");

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
}
