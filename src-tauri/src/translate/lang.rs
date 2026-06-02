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
pub fn resolve_direction(text: &str, configured_target: Option<Lang>) -> (Lang, Lang) {
    let source = detect_lang(text);
    // 复用 source 的检测结果决定默认方向，避免二次扫描文本
    let default_target = if source.as_str() == "zh" {
        Lang::new("en")
    } else {
        Lang::new("zh")
    };
    let target = configured_target.unwrap_or(default_target);
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
/// - `configured_target` 有值 → 直接用
/// - 否则按源语走默认方向：zh → en，其余 → zh
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
    let target = configured_target.unwrap_or(default_target);
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
/// - `mymemory`：期望 "zh-CN"（MyMemory API 文档约定）
/// - `baidu`：期望 "zh"（百度翻译 API 文档约定）
/// - `deepl_free`：期望大写 "ZH"（DeepL API v2 约定，语言代码全大写）
/// - `google`：期望 "zh-CN"（Google Cloud Translation API 约定）
///
/// 未知 provider 原样透传内部代码，不 panic。
pub fn map_lang_for_provider(provider_id: &str, lang: &Lang) -> String {
    let normalized = normalize_zh_variant(lang.as_str());
    match provider_id {
        "mymemory" => map_for_mymemory(normalized),
        "baidu" => map_for_baidu(normalized),
        "deepl_free" => map_for_deepl(normalized),
        "google" => map_for_google(normalized),
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

fn map_for_mymemory(normalized: &str) -> String {
    match normalized {
        "zh" => "zh-CN".to_string(),
        "zh-TW" => "zh-TW".to_string(),
        "en" => "en".to_string(),
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
        let (source, _target) =
            resolve_direction_with_source("你好世界", Some("ja"), None);
        // Assert: source 应为 ja，而非 detect_lang 返回的 zh
        assert_eq!(source.as_str(), "ja");
    }

    #[test]
    fn resolve_direction_with_source_auto_falls_back_to_detection() {
        // Arrange: source="auto" 不是有效显式源语
        // Act: 应回退 detect_lang，中文文本检测为 zh
        let (source, _target) =
            resolve_direction_with_source("你好世界", Some("auto"), None);
        // Assert: 回退检测，中文 → zh
        assert_eq!(source.as_str(), "zh");
    }

    #[test]
    fn resolve_direction_with_source_none_falls_back_to_detection() {
        // Arrange: source=None，走检测路径
        let (source, _target) =
            resolve_direction_with_source("hello world", None, None);
        // Assert: 回退检测，非中文 → en
        assert_eq!(source.as_str(), "en");
    }

    #[test]
    fn resolve_direction_with_source_both_source_and_target_given() {
        // Arrange: 显式给定 source="ja" 和 target="ko"
        let (source, target) = resolve_direction_with_source(
            "hello",
            Some("ja"),
            Some(Lang::new("ko")),
        );
        // Assert: source 和 target 均按显式值生效
        assert_eq!(source.as_str(), "ja");
        assert_eq!(target.as_str(), "ko");
    }

    #[test]
    fn resolve_direction_with_source_explicit_source_no_target_uses_default_direction() {
        // Arrange: 显式给定 source="ja"，target=None
        // Act: 应按默认方向逻辑决定 target（非中文 → zh）
        let (_source, target) =
            resolve_direction_with_source("dummy", Some("ja"), None);
        // Assert: 默认方向 ja（非中文）→ zh
        assert_eq!(target.as_str(), "zh");
    }

    // 多语言直通 sanity 测试

    #[test]
    fn map_lang_for_provider_mymemory_passes_through_non_zh_langs() {
        // 验证 mymemory provider 对 ja/ko/fr/de/es/ru 原样透传
        // 这保证这些语言的 langpair 能拼出正确的 API 参数
        for lang_code in &["ja", "ko", "fr", "de", "es", "ru"] {
            let lang = Lang::new(*lang_code);
            let mapped = map_lang_for_provider("mymemory", &lang);
            assert_eq!(
                mapped,
                *lang_code,
                "mymemory 应原样透传语言码 {lang_code}"
            );
        }
    }
}
