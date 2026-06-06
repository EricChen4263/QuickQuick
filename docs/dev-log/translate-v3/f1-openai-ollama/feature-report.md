---
id: TV3-F1-report
type: feature_report
level: 大功能
parent: TV3
children: [TV3-F1-S01-code, TV3-F1-S01-test, TV3-F1-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: 2704e42
acceptance_ids: [TV3-F1-A01]
---

# TV3-F1 大功能验收报告：OpenAI + Ollama + Prompt 引擎

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV3-F1-S01 | chat-completion 抽象（ChatMessage/build_chat_body）+ Prompt 模板引擎（render_prompt：$text/$from/$to 替换、None 回退默认）+ OpenAiProvider（Bearer、/v1/chat/completions）+ OllamaProvider（本地无鉴权、/api/chat）+ schema | 通过 | coding/test/review 齐 |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV3-F1-A01 | **pass** | openai/ollama build+parse 命中 + 错误分支 + 本地无 Authorization 头 + 变异 A–E 红 |
| TV3-F3-A01（贡献 Prompt 引擎部分） | 部分（待 F3 收口整体裁决） | prompt_template 替换/回退两路径命中 + 变异 A/B 红 |

## 门禁
tester 动态证伪通过（6 冻结命中 + 变异 A–E 全红 + 11 边界用例 panic 安全 + 错误分类 Auth/RateLimit/ServerError + sentinel 不泄密 + debug×3/release 236 passed 无 flaky + clippy 0）、code-reviewer APPROVE（无 Critical/Important；端点/鉴权符合设计§二.2.3、非流式符合§四、未抄 pot、apiKey is_secret 加密存储、Ollama 不发 Authorization 头、registry 15→17 与 keyless_ids 同步无回归）。

## 关键决策
- 可编辑 Prompt 语义：`Some(非空模板)`→模板作单条 user 消息（用户完全控制）；`None`/空白→内置默认 system（含 $from/$to）+ user 原文。
- DRY：OpenAI/Ollama 请求体同构抽 build_chat_body 共用；parse 差异点各自处理。
- Ollama needs_key=false（本地自部署免鉴权），归 keyless_ids。

## 结论：**通过**（A01 objective pass；真网/本地 Ollama 真译 manual 待 TV3-M01 采证）。
