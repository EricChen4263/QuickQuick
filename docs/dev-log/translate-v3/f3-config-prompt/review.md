---
id: TV3-F3-S01-review
type: review_report
level: 小功能
parent: TV3-F3
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV3-F3-A01]
author: code-reviewer
---

# TV3-F3-S01 审查报告：LLM 配置 schema 完整性 + Prompt 引擎收口

## 审查范围

受审文件：

- `src-tauri/src/translate/credential.rs`：+73 行 `#[cfg(test)]`，新增 `credential_schema_for_v3_llm_sources` 测试 + `assert_llm_keyed_schema` helper。
- `src-tauri/src/translate/providers.rs`：+49 行 `#[cfg(test)]`，新增 `build_provider_llm_missing_field_errors`。
- 0 行生产代码改动（schema / build_provider 实现 F1/F2 已落地）。

审查依据：项目规范 + code-standards（AAA 结构、行为化命名、DRY、无死代码）+ 设计文档 `docs/design/translation-sources-pot.md` §五V3 / §二.2.3 + 验收标准 `acceptance.yaml` TV3-F3-A01 + hints TV2-RETRO-1。

---

## 一、发现项

### Critical

无。

### Important

无。

置信度 ≥80 的问题：**零**。

---

## 二、测试充分性核查（本小功能重点）

### 2.1 `credential_schema_for_v3_llm_sources`

| 检查点 | 结论 |
|---|---|
| 4 源全覆盖 | openai / chatglm / gemini / ollama 均有断言，无漏源 |
| 断言具体值（非弱断言） | 每字段均断言具体的 `is_secret`（true/false）和 `required`（true/false），非仅断言非空或数量——变异 A（openai apiKey is_secret true→false）精确命中 |
| ollama 无 apiKey 差异 | `ol.iter().all(|f| f.key != "apiKey")` 存在性否定断言 + `ol.len() == 3` 字段数双重保护；变异 B（插入 apiKey 字段）命中字段数断言 |
| `assert_llm_keyed_schema` helper | 抽取 openai/chatglm/gemini 同构 4 字段为 helper，避免重复（DRY）；作为普通 fn 不加 `#[test]` 是正确 Rust 惯例 |

### 2.2 `build_provider_llm_missing_field_errors`

| 检查点 | 结论 |
|---|---|
| sentinel 脏值非空（hints TV2-RETRO-1） | `const DIRTY: &str = "SENTINEL_DEADBEEF"` 可识别非空值，非空串占位——规避恒真假绿 |
| `!contains(DIRTY)` 断言有判别力 | 变异 D（chatglm 缺字段错误回显 sentinel 脏值）精确命中 line 4828，证明非恒真 |
| 4 源全覆盖 + 缺字段多样性 | openai 缺 apiKey、ollama 缺 model、chatglm 缺 apiKey、gemini 缺 model（而填 apiKey）——gemini 用例特意把 secret 字段填脏值，最强泄露探测 |
| `err.contains("未配置")` 正向断言 | 与 `!contains(DIRTY)` 配合双向约束，既防空实现假过又防泄露；变异 C（gemini 缺字段返回 Ok）精确命中 panic 分支 |
| `err_of` 闭包设计 | 正确处理 `Box<dyn TranslateProvider>` 不实现 Debug 的编译约束，匹配 Ok(_) 时 panic 提供清晰上下文 |

### 2.3 既有 F1 测试回归

`prompt_template_substitutes_text_from_to` / `prompt_template_falls_back_to_default` 确认仍绿（tester 命中校验证据），本小功能不重写，无回归。

---

## 三、设计符合性核查

对照 `docs/design/translation-sources-pot.md` §二.2.3（LLM 源配置字段）和 `acceptance.yaml` TV3-F3-A01：

| 源 | 设计要求（acceptance.yaml 冻结描述） | 实现 schema | 测试覆盖 | 结论 |
|---|---|---|---|---|
| openai | apiKey(secret)·model(必)·base_url(选)·prompt(选)，needs_key=true，is_unofficial=false | 4 字段完全吻合 | `assert_llm_keyed_schema` 逐字段断言 | 符合 |
| chatglm | apiKey(secret)·model(必)·base_url(选)·prompt(选)，needs_key=true，is_unofficial=false | 4 字段完全吻合 | `assert_llm_keyed_schema` 逐字段断言 | 符合 |
| gemini | apiKey(secret)·model(必)·base_url(选)·prompt(选)，needs_key=true，is_unofficial=false | 4 字段完全吻合 | `assert_llm_keyed_schema` 逐字段断言 | 符合 |
| ollama | model(必)·base_url(选)·prompt(选)，无 apiKey，needs_key=false，is_unofficial=false | 3 字段完全吻合，无 apiKey | 独立断言 + `all(!= apiKey)` | 符合 |

注：设计文档 §二.2.3 对 OpenAI 列出了 `requestPath/service/promptList/stream` 等字段，这些是对 pot 原始配置项的描述；acceptance.yaml TV3-F3-A01 冻结的实现字段为 `base_url/key/model/可编辑Prompt`（简化合并），实现与冻结验收标准一致，无分歧。

---

## 四、规范合规核查

| 规范项 | 结论 |
|---|---|
| 测试命名行为化 | `credential_schema_for_v3_llm_sources` / `build_provider_llm_missing_field_errors` 均为「验证什么行为」命名，符合 |
| AAA 结构 | Arrange（const DIRTY + cases 表）/ Act（err_of 闭包调用）/ Assert（contains + !contains）清晰分离 |
| DRY | `assert_llm_keyed_schema` 消除 3 源重复断言；`err_of` 闭包消除 4 源重复 match 模板；用例表驱动循环消除 4 段复制 |
| 无死代码 | `assert_llm_keyed_schema` 被 3 处调用，无未使用 helper |
| 无 TODO/FIXME | diff 内零遗留 |
| 安全 | DIRTY sentinel 仅在 `#[cfg(test)]` 内，不进生产路径；无打印密钥路径 |
| clippy | tester 报告 exit 0，无新警告 |

---

## 五、审查结论

**APPROVE**

本小功能为纯测试收口，0 生产代码改动。新增 2 个测试完全覆盖 TV3-F3-A01 的 4 源 schema 断言和缺字段错误路径，sentinel 脏值法落到实处（`!contains` 有判别力），变异 A–D 全红证实无恒真/旁路。设计符合性、测试充分性、规范合规均无高置信度问题。

---

**APPROVE**
