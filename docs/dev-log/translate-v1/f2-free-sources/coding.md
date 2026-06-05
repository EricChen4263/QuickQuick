---
id: TV1-F2-S01-code
type: coding_record
level: 小功能
parent: TV1-F2
status: done
commit: PENDING
acceptance_ids: [TV1-F2-A01]
---

# TV1-F2-S01 编码留痕：新增 Google 免费翻译源（google_free）

## 做了什么

为翻译框架新增免 key 机翻源 `google_free`（「Google（免费）」），基于 Google
`translate_a/single` 公开接口协议独立实现，作为 Lingva 之外的并列免 key 备选/兜底
（设计文档 §二.2.1）。

实现要点：
- `GoogleFreeProvider`（无凭据结构体），实现薄 provider 三件职责：
  - `capability`：`{id:"google_free", name:"Google（免费）", needs_key:false}`。
  - `build_request`：`GET https://translate.googleapis.com/translate_a/single?client=gtx&sl={src}&tl={tgt}&dt=t&q={percent_encode(text)}`；
    src/tgt 经 `map_lang_for_provider("google_free", ..)` 映射为 Google 式码。
  - `parse_response`：解析顶层 JSON 数组，取 `result[0]`（分句译文对数组），
    拼接各 `result[0][i][0]`（每分句第 0 元素）为译文；`result[0]` 缺失/非数组/空、
    非法 JSON 一律返回 `TranslateError::ParseError`。
- `map_for_google_free`（lang.rs）：Google 式语言码映射（zh→zh-CN、zh-TW 保留繁中、
  en、auto 及其余透传）。独立成函数（虽与 `map_for_google` 当前逻辑一致），便于两源各自演进。
- registry()：在 lingva 之后插入 `google_free`。
- build_provider()：加 `"google_free" => Ok(Box::new(GoogleFreeProvider::new()))`。

## 关键决策

- **命名避坑**：既有 `google`（官方 API、needs_key=true）保持不动；新免 key 源用
  `google_free`（needs_key=false），避免 id 冲突。
- **拼接规则**：实测 Google 不在分句间补分隔符，故 `result[0][i][0]` 原样顺序拼接
  （多分句 `[["Hello",..],["World",..]]` → `"HelloWorld"`）。
- **错误处理**：解析全程 Result，空 `result[0]`、缺字段、格式错、非法 JSON 均映射
  `ParseError`，不 panic、不静默当成功。
- **许可红线**：未打开、未复制 GPL-3.0 的 pot 源代码；端点/参数/响应结构按公开互操作
  协议事实独立用原创 Rust 实现，注释标「Google translate_a/single 公开接口协议」来源，
  未引用 pot URL。
- **安全（TV1-A-SEC）**：needs_key=false，build_provider 不读凭据存储；provider 代码
  无 eprintln/log，不打印待译文本/译文。

## 改动文件

- `src-tauri/src/translate/providers.rs`：新增 `GoogleFreeProvider` 实现；registry()
  与 build_provider() 注册 `google_free`；新增 4 个测试。
- `src-tauri/src/translate/lang.rs`：新增 `map_for_google_free` 并接入
  `map_lang_for_provider`；新增 1 个测试。

## 自测（TDD 红→绿）

- RED：`GoogleFreeProvider` 未实现 → `cargo test` 编译失败
  `error[E0433]: cannot find type GoogleFreeProvider`（功能缺失，非语法/环境错）。
- GREEN：补实现后全绿。原始证据（`artifacts/cargo-test-green.log`）：
  - `test translate::lang::tests::map_lang_for_provider_google_free_uses_google_style_codes ... ok`
  - `test translate::providers::tests::google_free_build_request_url ... ok`
  - `test translate::providers::tests::google_free_parse_concatenates_segments ... ok`
  - `test translate::providers::tests::google_free_is_keyless_and_built_without_credentials ... ok`
  - `test translate::providers::tests::registry_contains_google_free_keyless ... ok`
  - `test result: ok. 70 passed; 0 failed`
- clippy：`cargo clippy --all-targets -- -D warnings` exit 0（见 artifacts/clippy.log）。

## 修复追记：同步 TV1-F1 旧集成测试（tester 动态证伪打回）

新增 google_free 使注册表从 4 家增至 5 家，`tests/translate.rs` 里 TV1-F1 写死的
2 个旧测试与实际不符、全量 `cargo test` 变红。修复（**仅改测试，未动 google_free 实现**）：

1. `static_registry_lists_four_providers` → 重命名 `static_registry_lists_five_providers`，
   断言 `providers.len()` 由 4 改为 5；注释说明后续每新增免 key 源此数随之增长。
2. `static_registry_keyed_providers_need_key`：原逻辑"非 lingva 即 needs_key=true"对免 key
   源不成立。改为维护免 key id 集合 `["lingva","google_free"]`，断言集合内 needs_key=false、
   集合外 needs_key=true；注释提示 F2/F3 新增免 key 源（bing/yandex/transmart/deepl_web…）
   时往此集合补 id。

验证：
- `cargo test` 连跑 3 次全绿（抗 flaky）：每次集成测试 binary `test result: ok. 168 passed;
  0 failed`，全部 binary 0 failed、exit=0。原始证据见
  `artifacts/cargo-test-full-run{1,2,3}.log`。
- `cargo clippy --all-targets -- -D warnings` exit 0、0 warning（artifacts/clippy-after-fix.log）。
- 改动范围核对：仅 `tests/translate.rs` 这 2 个测试函数；providers.rs 的 GoogleFreeProvider
  实现 9 处引用完整未动。

---

# TV1-F2-S02 编码留痕：新增 Yandex、Transmart 两个免 key 翻译源

> id: TV1-F2-S02-code · acceptance: TV1-F2-A01 · commit: PENDING

## 做了什么

为翻译框架新增两个免 key 机翻源 `yandex`（「Yandex（免费）」）、`transmart`
（「腾讯交互翻译（免费）」），均按 curl 实测的公开互操作协议事实独立用原创 Rust 实现，
作为 Lingva/Google 之外的并列免 key 备选（设计文档 §二.2.1）。

## curl 实测结论（ground truth）

先用 curl 实测两端点（合法获取协议事实，不碰 pot），原始证据 `artifacts/curl-probe.log`：

- **Yandex — 可用**（variant B）。关键：`source_lang`/`target_lang` 分参 + 带连字符 uuid
  会被拒（`{"code":405,"message":"Session is invalid"}` HTTP 403）；**必须**用 body 单参
  `lang=en-zh`（连字符对）+ query `id={去连字符 uuid}-0-0&srv=android`，才返回
  `{"code":200,"lang":"en-zh","text":["冰川"]}`（HTTP 200）。译文取 `text` 数组拼接。
  样例 `artifacts/yandex-sample.json`、错误样例 `artifacts/yandex-error-sample.json`。
- **Transmart — 可用**。匿名 JSON `POST https://transmart.qq.com/api/imt`，body 含
  `header.fn=auto_translation`、`source.{lang,text_list}`、`target.lang`，返回
  `{"header":{"ret_code":"succ"},"auto_translation":["","冰川",""],...}`（HTTP 200）。
  译文取 `auto_translation` 数组拼接（text_list 逐项对应）。样例 `artifacts/transmart-sample.json`。

两端点均实测可用，无需暂缓任何源。

## 各源决策

- **Yandex id 生成**：`id={uuid.simple}-0-0`。id 是客户端会话标识、非安全凭据，用随机
  v4 uuid 防请求被去重；`map_for_yandex` 把简繁中文都归为 `zh`（实测 Yandex zh 不分简繁）。
- **Yandex 错误**：body 内 `code != 200` 走 `map_yandex_error`（405/403→Auth、429→RateLimit…）；
  `text` 缺失/全空 → ParseError。
- **Transmart body 构造**：用 `serde_json::json!` 构造而非手拼字符串，避免文本内引号/反斜杠
  导致 JSON 注入或转义错误；`client_key` 用随机 uuid 拼前缀（浏览器指纹式标识、非凭据）。
- **Transmart 错误**：`header.ret_code != "succ"` → ServerError；`auto_translation` 缺失/全空
  → ParseError。
- **简繁差异**：Yandex `zh/zh-TW` 都用 `zh`；Transmart 区分 `zh`（简）/`zh-TW`（繁）。

## 改动文件

- `src-tauri/src/translate/lang.rs`：新增 `map_for_yandex`、`map_for_transmart`，接入
  `map_lang_for_provider` 的 yandex/transmart 分支；新增 2 个映射测试。
- `src-tauri/src/translate/providers.rs`：新增 `YandexProvider`、`TransmartProvider`
  （capability/build_request/parse_response）+ `map_yandex_error`；registry() 在 google_free
  之后追加两源、build_provider() 加对应分支；新增 8 个测试。
- `src-tauri/tests/translate.rs`：免 key 集合补 `yandex`/`transmart`；注册表计数 5→7
  （`static_registry_lists_five_providers`→`_seven_providers`）。
- `docs/dev-log/translate-v1/f2-free-sources/artifacts/`：curl 实测样例与证据。

## 许可红线

未打开、未复制 GPL-3.0 的 pot 源代码；端点/参数/响应结构均按 curl 自测的公开互操作协议
事实独立用原创 Rust 实现，注释标各自协议来源（Yandex tr.json / transmart.qq.com imt），
未引用 pot URL。既有 keyed 源与 lingva/google_free 实现完全未动。

## 自测（TDD 红→绿）

- RED（lang）：`map_lang_for_provider_yandex_*` 因 yandex 未在 match 走透传、zh-CN 未归一
  → 断言失败（功能缺失）。RED（providers）：`YandexProvider`/`TransmartProvider` 未定义
  → `error[E0433]: cannot find type`（功能缺失，非语法错）。
- GREEN：补实现后绿。原始证据（rtk proxy 绕代理）：
  - `test translate::providers::tests::yandex_build_request_endpoint_and_body ... ok`
  - `test translate::providers::tests::yandex_parse_extracts_text_array ... ok`
  - `test translate::providers::tests::transmart_build_request_endpoint_and_json_body ... ok`
  - `test translate::providers::tests::transmart_parse_concatenates_auto_translation ... ok`
  - `test translate::lang::tests::map_lang_for_provider_yandex_zh_not_split_traditional ... ok`
  - `test translate::lang::tests::map_lang_for_provider_transmart_distinguishes_traditional ... ok`
  - 另含各源 is_keyless / registry_contains 共 8 个 providers 测试 + 2 个 lang 测试，全 ok。
- 全量 `cargo test`：421 passed、0 failed、exit=0。
- 连跑 3×（抗 flaky）：run1/2/3 均 exit=0、0 FAILED，原始证据
  `artifacts/cargo-test-s02-run{1,2,3}.log`。
- `cargo clippy --all-targets -- -D warnings` exit=0、0 警告（`artifacts/clippy-s02.log`）。
  （首跑被 `redundant_pattern_matching` 拦 `matches!(err, Err(_))`，已改为先 `is_err()`
  再断言具体错误变体——断言更强、非恒真。）
- 逐行 ok 原始证据（rtk proxy 绕代理）：`artifacts/cargo-test-s02-targeted.log`，
  10 个新测试（8 providers + 2 lang）全 `... ok`，`test result: ok. 75 passed; 0 failed`。

## 提交前自检（对齐 code-standards）

- 装饰性横线分隔注释：`grep -E '──|═══|━━━|====='` 对改动 src/tests 无命中。
- 无 TODO/FIXME：grep 改动文件无残留。
- 安全（TV1-A-SEC）：两源 needs_key=false，build_provider 不读凭据存储；providers.rs
  无 eprintln/println/log，不打印待译文本/译文。Yandex `id`、Transmart `client_key`
  为非安全的客户端标识，用随机 uuid（非硬编码）。
- 断言验具体值/变体：parse 命中断言译文确切字符串；错误分支断言具体 `Auth`/`ServerError`/
  `ParseError` 变体，非恒真、非旁路（直调被测 `parse_response`/`build_request`）。

---

# TV1-F2-S03：DeepL 免 key（free web / jsonrpc）—— 实测不可用，暂缓

> 尝试新增 DeepL free web（非官方 jsonrpc 互操作接口）作为免 key 源。按红线先 curl 反复实测端点能否工作，再决定是否实现。

## curl 实测结论（ground truth）：不可用 → 暂缓

端点 `POST https://www2.deepl.com/jsonrpc`，method `LMT_handle_jobs`，含 id 自增 +
timestamp 混淆规则（timestamp 调整为能被「待译文本里 'i' 字符数 + 1」整除）。原始证据
`artifacts/deepl-web-probe.log`。

所有请求组合均返回 **HTTP 429 / `{"jsonrpc":"2.0","error":{"code":1042912,"message":"Too many requests"}}`**，无一次返回 `result.texts`：

- 基础请求（EN→ZH，timestamp 已按 'i' 计数整除规则混淆、id 自增）→ 429。
- 5 次间隔 3s、文本各异（规避去重）退避请求 → 全 429。
- Cookie 预热对照：先 GET `www.deepl.com/translator` 拿到 6 条 cookie 再带 cookie POST。
  `www.deepl.com/jsonrpc` 返回 301（openresty Moved Permanently，已非有效端点）；
  `www2.deepl.com/jsonrpc` 仍 429。
- 多 UA（Windows Chrome 119 / Linux Chrome 118）+ 8s 间隔对照 → 全 429。

**判定**：DeepL free web 端点对本机匿名访问已稳定限流/封禁。按本小功能红线「实测被限流/封禁/
反复不通则不硬造实现、标暂缓」，**TV1-F2-S03 标记为暂缓/不可用**，不新增 `deepl_free_web`
provider、不动 providers.rs/registry/build_provider/lang.rs/tests。设计文档已接受非官方源
可能失效；keyed 官方 `deepl_free` 源已存在，有需要的用户可填官方 auth_key 使用。

后续如需重启本源：可在不同网络/IP 重新 curl 实测，若能稳定返回 `result.texts` 再按本段
记录的协议事实（端点/method/混淆规则）独立实现。

## 顺带交付（无论第一步结果）：修过时注释

`src-tauri/src/translate/providers.rs:1` 模块头注释自 S01 起过时（仍写「4 家 provider」，
实际已 7 家）。改为不写死数字的描述「编译期静态注册表与各 provider 的完整实现」，避免再随
provider 增减过时。

## 自检

- 许可红线：全程未打开/复制 GPL-3.0 的 pot 源代码；DeepL jsonrpc 协议事实（端点/method/
  timestamp 混淆规则）按公开互操作事实独立用 curl 验证，证据为自测 curl 返回，未引用 pot。
- 未硬造实现：`grep -rn deepl_free_web src-tauri/` 无命中；既有 7 家源与框架完全未动。
- 编译/审查：`cargo check` exit=0、`cargo clippy --all-targets -- -D warnings` exit=0
  （`artifacts/clippy-s03.log`）。本次仅一行文档注释改动，无新增逻辑、无 TDD 实现（暂缓）。
