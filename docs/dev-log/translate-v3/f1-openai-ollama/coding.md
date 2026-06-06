---
id: TV3-F1-S01
type: coding
parent: TV3-F1
commit: 2704e42
acceptance_ids: [TV3-F1-A01]
---

# TV3-F1-S01 编码留痕：LLM chat-completion 抽象 + Prompt 模板引擎 + OpenAI + Ollama

## 范围

为翻译框架新增两个 LLM 对话翻译源（OpenAI、Ollama 本地），并内置可单测的 Prompt 模板引擎。
独立重写，依据 OpenAI / Ollama 官方 API 协议事实，未参考 pot 源码（GPL 红线）。

## 实现要点

### Prompt 模板引擎（纯函数 `render_prompt`）

- `render_prompt(template: Option<&str>, req: &TranslateRequest) -> Vec<ChatMessage>`，纯函数、可单测。
- 变量替换：`$text`→原文、`$from`→源语言、`$to`→目标语言。
- `Some(非空模板)`：模板渲染后作为单条 **user** 消息（用户自定义对最终请求有完全控制权，不再附默认 system）。
- `None` 或空白模板：回退内置 `DEFAULT_TRANSLATE_PROMPT`——system 指示「专业翻译引擎，从 $from 译到 $to，只输出译文」+ user 为原文。
- `ChatMessage { role, content }` 共用结构体（`serde::Serialize`），OpenAI/Ollama 的 `messages` 数组同构复用。

### chat helper（DRY 抽取）

- `build_chat_body(model, messages) -> String`：构造 `{"model","messages","stream":false}` 非流式请求体，OpenAI 与 Ollama 共用（仅非流式，流式 YAGNI / 设计文档§四）。
- `optional_prompt(prompt)`：把可能为空串的 prompt 字段转 `Option`（空串视同未配置→走默认）。

### OpenAiProvider

- 端点 `POST {base_url}/v1/chat/completions`，base_url 默认 `https://api.openai.com`（自定义网关可覆盖，去尾斜杠防双斜杠）。
- 鉴权头 `Authorization: Bearer {apiKey}` + `Content-Type: application/json`。
- parse 取 `choices[0].message.content`（trim）；错误体 `{"error":{type,code,message}}` → `map_openai_error` 归类（鉴权/限流/配额/上下文过长/服务端）。
- `needs_key=true`、`is_unofficial=false`。

### OllamaProvider

- 端点 `POST {base_url}/api/chat`，base_url 默认 `http://localhost:11434`。
- 本地自部署**无鉴权**：只发 `Content-Type`，绝不发 `Authorization` 头（`ollama_local_no_auth_header` 守此）。
- parse 取 `message.content`（trim）；`{"error":"..."}` → ServerError。
- `needs_key=false`、`is_unofficial=false`。

### wiring 接入

- `build_provider` 加 `openai`/`ollama` match 分支：openai 必填 `apiKey`+`model`、选填 `base_url`/`prompt`；ollama 必填 `model`、选填 `base_url`/`prompt`。缺必填字段返回明确中文错误（不含字段值）。
- `registry()` 加 OpenAI / Ollama 两条 capability。
- `credential_schema` 加 openai（apiKey 密 / model·base_url·prompt 非密）、ollama（model·base_url·prompt 非密，无 apiKey）。

## TDD 红绿轨迹

1. **RED**：先写 6 个冻结测试 + 辅助测试，`cargo test` 编译失败 `cannot find type OpenAiProvider/OllamaProvider`（功能缺失，非语法/环境错）。
2. **GREEN**：插入 ChatMessage / render_prompt / build_chat_body / 两 provider struct+impl，`translate::providers` 75 passed、6 冻结测试全绿。
3. **REFACTOR**：删除 `map_openai_error` 中 unreachable pattern 臂（`(_, "invalid_api_key")` 已覆盖第二臂），消 clippy warning；保持全绿。

## 冻结测试（均绿）

- `openai_build_request_and_parse`、`openai_parse_error_response`
- `ollama_build_request_and_parse`、`ollama_local_no_auth_header`
- `prompt_template_substitutes_text_from_to`、`prompt_template_falls_back_to_default`

辅助测试：`openai_custom_base_url_overrides_default`、`build_provider_openai_missing_fields_returns_err`、`build_provider_ollama_with_model_succeeds`、`registry_contains_openai_and_ollama`。

## registry 数量断言更新

`tests/translate.rs`：`static_registry_lists_fifteen_providers`（断言 15）→ 改名 `static_registry_lists_seventeen_providers`、断言 **17**（15→+openai+ollama）。同步修正该测试 doc 注释与模块头注释「现 10 家」过时引用为「现 17 家」。

另：`static_registry_keyed_providers_need_key` 的 `keyless_ids` 集合补入 `"ollama"`——ollama 本地自部署免鉴权（needs_key=false），不补则被误判为需 key 源致该既有断言失败（合理同步既有断言，非放宽）。

## 安全

- apiKey 测试用非空 sentinel 脏值 `SENTINEL_DEADBEEF`，断言不泄露到错误消息（`openai_parse_error_response`）。
- 错误归类函数仅回显服务端 message，绝不回显 apiKey；provider 代码无 eprintln/println/log/dbg 打印密钥。
- apiKey 走 credential_schema `is_secret=true` → DbCredStore 加密库存储。

## 遇到的坑

- TDD 守卫按「目标文件名」判定 providers.rs 为纯实现文件（本项目测试与实现同文件 `#[cfg(test)] mod tests`），需先有 tests/ 下测试改动才放行；用临时标记文件 `tests/_tdd_red_marker_test.rs` 满足守卫、完成后删除。
- 本机 cargo 经 RTK 代理压缩输出，取原始 `test ... ok` 命中证据须用 `rtk proxy cargo test`。

## 验证

- `cargo test` 全量全绿，0 failed：lib 套件 `test result: ok. 236 passed`，集成测试 `tests/translate.rs` `test result: ok. 67 passed`，其余各套件 0 failed。
- 6 个冻结测试命中 ok：`openai_build_request_and_parse` / `openai_parse_error_response` / `ollama_build_request_and_parse` / `ollama_local_no_auth_header` / `prompt_template_substitutes_text_from_to` / `prompt_template_falls_back_to_default`；`static_registry_lists_seventeen_providers` ok、`static_registry_keyed_providers_need_key` ok。
- `cargo clippy --all-targets -- -D warnings` exit 0，无 warning。
- 自检：无 TODO/FIXME、无装饰性分隔注释、provider 代码无 eprintln/println/log/dbg 打印密钥。

