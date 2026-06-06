---
id: TV3-F1-S01-review
type: review_report
level: 小功能
parent: TV3-F1
created: 2026-06-06T00:00:00Z
status: 通过
commit: 2704e42
acceptance_ids: [TV3-F1-A01]
author: code-reviewer
---

# TV3-F1-S01 审查报告：LLM chat-completion + Prompt 模板引擎 + OpenAI + Ollama

## 审查结论

**APPROVE** — 无置信度 ≥80 的 Critical 或 Important 问题，代码符合项目规范与设计文档要求。

## 审查范围

- `src-tauri/src/translate/providers.rs`：ChatMessage / render_prompt / build_chat_body / optional_prompt / OpenAiProvider / OllamaProvider + 10 项测试（6 冻结 + 4 辅助）。
- `src-tauri/src/translate/credential.rs`：openai schema（apiKey 密 / model·base_url·prompt 非密）、ollama schema（无 apiKey，三字段均非密）。
- `src-tauri/tests/translate.rs`：registry 断言 15→17、keyless_ids 补 ollama、注释同步。

## 发现项

### Critical（阻塞级）

无。

### Important（建议级）

无置信度 ≥80 的 Important 项。

## 逐项核查

### 1. 项目规范符合性

| 规范项 | 结果 |
|---|---|
| 函数 ≤50 行 | 通过（所有新增函数 ≤35 行） |
| 嵌套 ≤3 层 | 通过（parse_response 嵌套 ≤3） |
| 注释写「为什么」 | 通过（pub 函数均有 doc 注释，说明设计意图） |
| 无死代码 / 装饰性分隔注释 | 通过 |
| 公共 API 文档完整 | 通过（ChatMessage / render_prompt / OpenAiProvider / OllamaProvider 均有 doc） |
| 命名描述性 | 通过（optional_prompt / build_chat_body / resolved_base 均符合动名词规范） |

### 2. 设计符合性（docs/design/translation-sources-pot.md）

- **§二.2.3 端点与鉴权**：OpenAI `POST {base_url}/v1/chat/completions` + `Authorization: Bearer {apiKey}`，Ollama `POST {base_url}/api/chat` 本地无鉴权——与设计文档一致。
- **§四 非流式 YAGNI**：`build_chat_body` 固定 `stream: false`，无流式分支——符合。
- **§〇 独立重写不抄 pot**：检查了全文 `pot` 出现位置（15 处），全部为 `lingva.pot-app.com` 域名（Lingva 公开实例 URL）及「非 pot 源码」声明注释，无任何 pot 源码引入——通过。
- **Prompt 模板引擎**：`render_prompt` 实现 `Some(非空模板)→单条 user`、`None/空白→system+user` 默认回退，与 coding.md §Prompt 模板引擎 描述一致。

### 3. 安全核查

| 安全点 | 结果 |
|---|---|
| apiKey `is_secret=true` 走 DbCredStore 加密存储 | 通过（credential.rs openai schema 第一字段 is_secret=true） |
| 生产路径无 eprintln/println/log/dbg 打印密钥 | 通过（grep 全文零匹配） |
| 错误消息不含密钥值 | 通过（map_openai_error 仅回显服务端 msg，不格式化 apiKey） |
| Ollama 不发 Authorization 头 | 通过（OllamaProvider::build_request headers 列表仅含 Content-Type） |
| 密钥不泄露断言用非空 sentinel（hints TV2-RETRO-1） | 通过（`SENTINEL_DEADBEEF` + `!contains` 断言，`if let Err(e) = auth_err` 上方 `matches!` 已确保分支必然进入，无假绿风险） |

### 4. 既有回归验证

- registry 从 15→17（+openai +ollama），`static_registry_lists_seventeen_providers` 断言数值已同步。
- `keyless_ids` 补入 `"ollama"`（needs_key=false 本地免鉴权），与 capability 声明一致，`static_registry_keyed_providers_need_key` 断言合理同步。
- 模块头注释「现 10 家/15 家」已更新为「现 17 家」，无过时注释残留（符合 hints TV1-RETRO-1 要求全仓清旧引用）。

### 5. 测试充分性复核

| 测试要点 | 状态 |
|---|---|
| Prompt 自定义路径（$text/$from/$to 替换） | `prompt_template_substitutes_text_from_to` 覆盖 |
| Prompt 默认回退路径（system+user 两条） | `prompt_template_falls_back_to_default` 覆盖 |
| OpenAI 端点/Bearer 头/messages 结构/parse | `openai_build_request_and_parse` 覆盖 |
| OpenAI 错误分类（Auth/RateLimit） | `openai_parse_error_response` 覆盖 |
| Ollama 端点/无鉴权/parse | `ollama_build_request_and_parse` 覆盖 |
| Ollama needs_key=false 守卫 | `ollama_local_no_auth_header` 覆盖 |
| OpenAI 自定义 base_url 覆盖 | `openai_custom_base_url_overrides_default` 覆盖 |
| build_provider 缺必填字段报错 | `build_provider_openai_missing_fields_returns_err` 覆盖 |
| registry 含两新 provider | `registry_contains_openai_and_ollama` 覆盖 |
| 密钥不泄露（sentinel 脏值 + !contains） | 通过，无假绿风险 |

tester 报告（test.md）变异 A–E 全红，变异 F 分析合理（多断言组合仍有判别力）。边界探测 11 用例覆盖 panic 安全 / 错误分类 / Prompt 两路径。

## 最终机器结论

APPROVE
