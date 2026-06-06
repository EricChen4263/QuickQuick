---
id: TV3-F3-S01
type: coding
parent: TV3-F3
commit: PENDING
acceptance_ids: [TV3-F3-A01]
---

# TV3-F3-S01 编码留痕：LLM 配置 schema 完整性 + Prompt 引擎收口

## 范围

横切收口小功能（性质同 TV2-F5）。LLM 4 源（openai/ollama/chatglm/gemini）的 provider 实现、
`credential_schema`、`render_prompt` 在 TV3-F1/F2 已落地并接入生产路径。本小功能**不新增源、不改 wiring**，
只做两件事：

1. **补横切验收测试**，让 TV3-F3-A01 的 4 个冻结测试名真实通过：
   - `credential_schema_for_v3_llm_sources`（新增）：累积断言 4 源 schema 字段 + is_secret + required 标记正确。
   - `build_provider_llm_missing_field_errors`（新增）：4 源缺必填字段时 `build_provider` 返回明确错误且不含字段值。
   - `prompt_template_substitutes_text_from_to`（F1 已实现，确认仍绿，本小功能不重写）。
   - `prompt_template_falls_back_to_default`（F1 已实现，确认仍绿，本小功能不重写）。
2. **逐源核对 schema 是否与冻结预期一致**，发现不符则修正。

## schema 核对结果（逐源对照冻结预期）

逐字段核对 `src/translate/credential.rs` 的 `credential_schema`，结论：**4 源 schema 全部已符合冻结预期，零修正**。

| 源 | apiKey | model | base_url | prompt | needs_key | is_unofficial |
|----|--------|-------|----------|--------|-----------|---------------|
| openai  | 密·必填 | 非密·必填 | 非密·选填 | 非密·选填 | true  | false |
| ollama  | （无）  | 非密·必填 | 非密·选填 | 非密·选填 | false | false |
| chatglm | 密·必填 | 非密·必填 | 非密·选填 | 非密·选填 | true  | false |
| gemini  | 密·必填 | 非密·必填 | 非密·选填 | 非密·选填 | true  | false |

- openai/chatglm/gemini 为 OpenAI 同构 4 字段（apiKey 密 + model/base_url/prompt 非密），测试抽 `assert_llm_keyed_schema` helper 累积断言。
- ollama 本地自部署无鉴权，schema **无 apiKey**、仅 3 字段（model 必填 + base_url/prompt 选填）；needs_key=false 由 `OllamaProvider::capability()` 声明（既有 `registry_contains_openai_and_ollama` 测试覆盖）。
- needs_key/is_unofficial 由各 provider 的 `capability()` 提供，非 `credential_schema` 职责；既有 registry 测试已覆盖，本小功能不重复断言。

## 实现决策

- **build_provider 缺字段测试用「用例表 + 闭包」而非 4 段复制**：`cases: &[(&str, Vec<(String,String)>)]` 驱动循环，去重 4 源的 assert 模板（DRY）。
- **错误提取不用 `unwrap_err`**：`build_provider` 的 `Ok` 内层是 `Box<dyn TranslateProvider>`，**不实现 `Debug`**，`unwrap_err` 会编译失败（首轮已撞此错并修正）。改用 `match { Ok(_) => panic!(...), Err(e) => e }` 闭包 `err_of` 提取错误串。
- **防泄露用非空 sentinel 脏值**（hints TV2-RETRO-1）：每例只缺一个必填字段，**其余字段填入 `SENTINEL_DEADBEEF`** 脏值，断言错误消息 `!contains(DIRTY)`。空值占位是恒真假绿，已规避。gemini 用例特意把 `apiKey`（secret）填脏值、缺 model，验证 secret 不进错误消息。

## 命中证据

4 冻结测试逐个子串过滤跑（绕 RTK 取原始输出），均真命中（N≥1）：

- `test translate::credential::tests::credential_schema_for_v3_llm_sources ... ok`（1 passed）
- `test translate::providers::tests::build_provider_llm_missing_field_errors ... ok`（1 passed）
- `test translate::providers::tests::prompt_template_substitutes_text_from_to ... ok`（1 passed）
- `test translate::providers::tests::prompt_template_falls_back_to_default ... ok`（1 passed）

全量 `cargo test`：31 个测试套件全 `test result: ok`，0 failed（lib 套件 244 passed）。原始日志见
`artifacts/cargo-test.log` / `artifacts/cargo-test-release.log`。

## 变异 sanity（自证测试非恒真）

临时把 gemini 段 `apiKey` 的 `is_secret: true` 翻成 `false`（cp 备份 + 改 + 还原，禁 git restore），
`credential_schema_for_v3_llm_sources` 立即 `FAILED` 并精确报「gemini apiKey 应为 secret」，证明断言验具体值、
非恒真。已从备份还原，`git diff` 确认 schema 实现零改动。

## registry 数量

仍 **19 项**（本小功能不新增源），`registry()` 列表未改动。

## 坑 / 注意

- **`cargo test <名> --exact` 会假绿**：`--exact` 把过滤串当**完整路径**匹配，而 Rust 内联测试真实路径含模块前缀（`translate::credential::tests::...`），裸测试名 `--exact` 匹配到 0 个、`0 passed` 仍 exit 0（hints 假绿坑）。**命中校验须用子串过滤（不加 `--exact`）**，确认 `... ok` 行 + `N passed`（N≥1）。
- 本小功能纯补测试，`credential.rs` / `providers.rs` 仅新增 `#[cfg(test)]` 内测试代码，无生产路径改动、无接入/wiring 变更。
