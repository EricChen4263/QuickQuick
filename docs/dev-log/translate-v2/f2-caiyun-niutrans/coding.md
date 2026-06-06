---
id: TV2-F2-S01-code
type: coding_record
level: 小功能
parent: TV2-F2
status: done
commit: PENDING
acceptance_ids: [TV2-F2-A01, TV2-F5-A01]
---

# TV2-F2 彩云 + 小牛翻译源 · 编码留痕

## 范围

新增两个需 key 官方机翻源：彩云小译（caiyun，token 鉴权）+ 小牛翻译（niutrans，body apikey）。
照各厂商**官方 API 文档**公开规范独立实现，**未复制/未参考 pot（GPL-3.0）源代码**。
不动既有任何源。

## TDD 红→绿证据

先写测试（RED：`CaiyunProvider`/`NiutransProvider` 未实现 → 编译错误 E0433），
再实现（GREEN）。原始逐测试名输出见同目录 `artifacts/`。

目标测试（acceptance ref 对齐）：

- `caiyun_build_and_parse` ... ok（端点/x-authorization 头/JSON body source+trans_type=en2zh+request_id/parse target[0]+字符串兼容/错误响应→TranslateError/非法 JSON→ParseError）
- `niutrans_build_and_parse` ... ok（端点/JSON body from/to/apikey/src_text/parse tgt_text/error_code→TranslateError/非法 JSON→ParseError）
- `credential_schema_for_v2_keyed_sources` ... ok（扩充覆盖 caiyun token / niutrans apikey 均 is_secret=true、required=true、各 1 字段）
- `build_provider_caiyun_missing_token_returns_err` / `build_provider_niutrans_missing_apikey_returns_err` ... ok（缺必填→Err，错误消息不含值）
- `build_provider_caiyun_with_token_succeeds` / `build_provider_niutrans_with_apikey_succeeds` ... ok
- `registry_contains_caiyun_keyed_official` / `registry_contains_niutrans_keyed_official` ... ok（needs_key=true, is_unofficial=false）
- `map_lang_for_provider_caiyun_only_zh_en_ja` / `map_lang_for_provider_niutrans_uses_zh_cht_codes` ... ok
- `static_registry_lists_twelve_providers`（tests/translate.rs）... ok（注册表计数 10→12）

cargo test（debug）：见 artifacts/cargo-test-debug.log（全绿）
cargo test --release：见 artifacts/cargo-test-release.log（全绿）
全量连跑 3×：见 artifacts/cargo-test-loop3.log（每轮全绿，无 flaky）
cargo clippy --all-targets -- -D warnings：exit 0，见 artifacts/clippy.log

## 改动文件清单

- `src-tauri/src/translate/providers.rs`：新增 `CaiyunProvider`（token，x-authorization 头，POST translator，body source 数组+trans_type+request_id，parse target[0]/字符串）、`NiutransProvider`（apikey 入 body，POST translation，parse tgt_text）；各配错误码归一函数 `map_caiyun_error`/`map_niutrans_error`；`build_provider` 加 caiyun/niutrans 分支（缺必填→明确错误不含值）；`registry()` 追加两源；测试模块追加两源 build_and_parse + 缺字段 + registry 单测。
- `src-tauri/src/translate/credential.rs`：`credential_schema` 加 caiyun（token 密）、niutrans（apikey 密）；扩充 `credential_schema_for_v2_keyed_sources` 测试覆盖两源。
- `src-tauri/src/translate/lang.rs`：`map_lang_for_provider` 加 caiyun/niutrans 分支；新增 `map_for_caiyun`（仅 zh/en/ja，简繁归 zh）、`map_for_niutrans`（zh/cht/en）；加两源映射单测。
- `src-tauri/tests/translate.rs`：注册表计数测试 `static_registry_lists_ten_providers`→`twelve`，断言 10→12。

## 关键实现决策

1. **彩云 trans_type 形态**：官方约定「源2目标」拼接（如 `en2zh`/`auto2zh`），用 `format!("{src}2{tgt}")` 由 lang 映射产出的码拼成。彩云仅支持中英日，`map_for_caiyun` 限定 zh/en/ja，简繁中文均归 `zh`（彩云不分简繁）。
2. **彩云 target 双形态兼容**：官方主形态返回 `target` 数组（与 source 逐项对应），取首项；同时兼容单字符串形态，避免接口形态差异导致 ParseError。错误响应用 `message` 字段（无 target）→ `map_caiyun_error`（token 相关归 Auth，余 ServerError）。
3. **小牛 apikey 入 body**：官方约定 apikey 置请求体（非请求头），随 from/to/src_text 一并 `serde_json::json!` 构造（自动转义文本，防 JSON 注入）。错误用 `error_code`（兼容字符串/数字，复用既有 `extract_number_or_string`）→ `map_niutrans_error`。
4. **复用既有约定**：沿用 `ProviderHttpRequest`/`TranslateProvider` trait、`map_lang_for_provider` 分发、`extract_number_or_string`、`credential_schema` is_secret 路由（DbCredStore）、build_provider `find` trim 空值判定，与 baidu_field/youdao 结构一致。

## 安全（TV2-A-SEC）

- token/apikey 仅进请求构造与 DbCredStore 加密库；providers.rs 无 `eprintln/println/log::/dbg!` 打印任何密钥。
- 缺字段错误消息只含字段名（"caiyun 未配置 token…"/"niutrans 未配置 apikey…"），不含值；build_provider 缺字段测试断言错误消息不泄露。
- 用 serde_json::json! 构造 body，文本/密钥自动转义，无手拼注入面。

## 许可红线确认

- 两源均按彩云小译 / 小牛翻译**官方 API 文档**公开规范独立实现，代码注释标官方文档 URL（非 pot）。
- 未打开/未复制/未近似改写 pot 任何源代码。

## 测试补强（code-reviewer 复审后，只改测试不动生产逻辑）

针对 code-reviewer 指出的 2 处测试充分性差距补强（生产码无缺陷）：

1. **缺字段测试坐实「错误不含密钥值」（对齐 TV2-F5-A01）**：
   `build_provider_caiyun_missing_token_returns_err` / `build_provider_niutrans_missing_apikey_returns_err`
   由原先空凭据 `&[]` 仅断言 `is_err()`，改为传一个**错拼 key 的脏密钥值**
   （caiyun `("toke","tok_secret_must_not_leak")`、niutrans `("apike","key_secret_must_not_leak")`）——
   token/apikey 仍缺失走缺字段路径，新增 `assert!(!err.contains(脏值))`，坐实错误消息不泄露字段值。
   写法对齐同文件 `build_provider_baidu_field_missing_required_fields_returns_err`。

2. **错误响应分支补类型断言（验 map_*_error 归类）**：
   - `caiyun_build_and_parse`：`{"message":"token is invalid"}` → 断言 `Err(Auth(_))`；
     新增 `{"message":"internal error"}` → 断言 `Err(ServerError(_))`（非鉴权类兜底）。
   - `niutrans_build_and_parse`：`error_code "13001"` → 断言 `Err(Auth(_))`；
     新增 `error_code "19001"`（余额不足）→ 断言 `Err(Quota(_))`。
   写法对齐 `youdao_parse`/`baidu_field_parse` 的 `matches!(err, Err(TranslateError::Auth(_)))`。

补强后：`rtk proxy cargo test --lib caiyun` 5 passed、`niutrans` 5 passed（逐行 ... ok）；
`cargo clippy --all-targets -- -D warnings` exit 0 无问题。

## 未决 / 人工确认点

- 真网端到端（需用户配置 token/apikey）走 TV2-F1234-M01 manual_confirm，本阶段以 build/parse objective 单测覆盖。
