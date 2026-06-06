---
id: TV3-F3-S01-test
type: test_report
level: 小功能
parent: TV3-F3
created: 2026-06-06T00:00:00Z
status: 通过
commit: 06f3c50
acceptance_ids: [TV3-F3-A01]
---

# TV3-F3 测试报告（动态证伪）· LLM 配置 schema 完整性 + Prompt 引擎收口

> tester 动态证伪（含一次 maxTurns 续跑）。本小功能纯测试新增（credential.rs/providers.rs 仅 #[cfg(test)] +73/+49 行，0 生产代码改动），重点验新测试判别力——变异改的是 F1/F2 生产代码。tester 无 Write，编排器据其结论落盘。变异经 cp 备份还原（禁 git checkout）。

## 一、命中校验（RTK 完整路径取原始输出，防假绿）
4 冻结测试真命中（各 1 passed）：`credential_schema_for_v3_llm_sources`、`build_provider_llm_missing_field_errors`、`prompt_template_substitutes_text_from_to`（F1 既有仍绿）、`prompt_template_falls_back_to_default`（F1 既有仍绿）。

## 二、变异 sanity（改生产代码验新测试判别力，cp 还原禁 git checkout）
| 变异 | 改坏点（生产代码） | 对应测试 | 结果 |
|---|---|---|---|
| A | credential.rs openai apiKey is_secret true→false | credential_schema_for_v3_llm_sources | 如期红（line 986 apiKey is_secret 断言）|
| B | ollama schema 插入 apiKey 必填字段（3→4） | credential_schema_for_v3_llm_sources | 如期红（line 1038 字段数断言）|
| C | gemini model 必填校验删除（缺也返 Ok） | build_provider_llm_missing_field_errors | 如期红（line 4798 缺字段应 Err）|
| D | chatglm 缺 apiKey 错误消息回显 sentinel 脏值 | build_provider_llm_missing_field_errors | 如期红（line 4828 !contains 防泄露断言）|

A–D 全红，新测试有真实判别力，无恒真/旁路。**变异 D 专攻防泄露 `!contains` 断言**：改坏后精确命中 line 4828，证明该断言非恒真（验证了 hints TV2-RETRO-1 的 sentinel 脏值法落到实处）。

## 三、边界/安全
- **sentinel 不泄露（运行时证据）**：build_provider_llm_missing_field_errors 用 `const DIRTY="SENTINEL_DEADBEEF"` 填各源字段，断言全 4 源缺字段错误消息 `!contains(DIRTY)` 全绿；变异 D 证此断言有判别力。
- **无打印密钥**：grep providers.rs/credential.rs 生产代码 `eprintln|println|log::|dbg!` 零匹配。

## 四、debug + release 双绿 + 抗 flaky
`cargo test` debug 连跑 3× 均 `489 passed; 0 failed`（计数稳定无 flaky）+ `cargo test --release` `489 passed`；`cargo clippy --all-targets -- -D warnings` exit 0 无新警告。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致（M credential.rs/providers.rs + 4 无关未跟踪），变异 A–D 经 cp 全还原，无 git checkout，tester 未留改动。

## 门禁结论：**通过（放行）**
