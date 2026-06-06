---
id: TV2-F2-S01-review
type: review
level: 小功能
parent: TV2-F2
created: 2026-06-06T00:00:00Z
status: 通过
commit: 29bae0c
acceptance_ids: [TV2-F2-A01, TV2-F5-A01]
author: code-reviewer
---

# TV2-F2 审查报告：彩云小译（caiyun）+ 小牛翻译（niutrans）

## 审查范围

- `src-tauri/src/translate/providers.rs`（新增 `CaiyunProvider`、`NiutransProvider`、`map_caiyun_error`、`map_niutrans_error`；`build_provider`/`registry` 追加）
- `src-tauri/src/translate/credential.rs`（caiyun/niutrans schema）
- `src-tauri/src/translate/lang.rs`（`map_for_caiyun`/`map_for_niutrans` 及映射测试）
- `src-tauri/tests/translate.rs`（注册表计数 10→12）
- 照项目规范（设计文档 §〇/§二.2.2/§三.决策4）+ code-standards + 验收 TV2-F2-A01 / TV2-F5-A01 / TV2-A-SEC

---

## 审查维度

### 1. 协议正确性（TV2-F2-A01）

**CaiyunProvider**

- 端点：`POST https://api.interpreter.caiyunai.com/v1/translator`，与彩云小译官方 API 文档（https://docs.caiyunapp.com/blog/2018/09/03/translator/）及设计文档 §二.2.2 一致。
- 鉴权头：`x-authorization: token {token}`，格式与官方文档约定完全一致（`token ` 前缀后跟 token 值，非 `Bearer`）。
- 请求 body：`serde_json::json!` 构造，含 `source`（数组形态）、`trans_type`（`{src}2{tgt}` 拼接，如 `en2zh`/`auto2zh`）、`request_id`（uuid v4）、`detect: true`，与官方文档请求参数一致。
- parse 双形态兼容：`target` 主形态数组取首项，兼容单字符串形态；错误响应（无 `target` 有 `message`）→ `map_caiyun_error`，无 target 也无 message → `ParseError("missing target")`，三路均不 panic。

**NiutransProvider**

- 端点：`POST https://api.niutrans.com/NiuTransServer/translation`，与小牛翻译 API 文档（https://niutrans.com/documents/contents/trans_text）及设计文档 §二.2.2 一致。
- 鉴权：`apikey` 置于请求体（非请求头），与官方文档约定一致；由 `serde_json::json!` 自动转义，无手拼注入面。
- body 字段：`from`/`to`/`apikey`/`src_text`，与官方文档请求参数对应。
- parse：成功取 `tgt_text`；错误通过 `error_code`（`extract_number_or_string` 兼容字符串/数字形态）→ `map_niutrans_error`，不含 apikey 值，不 panic。

### 2. 安全审查（TV2-A-SEC）

- providers.rs 全文 grep `eprintln|println!|log::|tracing::|dbg!` → **0 匹配**，密钥（token/apikey）不入任何日志路径。
- `build_provider` 缺字段错误消息为硬编码字符串（`"caiyun 未配置 token，请前往设置填入 API Key"`），**不含凭据值**，安全约定满足。
- `serde_json::json!` 构造 body，文本和密钥自动 JSON 转义，防注入。
- `map_caiyun_error`/`map_niutrans_error` 接收服务端响应 msg（非本地私钥），不泄露本地 token/apikey。
- request_id 由 `uuid::Uuid::new_v4()` CSPRNG 生成。

### 3. 凭据 schema（TV2-F5-A01）

- `caiyun`：`token`（is_secret=true, required=true），唯一字段，标记正确。
- `niutrans`：`apikey`（is_secret=true, required=true），唯一字段，标记正确。
- `capability().needs_key=true`、`is_unofficial=false`，与官方 API 身份一致。

### 4. 许可合规（GPL 红线）

- 彩云注释标注官方文档 URL：`https://docs.caiyunapp.com/blog/2018/09/03/translator/`（非 pot 源码）。
- 小牛注释标注官方文档 URL：`https://niutrans.com/documents/contents/trans_text`（非 pot 源码）。
- 协议细节（端点/header/body 参数/响应字段）属功能性事实，独立实现，与设计文档 §〇 要求一致。

### 5. 语言码映射

- `map_for_caiyun`：zh/zh-TW 均归 `zh`（彩云不分简繁），en/ja 直传，其余透传（含 auto）。实际 zh-CN 经上层 `normalize_zh_variant` 已规范化为 `zh`，行为正确。
- `map_for_niutrans`：zh→`zh`，zh-TW→`cht`，en 直传，其余透传。
- 两映射均有对应测试，覆盖 zh/zh-CN/zh-TW/en/ja/auto 共 4–5 个组合。

### 6. 既有源不回归

- git diff 全量删除行为零，providers.rs/credential.rs/lang.rs 均纯新增，既有 10 源代码未被改动。
- `registry()` 新源追加在末尾，不改已有源顺序。
- tester 报告确认全量 209 passed（含既有 10 源）。

### 7. 工程质量

- 无 `unwrap()`/`expect()` 在生产路径：parse 走 `map_err`/`ok_or_else`，错误路径全部返回 `Err`。
- `CaiyunProvider::build_request` ≈ 27 行，`parse_response` ≈ 29 行；`NiutransProvider::build_request` ≈ 20 行，`parse_response` ≈ 17 行；`map_caiyun_error` ≈ 8 行，`map_niutrans_error` ≈ 11 行——全部在 50 行内，嵌套深度 ≤ 3。
- 无装饰性分隔注释（全文 grep `═══/───/━━━/=====` 为空）。
- clippy `-D warnings` exit 0（tester 报告确认）。
- 无遗留 TODO/FIXME（全三文件 grep 为空）。

### 8. 测试充分性（静态视角）

- `caiyun_build_and_parse`：端点/鉴权头/body 结构/target 数组形态/target 字符串形态/错误响应/非法 JSON 七路覆盖。
- `niutrans_build_and_parse`：端点/body 结构（from/to/apikey/src_text）/成功 tgt_text/错误响应/非法 JSON 五路覆盖。
- build_provider 缺字段/有字段：各 2 个独立测试，capability.id 验证一致。
- registry needs_key/is_unofficial：各 1 个独立测试。
- credential schema is_secret/required：`credential_schema_for_v2_keyed_sources` 覆盖两源全字段。
- 注册表计数：`static_registry_lists_twelve_providers` 断言 12。

---

## 发现问题汇总

审查未发现置信度 ≥ 80 的 Critical 问题。

以下为 Important 级别观察（置信度在 80 区间，建议改但不阻塞——tester 动态证伪已过门禁，生产代码本身无缺陷）：

**观察 A（Important · 置信度 80）**：`build_provider_caiyun_missing_token_returns_err` 和 `build_provider_niutrans_missing_apikey_returns_err` 两个测试均使用空凭据 `&[]`，仅断言 `is_err()`，未断言"错误消息不含凭据值"。对比 `build_provider_baidu_field_missing_required_fields_returns_err`（刻意传入含 `"sk_secret_must_not_leak"` 的凭据、断言错误消息不含该值），两者测试充分性不一致。验收标准 TV2-F5-A01 明确要求"不含字段值"——当前测试无法实际验证此安全属性（空凭据本身没有值可泄）。**生产代码本身无安全漏洞**（错误消息为硬编码字符串），但测试未能有效证伪该安全约定。建议补充：传含脏值凭据（如 token 存在但触发不同错误时）、或在现有空凭据测试中加断言 `!err.contains("caiyun")` 以外的安全属性验证。

**观察 B（Important · 置信度 80）**：`caiyun_build_and_parse` 对错误响应（`{"message":"token is invalid"}`）只断言 `is_err()`，未断言错误类型为 `Auth`（对比 `youdao_parse` 断言 `matches!(err, Err(TranslateError::Auth(_)))`，`baidu_field_parse` 同样有具体类型断言）。`map_caiyun_error` 对含 "token" 的 message 归一为 `Auth`，但测试未覆盖此归类行为，变异测试（如将 Auth 改为 ServerError）不会被现有测试捕获。`niutrans_build_and_parse` 同理（error_code 13001 应归 Auth，测试只断言 `is_err()`）。建议补充 `matches!(err, Err(TranslateError::Auth(_)))` 断言，与既有测试风格对齐。

以下为置信度 < 80 的观察（仅供参考，不阻塞）：

**观察 C（置信度 50）**：设计文档 §〇 要求"注释里标注官方 API 文档 URL"；两源均有 URL 标注（彩云/小牛 URL 在文档注释中），满足设计要求。但彩云 URL 指向一篇博客（`docs.caiyunapp.com/blog/2018/09/03/translator/`），不是 API 控制台正式文档页，若该博客 URL 失效会造成溯源断链。风险低，不构成本次增量缺陷。

---

## 结论

协议实现（鉴权头/端点/body/parse 双形态）与彩云/小牛官方文档约定一致，独立实现未复制 pot（GPL-3.0）代码；密钥安全路径（不入日志/不入错误消息）、凭据 schema（is_secret/needs_key/is_unofficial）、既有源不回归、函数规模/嵌套/注释规范均满足要求；tester 动态证伪（8/8 命中 + 变异 A/B/C/D 全红 + 边界安全 + debug/release 双绿 + clippy 0）已通过门禁。观察 A/B 为测试充分性差距（非生产缺陷），不阻塞此小功能。

---

**WARNING**（无 Critical；观察 A/B 为 Important 级测试充分性差距，建议在后续迭代中补充断言，不阻塞本小功能验收）
