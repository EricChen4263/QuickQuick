//! 语言归一模块（V2-F1-S02）
//!
//! 职责：
//! 1. 本地检测源语言（Unicode 脚本判断，区分中文与非中文）
//! 2. 智能双向定方向（§4.3）：中文→英文，非中文→中文（可被 configured_target 覆盖）
//! 3. 内部 BCP-47 规范化
//! 4. 每 provider 映射表：把内部 BCP-47 抹平成各 provider 期望的代码

use super::Lang;

/// CJK 统一表意文字的 Unicode 区间列表。
///
/// 按 Unicode 15.1 标准，以下区间涵盖核心中文汉字：
/// - U+4E00–U+9FFF：CJK 统一表意文字（基本区）
/// - U+3400–U+4DBF：CJK 统一表意文字扩展 A
/// - U+20000–U+2A6DF：CJK 统一表意文字扩展 B（超出 BMP，用 u32 判断）
/// - U+F900–U+FAFF：CJK 兼容表意文字
///
/// 注意：日文假名（U+3040–U+30FF）和韩文（U+AC00–U+D7A3）不在此区间，
/// 避免误判纯日/韩文本为"中文"。
const CJK_RANGES: &[(u32, u32)] = &[
    (0x4E00, 0x9FFF),   // CJK 统一表意文字基本区
    (0x3400, 0x4DBF),   // CJK 扩展 A
    (0x20000, 0x2A6DF), // CJK 扩展 B
    (0xF900, 0xFAFF),   // CJK 兼容表意文字
];

/// 判断字符是否落在 CJK 表意文字区间内。
fn is_cjk_char(c: char) -> bool {
    let cp = c as u32;
    CJK_RANGES
        .iter()
        .any(|&(start, end)| cp >= start && cp <= end)
}

/// 判断文本是否包含中文（CJK 统一表意文字）字符。
///
/// 只要文本中存在至少一个 CJK 表意文字，即判定为"含中文"。
/// 此策略适合混合文本（如"hello 你好"），符合§4.3 智能双向设计意图。
pub fn detect_is_chinese(text: &str) -> bool {
    text.chars().any(is_cjk_char)
}

/// 本地检测文本的语言，返回内部 BCP-47 Lang。
///
/// 当前实现：区分中文（zh）与非中文（en/und）。
/// - 含 CJK 字符 → "zh"
/// - 否则 → "en"（默认，适合大多数非中文翻译场景）
pub fn detect_lang(text: &str) -> Lang {
    if detect_is_chinese(text) {
        Lang::new("zh")
    } else {
        Lang::new("en")
    }
}

/// 智能双向定方向（§4.3）。
///
/// 检测源语言，根据源语言决定默认目标方向：
/// - 源为中文 → 目标英文（zh → en）
/// - 源为非中文 → 目标中文（en → zh）
///
/// 若 `configured_target` 有值，则用它覆盖上述默认目标。
/// 守卫：若 configured_target 与检测到的源语相同，则回退 default_target，
/// 避免源==目标的退化对（provider 会拒绝，如 MyMemory 返回 403）。
pub fn resolve_direction(text: &str, configured_target: Option<Lang>) -> (Lang, Lang) {
    let source = detect_lang(text);
    // 复用 source 的检测结果决定默认方向，避免二次扫描文本
    let default_target = if source.as_str() == "zh" {
        Lang::new("en")
    } else {
        Lang::new("zh")
    };
    // 守卫：configured_target 只在与 source 不同时采用，否则回退 default_target。
    // 源==目标无法翻译（provider 报需两种不同语言，已 curl 证实 MyMemory 403）。
    let target = match configured_target {
        Some(t) if t.as_str() != source.as_str() => t,
        _ => default_target,
    };
    (source, target)
}

/// "auto" 占位符：前端传此值代表"不指定源语，走检测"。
const AUTO_SOURCE: &str = "auto";

/// 智能定方向，支持显式源语覆盖。
///
/// 决策规则：
/// - `configured_source` 为非空、非 "auto" 的具体语言码 → 直接用作源语，跳过检测
/// - 否则（None / 空串 / "auto"）→ 调 `detect_lang` 自动检测源语
///
/// 目标语规则：
/// - `configured_target` 有值且与源语不同 → 直接用
/// - `configured_target` 与源语相同（退化对）→ 回退 default_target，
///   避免 provider 收到相同源/目标（如 MyMemory 返回 403 已 curl 证实）
/// - `configured_target` 为 None → 按源语走默认方向：zh → en，其余 → zh
pub fn resolve_direction_with_source(
    text: &str,
    configured_source: Option<&str>,
    configured_target: Option<Lang>,
) -> (Lang, Lang) {
    let source = if is_explicit_source(configured_source) {
        // 调用方已明确指定源语，无需扫描文本
        Lang::new(configured_source.unwrap().trim())
    } else {
        detect_lang(text)
    };

    let default_target = if source.as_str() == "zh" {
        Lang::new("en")
    } else {
        Lang::new("zh")
    };
    // 守卫：configured_target 只在与 source 不同时采用，否则回退 default_target。
    // 源==目标无法翻译（provider 报需两种不同语言，已 curl 证实 MyMemory 403）。
    let target = match configured_target {
        Some(t) if t.as_str() != source.as_str() => t,
        _ => default_target,
    };
    (source, target)
}

/// 判定 configured_source 是否为有效的显式源语。
///
/// 满足以下全部条件才算有效：
/// 1. 不是 None
/// 2. trim 后非空串
/// 3. 不等于 AUTO_SOURCE（"auto"）
fn is_explicit_source(configured_source: Option<&str>) -> bool {
    match configured_source {
        None => false,
        Some(s) => {
            let trimmed = s.trim();
            !trimmed.is_empty() && trimmed != AUTO_SOURCE
        }
    }
}

/// 把内部 BCP-47 语言代码映射为指定 provider 期望的格式。
///
/// 各 provider 的中文代码差异（zh 变体归一）：
/// - `lingva`：期望 "zh"（实测 Lingva 协议直传 auto/zh/en，其余沿用 Google 式码）
/// - `baidu`：期望 "zh"（百度翻译 API 文档约定）
/// - `deepl_free`：期望大写 "ZH"（DeepL API v2 约定，语言代码全大写）
/// - `google`：期望 "zh-CN"（Google Cloud Translation API 约定）
///
/// 未知 provider 原样透传内部代码，不 panic。
pub fn map_lang_for_provider(provider_id: &str, lang: &Lang) -> String {
    let normalized = normalize_zh_variant(lang.as_str());
    match provider_id {
        "lingva" => map_for_lingva(normalized),
        "baidu" => map_for_baidu(normalized),
        "deepl_free" => map_for_deepl(normalized),
        "google" => map_for_google(normalized),
        "google_free" => map_for_google_free(normalized),
        "yandex" => map_for_yandex(normalized),
        "transmart" => map_for_transmart(normalized),
        "bing" => map_for_bing(normalized),
        "baidu_field" => map_for_baidu_field(normalized),
        "youdao" => map_for_youdao(normalized),
        "caiyun" => map_for_caiyun(normalized),
        "niutrans" => map_for_niutrans(normalized),
        "tencent" => map_for_tencent(normalized),
        "alibaba" => map_for_alibaba(normalized),
        "volcengine" => map_for_volcengine(normalized),
        _ => lang.as_str().to_string(),
    }
}

/// 把 zh 系变体（zh、zh-CN、zh-Hans、zh-TW、zh-Hant 等）归一为规范 tag。
///
/// 返回 "zh"（规范内部表示）或原始值（非 zh 变体不变）。
fn normalize_zh_variant(tag: &str) -> &str {
    match tag {
        "zh" | "zh-CN" | "zh-Hans" | "zh-SG" => "zh",
        "zh-TW" | "zh-Hant" | "zh-HK" => "zh-TW",
        other => other,
    }
}

/// Lingva 语言码映射。
///
/// 按实测协议（`/zh/en/冰川`→glacier、`/en/zh/...`→你好世界）：auto/zh/en 直传；
/// zh-TW 映射为 Lingva 的繁中码 zh_HANT；其余语言沿用 Google 式直传码。
fn map_for_lingva(normalized: &str) -> String {
    match normalized {
        "zh" => "zh".to_string(),
        "zh-TW" => "zh_HANT".to_string(),
        "en" => "en".to_string(),
        "auto" => "auto".to_string(),
        other => other.to_string(),
    }
}

fn map_for_baidu(normalized: &str) -> String {
    match normalized {
        "zh" | "zh-TW" => "zh".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

fn map_for_deepl(normalized: &str) -> String {
    match normalized {
        "zh" => "ZH".to_string(),
        "en" => "EN".to_string(),
        other => other.to_uppercase(),
    }
}

fn map_for_google(normalized: &str) -> String {
    match normalized {
        "zh" => "zh-CN".to_string(),
        "zh-TW" => "zh-TW".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

/// Google 免费接口（translate_a/single 公开接口协议）语言码映射。
///
/// 该接口与官方 Cloud Translation 用同一套 Google 式语言码（auto/zh-CN/zh-TW/en…），
/// 故映射规则与 `map_for_google` 一致；独立成函数以便两源各自演进、互不影响
/// （免费接口若未来需特殊码不会污染官方源）。zh 归一为简中 zh-CN，zh-TW 保留繁中，
/// auto 与其余语言原样透传作源语自动检测。
fn map_for_google_free(normalized: &str) -> String {
    match normalized {
        "zh" => "zh-CN".to_string(),
        "zh-TW" => "zh-TW".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

/// Yandex 免费接口（伪装 Android 客户端公开协议）语言码映射。
///
/// 实测协议用单参 `lang=src-tgt`（连字符对，如 `en-zh`）。Yandex 的中文**不分简繁**，
/// 简中/繁中变体一律归为 `zh`；其余语言原样透传作 lang 对的一端。
/// 注释来源：Yandex translate v1 tr.json 公开互操作协议事实（非 pot 源码）。
fn map_for_yandex(normalized: &str) -> String {
    match normalized {
        // normalize_zh_variant 已把 zh-CN/zh-Hans 归为 "zh"、繁中归为 "zh-TW"；
        // 两者在 Yandex 都用 "zh"（不分简繁）。
        "zh" | "zh-TW" => "zh".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

/// Transmart（腾讯交互翻译·匿名接口公开协议）语言码映射。
///
/// 实测协议用 `source.lang`/`target.lang`，**区分简繁**：简中 `zh`、繁中 `zh-TW`；
/// 其余语言原样透传。注释来源：transmart.qq.com/api/imt 公开互操作协议事实（非 pot 源码）。
fn map_for_transmart(normalized: &str) -> String {
    match normalized {
        "zh" => "zh".to_string(),
        "zh-TW" => "zh-TW".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

/// Bing edge 翻译接口（公开互操作协议事实，非 pot 源码）语言码映射。
///
/// 实测 curl（artifacts/bing-translate-sample.json）：Bing **区分简繁**，
/// 简中用 `zh-Hans`、繁中用 `zh-Hant`；其余语言原样透传（auto 作源语自动检测）。
/// normalize_zh_variant 已把 zh-CN/zh-Hans 归为 "zh"、zh-TW/zh-Hant 归为 "zh-TW"。
fn map_for_bing(normalized: &str) -> String {
    match normalized {
        "zh" => "zh-Hans".to_string(),
        "zh-TW" => "zh-Hant".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

/// 百度专业（fieldtranslate）语言码映射。
///
/// 按百度翻译开放平台 API 文档「语种列表」：简中 `zh`、繁中 `cht`、日语 `jp`、英语 `en`。
/// 与基础百度同属百度语种体系，但基础源历史实现把 zh-TW 也归 `zh`；
/// 此处按官方文档对繁中/日语用正确码（cht/jp），其余原样透传（auto 作自动检测）。
fn map_for_baidu_field(normalized: &str) -> String {
    match normalized {
        "zh" => "zh".to_string(),
        "zh-TW" => "cht".to_string(),
        "en" => "en".to_string(),
        "ja" => "jp".to_string(),
        other => other.to_string(),
    }
}

/// 有道智云翻译语言码映射。
///
/// 按有道智云「自然语言翻译服务 API 文档」支持语言表：简中 `zh-CHS`、繁中 `zh-CHT`、
/// 英语 `en`；其余语言（如 ja/ko/fr）原样透传，auto 作自动检测。
fn map_for_youdao(normalized: &str) -> String {
    match normalized {
        "zh" => "zh-CHS".to_string(),
        "zh-TW" => "zh-CHT".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

/// 彩云小译语言码映射。
///
/// 按彩云小译 API 文档：trans_type 形如 `en2zh`、`auto2zh`，
/// 仅支持中（`zh`）/英（`en`）/日（`ja`）三语；简繁中文均归一为 `zh`（彩云不分简繁），
/// auto 作源语自动检测。文档来源：https://docs.caiyunapp.com/blog/2018/09/03/translator/
fn map_for_caiyun(normalized: &str) -> String {
    match normalized {
        "zh" | "zh-TW" => "zh".to_string(),
        "en" => "en".to_string(),
        "ja" => "ja".to_string(),
        other => other.to_string(),
    }
}

/// 小牛翻译语言码映射。
///
/// 按小牛翻译 API 文档「语种列表」：简中 `zh`、繁中 `cht`、英语 `en`；
/// 其余语言原样透传，auto 作源语自动检测。
/// 文档来源：https://niutrans.com/documents/contents/trans_text
fn map_for_niutrans(normalized: &str) -> String {
    match normalized {
        "zh" => "zh".to_string(),
        "zh-TW" => "cht".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

/// 腾讯云机器翻译（TMT）语言码映射。
///
/// 按腾讯云 TMT API 文档「支持语言」：简中 `zh`、繁中 `zh-TW`、英语 `en`；
/// 其余语言原样透传，auto 作源语自动检测。
/// 文档来源：cloud.tencent.com/document/api/551/15619（非 pot 源码）。
fn map_for_tencent(normalized: &str) -> String {
    match normalized {
        "zh" => "zh".to_string(),
        "zh-TW" => "zh-TW".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

/// 阿里云机器翻译语言码映射。
///
/// 按阿里云机器翻译 API 文档「支持语言」：简中 `zh`、繁中 `zh-tw`（小写）、英语 `en`；
/// 其余语言原样透传，auto 作源语自动检测。
/// 文档来源：help.aliyun.com/document_detail/215387（非 pot 源码）。
fn map_for_alibaba(normalized: &str) -> String {
    match normalized {
        "zh" => "zh".to_string(),
        "zh-TW" => "zh-tw".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

/// 火山引擎机器翻译语言码映射。
///
/// 按火山引擎 TranslateText API 文档「支持语种」：简中 `zh`、繁中 `zh-Hant`、英语 `en`；
/// 其余语言原样透传，auto 作源语自动检测。
/// 文档来源：volcengine.com/docs/4640/65067（非 pot 源码）。
fn map_for_volcengine(normalized: &str) -> String {
    match normalized {
        "zh" => "zh".to_string(),
        "zh-TW" => "zh-Hant".to_string(),
        "en" => "en".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_zh_variant_maps_zh_cn_to_zh() {
        assert_eq!(normalize_zh_variant("zh-CN"), "zh");
        assert_eq!(normalize_zh_variant("zh-Hans"), "zh");
        assert_eq!(normalize_zh_variant("zh"), "zh");
    }

    #[test]
    fn normalize_zh_variant_leaves_non_zh_unchanged() {
        assert_eq!(normalize_zh_variant("en"), "en");
        assert_eq!(normalize_zh_variant("ja"), "ja");
        assert_eq!(normalize_zh_variant("fr"), "fr");
    }

    // Bing edge 接口区分简繁：简中 zh-Hans、繁中 zh-Hant（实测 curl 返回繁/简各异）。
    #[test]
    fn map_lang_for_bing_uses_zh_hans_and_hant() {
        assert_eq!(map_lang_for_provider("bing", &Lang::new("zh")), "zh-Hans");
        assert_eq!(map_lang_for_provider("bing", &Lang::new("zh-CN")), "zh-Hans");
        assert_eq!(
            map_lang_for_provider("bing", &Lang::new("zh-Hans")),
            "zh-Hans"
        );
        assert_eq!(
            map_lang_for_provider("bing", &Lang::new("zh-TW")),
            "zh-Hant"
        );
        assert_eq!(
            map_lang_for_provider("bing", &Lang::new("zh-Hant")),
            "zh-Hant"
        );
        assert_eq!(map_lang_for_provider("bing", &Lang::new("en")), "en");
        // auto 透传作源语自动检测（Bing from 留空亦可，但本实现走显式透传）。
        assert_eq!(map_lang_for_provider("bing", &Lang::new("auto")), "auto");
    }

    #[test]
    fn is_cjk_char_identifies_basic_cjk() {
        assert!(is_cjk_char('你'));
        assert!(is_cjk_char('好'));
        assert!(!is_cjk_char('A'));
        assert!(!is_cjk_char('あ'));
    }

    // resolve_direction_with_source 新增测试

    #[test]
    fn resolve_direction_with_source_explicit_source_overrides_detection() {
        // Arrange: 文本是中文，但显式指定 source="ja"
        // Act: 源语应为 ja，不应走 detect_lang
        let (source, _target) = resolve_direction_with_source("你好世界", Some("ja"), None);
        // Assert: source 应为 ja，而非 detect_lang 返回的 zh
        assert_eq!(source.as_str(), "ja");
    }

    #[test]
    fn resolve_direction_with_source_auto_falls_back_to_detection() {
        // Arrange: source="auto" 不是有效显式源语
        // Act: 应回退 detect_lang，中文文本检测为 zh
        let (source, _target) = resolve_direction_with_source("你好世界", Some("auto"), None);
        // Assert: 回退检测，中文 → zh
        assert_eq!(source.as_str(), "zh");
    }

    #[test]
    fn resolve_direction_with_source_none_falls_back_to_detection() {
        // Arrange: source=None，走检测路径
        let (source, _target) = resolve_direction_with_source("hello world", None, None);
        // Assert: 回退检测，非中文 → en
        assert_eq!(source.as_str(), "en");
    }

    #[test]
    fn resolve_direction_with_source_both_source_and_target_given() {
        // Arrange: 显式给定 source="ja" 和 target="ko"
        let (source, target) =
            resolve_direction_with_source("hello", Some("ja"), Some(Lang::new("ko")));
        // Assert: source 和 target 均按显式值生效
        assert_eq!(source.as_str(), "ja");
        assert_eq!(target.as_str(), "ko");
    }

    #[test]
    fn resolve_direction_with_source_explicit_source_no_target_uses_default_direction() {
        // Arrange: 显式给定 source="ja"，target=None
        // Act: 应按默认方向逻辑决定 target（非中文 → zh）
        let (_source, target) = resolve_direction_with_source("dummy", Some("ja"), None);
        // Assert: 默认方向 ja（非中文）→ zh
        assert_eq!(target.as_str(), "zh");
    }

    // 同语种退化对守卫测试（核心 bug 修复）

    #[test]
    fn resolve_direction_with_source_chinese_text_with_zh_target_falls_back_to_en() {
        // Arrange: 中文文本 + 前端默认传 target=zh，会产生 zh→zh 退化对
        // Act & Assert: 守卫应回退为 zh→en，provider 不会收到相同源/目标
        let (source, target) = resolve_direction_with_source("接受", None, Some(Lang::new("zh")));
        assert_eq!(source.as_str(), "zh");
        assert_eq!(target.as_str(), "en");
    }

    #[test]
    fn resolve_direction_with_source_auto_source_zh_target_falls_back_to_en() {
        // Arrange: source="auto"（前端明确传 auto）+ target=zh，中文文本
        // Act & Assert: auto 回退检测到 zh，守卫介入，目标回退为 en
        let (source, target) =
            resolve_direction_with_source("接受", Some("auto"), Some(Lang::new("zh")));
        assert_eq!(source.as_str(), "zh");
        assert_eq!(target.as_str(), "en");
    }

    #[test]
    fn resolve_direction_with_source_explicit_same_lang_pair_falls_back_to_default_target() {
        // Arrange: 显式指定 source="ja" + target="ja"，产生 ja→ja 退化对
        // Act & Assert: 守卫介入，target 回退为 default_target（ja 非 zh → zh）
        let (source, target) =
            resolve_direction_with_source("hello", Some("ja"), Some(Lang::new("ja")));
        assert_eq!(source.as_str(), "ja");
        assert_eq!(target.as_str(), "zh");
    }

    #[test]
    fn resolve_direction_with_source_english_text_with_zh_target_not_affected() {
        // Arrange: 英文文本 + target=zh，是合法的 en→zh 对，守卫不应介入
        let (source, target) = resolve_direction_with_source("hello", None, Some(Lang::new("zh")));
        assert_eq!(source.as_str(), "en");
        assert_eq!(target.as_str(), "zh");
    }

    #[test]
    fn resolve_direction_with_source_cross_language_pair_not_affected() {
        // Arrange: 跨语种且不同语言（ja→ko），守卫不应介入
        let (source, target) =
            resolve_direction_with_source("hello", Some("ja"), Some(Lang::new("ko")));
        assert_eq!(source.as_str(), "ja");
        assert_eq!(target.as_str(), "ko");
    }

    #[test]
    fn resolve_direction_same_lang_pair_falls_back_to_default_target() {
        // resolve_direction 变体：zh 文本 + target=zh 退化对，守卫介入回退 en
        let (source, target) = resolve_direction("接受", Some(Lang::new("zh")));
        assert_eq!(source.as_str(), "zh");
        assert_eq!(target.as_str(), "en");
    }

    // 多语言直通 sanity 测试

    #[test]
    fn map_lang_for_provider_lingva_passes_through_non_zh_langs() {
        // 验证 lingva provider 对 ja/ko/fr/de/es/ru 原样透传
        // 这保证这些语言能拼出正确的 Lingva 路径段
        for lang_code in &["ja", "ko", "fr", "de", "es", "ru"] {
            let lang = Lang::new(*lang_code);
            let mapped = map_lang_for_provider("lingva", &lang);
            assert_eq!(mapped, *lang_code, "lingva 应原样透传语言码 {lang_code}");
        }
    }

    #[test]
    fn map_lang_for_provider_lingva_maps_core_langs_per_protocol() {
        // 实测 Lingva 协议：auto/zh/en 直传；zh-CN 归一后为 zh
        assert_eq!(map_lang_for_provider("lingva", &Lang::new("zh")), "zh");
        assert_eq!(map_lang_for_provider("lingva", &Lang::new("zh-CN")), "zh");
        assert_eq!(map_lang_for_provider("lingva", &Lang::new("en")), "en");
        assert_eq!(map_lang_for_provider("lingva", &Lang::new("auto")), "auto");
        assert_eq!(
            map_lang_for_provider("lingva", &Lang::new("zh-TW")),
            "zh_HANT"
        );
    }

    #[test]
    fn map_lang_for_provider_yandex_zh_not_split_traditional() {
        // 实测 Yandex 协议用 lang=en-zh（连字符对）；其 zh 不分简繁，zh/zh-CN/zh-TW 一律 "zh"。
        assert_eq!(map_lang_for_provider("yandex", &Lang::new("zh")), "zh");
        assert_eq!(map_lang_for_provider("yandex", &Lang::new("zh-CN")), "zh");
        assert_eq!(map_lang_for_provider("yandex", &Lang::new("zh-TW")), "zh");
        assert_eq!(map_lang_for_provider("yandex", &Lang::new("en")), "en");
        // 其余语言原样透传，作 lang 对的一端。
        assert_eq!(map_lang_for_provider("yandex", &Lang::new("ja")), "ja");
    }

    #[test]
    fn map_lang_for_provider_transmart_distinguishes_traditional() {
        // 实测 Transmart 用 source.lang/target.lang；区分简繁：zh 简中、zh-TW 繁中。
        assert_eq!(map_lang_for_provider("transmart", &Lang::new("zh")), "zh");
        assert_eq!(map_lang_for_provider("transmart", &Lang::new("zh-CN")), "zh");
        assert_eq!(
            map_lang_for_provider("transmart", &Lang::new("zh-TW")),
            "zh-TW"
        );
        assert_eq!(map_lang_for_provider("transmart", &Lang::new("en")), "en");
        assert_eq!(map_lang_for_provider("transmart", &Lang::new("ja")), "ja");
    }

    // 百度专业沿用百度语言码（官方文档：与基础百度一致，zh/cht/en）。
    #[test]
    fn map_lang_for_provider_baidu_field_uses_baidu_codes() {
        assert_eq!(map_lang_for_provider("baidu_field", &Lang::new("zh")), "zh");
        assert_eq!(
            map_lang_for_provider("baidu_field", &Lang::new("zh-CN")),
            "zh"
        );
        assert_eq!(
            map_lang_for_provider("baidu_field", &Lang::new("zh-TW")),
            "cht"
        );
        assert_eq!(map_lang_for_provider("baidu_field", &Lang::new("en")), "en");
        assert_eq!(map_lang_for_provider("baidu_field", &Lang::new("ja")), "jp");
    }

    // 有道用 zh-CHS（简）/ zh-CHT（繁）语言码（有道智云 API 文档）。
    #[test]
    fn map_lang_for_provider_youdao_uses_zh_chs_and_cht() {
        assert_eq!(
            map_lang_for_provider("youdao", &Lang::new("zh")),
            "zh-CHS"
        );
        assert_eq!(
            map_lang_for_provider("youdao", &Lang::new("zh-CN")),
            "zh-CHS"
        );
        assert_eq!(
            map_lang_for_provider("youdao", &Lang::new("zh-TW")),
            "zh-CHT"
        );
        assert_eq!(map_lang_for_provider("youdao", &Lang::new("en")), "en");
        assert_eq!(map_lang_for_provider("youdao", &Lang::new("ja")), "ja");
    }

    // 彩云仅中英日：简繁中文均归 zh（不分简繁），en/ja 直传（彩云 API 文档）。
    #[test]
    fn map_lang_for_provider_caiyun_only_zh_en_ja() {
        assert_eq!(map_lang_for_provider("caiyun", &Lang::new("zh")), "zh");
        assert_eq!(map_lang_for_provider("caiyun", &Lang::new("zh-CN")), "zh");
        assert_eq!(map_lang_for_provider("caiyun", &Lang::new("zh-TW")), "zh");
        assert_eq!(map_lang_for_provider("caiyun", &Lang::new("en")), "en");
        assert_eq!(map_lang_for_provider("caiyun", &Lang::new("ja")), "ja");
        assert_eq!(map_lang_for_provider("caiyun", &Lang::new("auto")), "auto");
    }

    // 小牛：简中 zh、繁中 cht、en 直传，其余透传（小牛 API 文档语种列表）。
    #[test]
    fn map_lang_for_provider_niutrans_uses_zh_cht_codes() {
        assert_eq!(map_lang_for_provider("niutrans", &Lang::new("zh")), "zh");
        assert_eq!(map_lang_for_provider("niutrans", &Lang::new("zh-CN")), "zh");
        assert_eq!(
            map_lang_for_provider("niutrans", &Lang::new("zh-TW")),
            "cht"
        );
        assert_eq!(map_lang_for_provider("niutrans", &Lang::new("en")), "en");
        assert_eq!(map_lang_for_provider("niutrans", &Lang::new("ja")), "ja");
    }

    // 腾讯云 TMT：简中 zh、繁中 zh-TW、en 直传（腾讯云 TMT API 文档）。
    #[test]
    fn map_lang_for_provider_tencent_uses_zh_and_zh_tw() {
        assert_eq!(map_lang_for_provider("tencent", &Lang::new("zh")), "zh");
        assert_eq!(map_lang_for_provider("tencent", &Lang::new("zh-CN")), "zh");
        assert_eq!(
            map_lang_for_provider("tencent", &Lang::new("zh-TW")),
            "zh-TW"
        );
        assert_eq!(map_lang_for_provider("tencent", &Lang::new("en")), "en");
        assert_eq!(map_lang_for_provider("tencent", &Lang::new("ja")), "ja");
    }

    // 阿里云：简中 zh、繁中 zh-tw（小写）、en 直传（阿里云机器翻译 API 文档）。
    #[test]
    fn map_lang_for_provider_alibaba_uses_zh_and_zh_tw_lowercase() {
        assert_eq!(map_lang_for_provider("alibaba", &Lang::new("zh")), "zh");
        assert_eq!(map_lang_for_provider("alibaba", &Lang::new("zh-CN")), "zh");
        assert_eq!(
            map_lang_for_provider("alibaba", &Lang::new("zh-TW")),
            "zh-tw"
        );
        assert_eq!(map_lang_for_provider("alibaba", &Lang::new("en")), "en");
        assert_eq!(map_lang_for_provider("alibaba", &Lang::new("ja")), "ja");
    }

    #[test]
    fn map_lang_for_provider_google_free_uses_google_style_codes() {
        // Google 免费接口（translate_a/single）用 Google 式码：zh→zh-CN、zh-TW 繁中、en、auto/其余透传。
        assert_eq!(
            map_lang_for_provider("google_free", &Lang::new("zh")),
            "zh-CN"
        );
        assert_eq!(
            map_lang_for_provider("google_free", &Lang::new("zh-CN")),
            "zh-CN"
        );
        assert_eq!(
            map_lang_for_provider("google_free", &Lang::new("zh-TW")),
            "zh-TW"
        );
        assert_eq!(map_lang_for_provider("google_free", &Lang::new("en")), "en");
        assert_eq!(
            map_lang_for_provider("google_free", &Lang::new("auto")),
            "auto"
        );
        assert_eq!(map_lang_for_provider("google_free", &Lang::new("ja")), "ja");
    }
}
