---
id: TV3-F2-S01
type: coding
parent: TV3-F2
commit: 9f92e25
acceptance_ids: [TV3-F2-A01]
---

# TV3-F2-S01 编码留痕：ChatGLM（智谱 JWT HS256）+ Gemini（URL 参 key）

## 范围

新增两个 LLM 对话翻译源：

- **ChatGLM（智谱）**：`POST https://open.bigmodel.cn/api/paas/v4/chat/completions`（OpenAI 兼容 chat/completions 形态），鉴权用手搓 JWT HS256。
- **Gemini（Google）**：`POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={apiKey}`，key 作 URL query 参（不进头），body 为 contents/parts 异构结构。

均 needs_key=true、is_unofficial=false，仅非流式。

## 改动文件

- `src-tauri/src/translate/providers.rs`
  - 新增纯函数 `base64url_no_pad`（base64url 无填充，复用既有 `base64 = "0.22"` 依赖的 `URL_SAFE_NO_PAD` 引擎）。
  - 新增纯函数 `chatglm_jwt(id, secret, exp_ms, timestamp_ms)`：手搓 JWT HS256，header `{"alg":"HS256","sign_type":"SIGN"}`、payload `{"api_key","exp","timestamp"}`、signature=HMAC-SHA256(secret) over `b64url(header).b64url(payload)`，三段 base64url 无填充以 `.` 连接。**exp/timestamp 作参数注入**使签名确定可锚定单测；复用既有 `hmac_sha256`。
  - 新增 `ChatGlmProvider`：build_request 复用 `render_prompt + build_chat_body`（与 OpenAI 同构 body），鉴权头由 `authorization()` 用当前时间签发 JWT（apiKey 拆 `{id}.{secret}`，TTL 1h）；parse 取 `choices[0].message.content`；`map_chatglm_error` 归一错误码。
  - 新增 `GeminiProvider`：build_request 用 `render_prompt` 得 messages 后经 `build_gemini_body` 转为 Gemini contents/parts（system→systemInstruction，user/assistant→contents[].parts[].text，assistant role 归一为 model）；key 经 `percent_encode` 作 URL query 参、绝不进头；parse 取 `candidates[0].content.parts[0].text`；`map_gemini_error` 归一 status/code。
  - `build_provider` match 增 `chatglm`（apiKey+model 必填）、`gemini`（apiKey+model 必填）分支。
  - `registry()` 增 `ChatGlmProvider` / `GeminiProvider` 两项 → 共 19 家。
  - 新增 7 个单测（4 冻结 + registry 命中 + sentinel 安全断言 + JWT 确定性锚定）。
- `src-tauri/src/translate/credential.rs`
  - `credential_schema` 增 `chatglm` / `gemini`：`apiKey`（is_secret=true 必填）、`model`（非密必填）、`base_url`/`prompt`（非密选填）。
- `src-tauri/tests/translate.rs`
  - `static_registry_lists_seventeen_providers` → `static_registry_lists_nineteen_providers`，断言值 17→19，doc 注释同步列入 chatglm/gemini。

## 关键实现决策

1. **JWT 独立复算锚定（反循环论证）**：`chatglm_jwt_hs256_deterministic` 不断言「等于本实现自己的输出」，而是用独立 Python（hmac+hashlib+base64）按官方 JWT HS256 手算出参照 token，断言逐字相等。
   - 固定输入：id=`test_id_12345`、secret=`test_secret_67890`、exp=`1717632000000`、timestamp=`1717631700000`。
   - 参照 token（Python 复算）：
     `eyJhbGciOiJIUzI1NiIsInNpZ25fdHlwZSI6IlNJR04ifQ.eyJhcGlfa2V5IjoidGVzdF9pZF8xMjM0NSIsImV4cCI6MTcxNzYzMjAwMDAwMCwidGltZXN0YW1wIjoxNzE3NjMxNzAwMDAwfQ.p-yF6cb9lFXduM5xA4qbBQkjTckbRU9tTFfO2IIIf4M`
   - exp/timestamp 做成入参才能锚定确定签名（请求路径才用 `current_unix_secs` 注入真实时间）。
2. **ChatGLM 复用 OpenAI 同构 body**：仅鉴权（JWT）与端点不同，body 形态（model/messages/stream=false）完全复用 `render_prompt + build_chat_body`，不重造。
3. **Gemini body 转换**：body 异构（contents/parts），用 `render_prompt` 拿到 chat messages 后经 `build_gemini_body` 转换；system 消息合并为 `systemInstruction`。
4. **安全**：apiKey/secret 经 schema is_secret=true 加密存储；JWT 派生与请求构造无任何 eprintln/println/log/dbg 打印密钥；错误消息（`map_chatglm_error`/`map_gemini_error`）只含错误码与服务端 msg、不回显 apiKey；Gemini key 仅在 URL query、不进日志/错误。sentinel 脏值（`SENTINEL_DEADBEEF`/`SENTINELID`）断言 `!contains` 证否泄露——ChatGLM 断言 Authorization 头为三段 JWT 且不明文含 secret/id。

## registry 断言

注册表 17 → **19**（+chatglm +gemini，均 needs_key=true 不进 keyless_ids）。`tests/translate.rs` 断言函数名同步改为 `static_registry_lists_nineteen_providers`、值改 19。

## 4 冻结测试转绿证据

证据原文：`artifacts/frozen-tests-green.log`

- `chatglm_jwt_hs256_deterministic ... ok`（test result: ok. 1 passed）
- `chatglm_build_request_and_parse ... ok`（test result: ok. 1 passed）
- `gemini_build_request_url_key_and_parse ... ok`（test result: ok. 1 passed）
- `gemini_parse_error_response ... ok`（test result: ok. 1 passed）

## 修复记录（tester 动态证伪打回 → 修复）

- **打回根因**：polish 阶段 `GeminiProvider::build_request` 的 `format!` URL 串只有 2 个 `{}` 占位符但传了 3 个参数（`percent_encode(&self.api_key)` 无对应占位符）→ `error: argument never used`（exit 101）阻断整 crate 编译，4 冻结测试无法执行；且 URL 漏带 `?key=`，真请求会被 Google 拒。上一轮自报 487 passed 与当前树不符——polish 编辑后只 cargo check 未全量 cargo test，漏掉。
- **修复①**：格式串改为 `"{}/v1beta/models/{}:generateContent?key={}"`（三占位符对应三参数），URL 正确携带 `?key=<api_key>` 且编译通过。
- **修复④（回归守卫）**：`gemini_build_request_url_key_and_parse` 原有完整 URL 全等断言已含 `?key=SENTINEL_DEADBEEF`（编译通过后即能抓此 bug）；额外补一条独立子串断言 `url.contains("?key=SENTINEL_DEADBEEF")`，确保「URL 漏带 key」bug 类今后被直接捕获。
- **修复后实跑（非 cargo check）**：全量 `cargo test` 全绿（lib 241 passed，各套件 test result: ok，0 failed，4 冻结测试命中）；`cargo test --release` 1 次全绿（241 passed）；`cargo clippy --all-targets -- -D warnings` exit 0。证据：`artifacts/frozen-tests-green.log`、`artifacts/clippy-clean.log`。

## 坑 / 备注

- `base64url_no_pad` 需在函数内 `use base64::Engine;`（trait 方法 `encode` 不在模块顶层 in-scope，与既有阿里云签名处同样手法）。
- ChatGLM/Gemini 未走 `map_lang_for_provider` 的专属分支（落入默认 `_` 直传 en/zh）——LLM 翻译由 Prompt 驱动，语言以 `$from/$to` 进 Prompt，无需 provider 专属语言码映射。
- 真网（需用户密钥）端到端译文归 manual_confirm（TV3-M01）。

## 验证

- 4 冻结测试 + 全量 `cargo test` 全绿（连跑 ≥3 次抗 flaky）。
- `cargo clippy --all-targets -- -D warnings` exit 0。
- 新增代码无 TODO/FIXME、无装饰性分隔注释。
