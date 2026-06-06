---
id: TV3-F3-report
type: feature_report
level: 大功能
parent: TV3
children: [TV3-F3-S01-code, TV3-F3-S01-test, TV3-F3-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: 06f3c50
acceptance_ids: [TV3-F3-A01]
---

# TV3-F3 大功能验收报告：LLM 配置 schema 完整性 + Prompt 引擎收口

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV3-F3-S01 | credential_schema_for_v3_llm_sources（4 源 schema 累积断言）+ build_provider_llm_missing_field_errors（4 源缺字段错误 + sentinel 脏值）横切收口测试 | 通过 | coding/test/review 齐 |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV3-F3-A01 | **pass** | 4 源 schema 断言具体值（字段/is_secret/needs_key）+ Prompt 替换/回退两路径（F1 实现）+ 缺字段错误不泄露 sentinel + 变异 A–D 红 |

## 横切说明（类似 TV2-F5）
TV3-F3-A01 为跨源验收项：4 源（openai/ollama/chatglm/gemini）的 credential_schema、build_provider 缺字段校验、render_prompt 在 F1/F2 各自实现，本小功能纯测试收口累积覆盖全部 4 源，无独立 F3 生产代码（0 生产改动）。schema 逐字段核对**零修正**（F1/F2 已全符合冻结预期）。

## 门禁
tester 动态证伪通过（4 冻结命中 + 变异 A–D 全红[改生产代码验新测试判别力] + 防泄露 !contains 断言判别力[变异 D] + sentinel 运行时不泄露 + debug×3/release 489 passed + clippy 0）、code-reviewer APPROVE（无 Critical/Important；4 源全覆盖、断言具体值非弱断言、ollama 无 apiKey 差异有断言、sentinel 脏值合规、设计符合）。

## 结论：**通过**（A01 objective pass）。
