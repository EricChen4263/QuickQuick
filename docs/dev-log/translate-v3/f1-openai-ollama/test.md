---
id: TV3-F1-S01-test
type: test_report
level: 小功能
parent: TV3-F1
created: 2026-06-06T00:00:00Z
status: 通过
commit: 48adaba
acceptance_ids: [TV3-F1-A01]
---

# TV3-F1 测试报告（动态证伪）· OpenAI + Ollama + Prompt 引擎

> tester 动态证伪（含两次 maxTurns 续跑）。tester 无 Write，编排器据其结构化结论落盘。变异经 cp 备份还原（禁 git checkout）。

## 一、命中校验（RTK 取原始输出，防假绿）
6 个冻结测试真命中（各 1 passed，N≥1）：`openai_build_request_and_parse`、`openai_parse_error_response`、`ollama_build_request_and_parse`、`ollama_local_no_auth_header`、`prompt_template_substitutes_text_from_to`、`prompt_template_falls_back_to_default`。过滤精确命中测试名行确认，无空匹配。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | render_prompt `$text` 替换成空串 | prompt_template_substitutes_text_from_to | 如期红 |
| B | None 分支返回空 messages（不回退默认 Prompt） | prompt_template_falls_back_to_default | 如期红 |
| C | OpenAI parse `choices[0].message.content`→错字段 | openai_build_request_and_parse | 如期红 |
| D | Ollama parse `message.content`→`choices[0]...` | ollama_build_request_and_parse | 如期红 |
| E | Ollama build 误加 `Authorization: Bearer` 头 | ollama_local_no_auth_header | 如期红 |
| F（旁路） | 注释 openai 测试内 `assert_eq!(translated,"你好")` | openai_build_request_and_parse | 仍绿——经分析为多断言组合（URL/model/messages 结构断言仍有效），非单点恒真，测试设计合理 |

A–E 全红，测试有真实判别力，无恒真/旁路假绿。

## 三、边界探测（11 临时边界用例，跑后 cp 还原未留存）
- **panic 安全**：OpenAI/Ollama parse 喂非法 JSON / 空 `choices[]` / 缺 `content` / 缺 `message.content` 均返回 `TranslateError`（ParseError）不 panic（源码 `map_err(ParseError)` / `ok_or_else` 路径）。
- **错误分类**：OpenAI `invalid_api_key→Auth`、`rate_limit_exceeded→RateLimit`、`internal_error→ServerError`；Ollama `{"error":...}→ServerError`。冻结 `openai_parse_error_response` 已覆盖 Auth/RateLimit。
- **密钥不泄露**（hints TV2-RETRO-1）：`grep eprintln|println|dbg!|log:: providers.rs` 零匹配；测试用非空 sentinel `SENTINEL_DEADBEEF` 断言 `!contains`（providers.rs:4357/4428），非空值占位，合规。
- **Prompt 两路径**：自定义模板（仅 1 条 user、$text/$from/$to 替换）+ 默认回退（system+user 两条）均有断言。

## 四、debug + release 双绿 + 抗 flaky
`cargo test` debug 连跑 3× 主库均 `236 passed; 0 failed`（完全一致，无 flaky）+ `cargo test --release` 主库 `236 passed; 0 failed`；集成 `tests/translate.rs` `67 passed`；`cargo clippy --all-targets -- -D warnings` exit 0 无新警告。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致（M credential.rs/providers.rs/tests/translate.rs + 4 无关未跟踪），6 变异 + 11 边界用例经 cp 全还原，tester 未留改动。

## 门禁结论：**通过（放行）**
