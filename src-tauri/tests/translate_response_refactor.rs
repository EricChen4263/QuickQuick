//! TranslateResponse 枚举重构集成测试（TV4-F1-A01）
//!
//! 覆盖三冻结验收测试：
//! - translate_response_plain_variant_roundtrip：Plain 变体 serde 往返 + kind 判别
//! - dict_entry_serializes_with_type_tag：Dict 变体序列化带 kind="dict" + DictEntry 结构化字段
//! - existing_providers_return_plain_no_regression：既有源默认 translate 流程仍产出 Plain（不回归）
use quickquick_lib::translate::{
    DictEntry, HttpExecutor, Lang, PosDefinition, ProviderCapability, ProviderHttpRequest,
    TranslateError, TranslateProvider, TranslateRequest, TranslateResponse,
};

/// Plain 变体可 serde 往返，序列化带 kind="plain" 类型判别。
#[test]
fn translate_response_plain_variant_roundtrip() {
    let resp = TranslateResponse::plain("glacier");

    let json = serde_json::to_value(&resp).expect("Plain 应可序列化");
    assert_eq!(
        json["kind"], "plain",
        "Plain 序列化应带 kind=\"plain\" 判别标签，实际：{json}"
    );
    assert_eq!(
        json["translated"], "glacier",
        "Plain 应携带 translated 字段，实际：{json}"
    );

    let back: TranslateResponse =
        serde_json::from_value(json).expect("带 kind=plain 的 JSON 应可反序列化回 Plain");
    assert_eq!(
        back,
        TranslateResponse::plain("glacier"),
        "Plain 变体 serde 往返应保值"
    );
}

/// Dict 变体序列化带 kind="dict" 类型判别，DictEntry 携带音标/按词性分组释义/例句/发音/变形。
#[test]
fn dict_entry_serializes_with_type_tag() {
    let entry = DictEntry {
        phonetic: Some("ˈɡleɪʃər".to_string()),
        definitions: vec![PosDefinition {
            pos: Some("noun".to_string()),
            meanings: vec!["冰川".to_string(), "冰河".to_string()],
        }],
        examples: vec!["The glacier is melting.".to_string()],
        audio: Some("https://audio.example/glacier.mp3".to_string()),
        inflections: vec!["glaciers".to_string()],
    };
    let resp = TranslateResponse::Dict {
        entry: entry.clone(),
    };

    let json = serde_json::to_value(&resp).expect("Dict 应可序列化");
    assert_eq!(
        json["kind"], "dict",
        "Dict 序列化应带 kind=\"dict\" 判别标签，实际：{json}"
    );
    assert_eq!(
        json["entry"]["phonetic"], "ˈɡleɪʃər",
        "entry 应携带音标 phonetic，实际：{json}"
    );
    assert_eq!(
        json["entry"]["definitions"][0]["pos"], "noun",
        "释义应按词性分组（pos），实际：{json}"
    );
    assert_eq!(
        json["entry"]["definitions"][0]["meanings"][0], "冰川",
        "词性下应有 meanings 列表，实际：{json}"
    );
    assert_eq!(
        json["entry"]["examples"][0], "The glacier is melting.",
        "entry 应携带例句 examples，实际：{json}"
    );
    assert_eq!(
        json["entry"]["audio"], "https://audio.example/glacier.mp3",
        "entry 应携带发音 audio URL，实际：{json}"
    );
    assert_eq!(
        json["entry"]["inflections"][0], "glaciers",
        "entry 应携带变形 inflections，实际：{json}"
    );

    let back: TranslateResponse =
        serde_json::from_value(json).expect("带 kind=dict 的 JSON 应可反序列化回 Dict");
    assert_eq!(
        back,
        TranslateResponse::Dict { entry },
        "Dict 变体 serde 往返应保值"
    );
}

/// 既有源经默认 translate 流程产出 Plain 变体（不回归探测）。
///
/// 用仅返回 Plain 的最小 provider 模拟既有单步源，断言默认 translate
/// 流程产物仍是 Plain 且译文可取出，不被枚举重构破坏。
#[test]
fn existing_providers_return_plain_no_regression() {
    struct PlainOnlyProvider;
    impl TranslateProvider for PlainOnlyProvider {
        fn capability(&self) -> ProviderCapability {
            ProviderCapability {
                id: "plain_only_test",
                name: "Plain Only Test",
                needs_key: false,
                is_unofficial: true,
            }
        }
        fn build_request(&self, _req: &TranslateRequest) -> ProviderHttpRequest {
            ProviderHttpRequest {
                method: "GET",
                url: "https://example.invalid/t".to_string(),
                body: None,
                headers: Vec::new(),
            }
        }
        fn parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError> {
            Ok(TranslateResponse::plain(raw))
        }
    }

    struct StubExecutor;
    impl HttpExecutor for StubExecutor {
        fn execute(&self, _req: &ProviderHttpRequest) -> Result<String, TranslateError> {
            Ok("glacier".to_string())
        }
    }

    let provider = PlainOnlyProvider;
    let req = TranslateRequest {
        text: "冰川".to_string(),
        source_lang: Lang::new("zh"),
        target_lang: Lang::new("en"),
    };

    let resp = provider
        .translate(&req, &StubExecutor)
        .expect("既有单步源默认流程应成功");

    match resp {
        TranslateResponse::Plain { translated } => {
            assert_eq!(translated, "glacier", "既有源译文应原样保留在 Plain 变体");
        }
        TranslateResponse::Dict { .. } => {
            panic!("既有机翻/LLM 源应返回 Plain，不应返回 Dict")
        }
    }
}
