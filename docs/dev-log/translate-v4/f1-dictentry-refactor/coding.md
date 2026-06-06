---
id: TV4-F1-S01
type: coding
level: 大功能
parent: TV4-F1
children: []
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV4-F1-A01]
evidence:
  - src-tauri/src/translate/mod.rs
  - src-tauri/src/ipc/translate.rs
  - src-tauri/tests/providers.rs
  - src-tauri/tests/translate.rs
  - src/ipc/ipc-client.ts
  - src/panels/translate/TranslatePage.tsx
  - docs/dev-log/translate-v4/f1-dictentry-refactor/artifacts/cargo-test-lib.log
  - docs/dev-log/translate-v4/f1-dictentry-refactor/artifacts/cargo-test-release.log
  - docs/dev-log/translate-v4/f1-dictentry-refactor/artifacts/clippy.log
  - docs/dev-log/translate-v4/f1-dictentry-refactor/artifacts/pnpm-test.log
  - docs/dev-log/translate-v4/f1-dictentry-refactor/artifacts/frozen-tests.log
author: coder
---

# 编码记录 · TranslateResponse 枚举重构（Plain | Dict + DictEntry）

## 做了什么

把翻译结果类型从单译文 struct 重构为可判别枚举 `TranslateResponse`，为后续词典源（TV4-F2/F3）腾出结构化承载，且既有 19 源（机翻 + LLM）零行为回归：

- 后端 `TranslateResponse` 改为 internally tagged 枚举（serde `tag = "kind"`、`rename_all = "lowercase"`）：
  - `Plain { translated: String }` → `{"kind":"plain","translated":"..."}`
  - `Dict { entry: DictEntry }` → `{"kind":"dict","entry":{...}}`
- 新增 `DictEntry`（音标 / 按词性分组释义 `PosDefinition` / 例句 / 发音 / 词形变化，全 `Option`/`Vec` 容空）与便捷构造 `TranslateResponse::plain(...)`。
- 全部既有源 `parse_response` 改返回 `Plain` 变体（构造点统一走 `TranslateResponse::plain`）。
- IPC 边界 `TranslateResultDto` 升级为带判别标签：新增 `kind` 字段（`"plain"`/`"dict"`，与后端 serde tag 取值一致）+ 可选 `entry`；`translate_text_impl` 按 variant 拆分映射，Dict 经 `dict_entry_summary` 压成纯文本写历史并回退展示。
- 前端 `TranslateResult` 改为可判别联合（`TranslatePlainResult | TranslateDictResult`，共享 `translated/sourceLang/targetLang` 基类，`kind` 判别），新增 TS `DictEntry`/`PosDefinition` 类型；消费处（TranslatePage 历史回填）补 `kind: "plain"`。Dict 渲染组件留 TV4-F4，本步只保证类型编译通过、Plain 渲染不回归。

## 关键决策与理由

- **enum 用 struct variant 而非 tuple variant**（`Plain { translated }` 而非 `Plain(String)`）：配合 serde internally tagged（`tag = "kind"`）——internally tagged 要求各变体序列化为 JSON object，tuple/newtype 变体无法内联 tag，struct variant 直接把 `kind` 与字段同层平铺，前端拿到扁平 `{kind, translated}` 即可判别，无需多一层包装。
- **serde tag 选 `kind` + `rename_all = "lowercase"`**：判别字段名 `kind` 与前端可判别联合习惯一致；lowercase 让 tag 值为 `"plain"`/`"dict"`（而非 `"Plain"`），前后端字符串字面量对齐、避免大小写错配。
- **IPC DTO 保留扁平三字段 + 加 `kind`/`entry`，而非直接吐裸 `TranslateResponse`**：既有前端组件与测试大量依赖 `result.translated`/`sourceLang`/`targetLang`，DTO 仍带方向信息且 Plain 路径字段不变 → 既有渲染与历史写入零回归；Dict 路径 `translated` 填词条纯文本摘要兜底，老组件即便不识别 Dict 也有可读回退。
- **`dict_entry_summary` 兜底取释义拼接、再退音标**：历史表 `translated_text` 是纯文本列，词条无法整存；取首批释义（按词性扁平化）join 成单行，无释义再回退音标，保证历史栏可读、不丢信息。
- **`entry` 用 `#[serde(skip_serializing_if = "Option::is_none")]`**：Plain 结果不带 `entry` 键，前端联合类型的 plain 分支无该字段，序列化产物与 TS 类型严格对齐。
- **集成测试各自加局部 `plain_text` helper**：`tests/providers.rs` 与 `tests/translate.rs` 是独立 crate，无法复用单测模块内的 helper，故各自加一份从 `Plain` 取译文、遇 `Dict` panic 的小工具，让断言聚焦译文值。

## 三冻结测试（acceptance TV4-F1-A01，全绿）

- `translate_response_plain_variant_roundtrip`：Plain JSON 往返带 `kind` 标签且语义不丢（断言序列化串为 `{"kind":"plain","translated":"glacier"}`、反序列化相等）。
- `existing_providers_return_plain_no_regression`：既有源（lingva）`parse_response` 仍返回 `Plain`，不回归为 Dict。
- `dict_entry_serializes_with_type_tag`：Dict 变体序列化携带 `kind=dict` 标签，逐字段验音标/词性/释义/例句/词形具体值。

## 坑与注意

- `.translated` 字段名在 `cache.rs`（`CacheEntry.translated` DB 列）与 `tests/translate.rs` 的 `CacheEntry { translated: ... }` 处同名但**无关**，批量 perl 替换只针对 `TranslateResponse` 变量（ok/ok_str/single/multi/resp），未误伤 cache 字段；line 2049 字符串字面量 `"missing data.translations[0].translatedText"` 亦保留。
- `matches!` 模式绑定会按值 move，断言失败消息里若再 `{resp:?}` 会 partial-move 报错 → 统一用 `matches!(&resp, ...)` 借用。
- RTK 代理会把 `cargo test` 输出折叠成 "N passed" 摘要、丢掉 per-test 行与 `test result` 行；验证三冻结命中与主库 `test result: ok` 时用 `rtk proxy cargo test ...` 取原始输出存入 artifacts。

## 验证结果（实跑）

- `cargo test`（主库）：`test result: ok. 246 passed; 0 failed`
- `cargo test --release`（主库）：`test result: ok. 246 passed; 0 failed`，全 32 套件 0 failed
- 三冻结：`test result: ok. 10 passed; 0 failed`（含 3 个目标测试 `... ok`）
- `cargo clippy --all-targets -- -D warnings`：exit 0，0 warning/error
- `pnpm test`：`Test Files 52 passed (52)` / `Tests 465 passed (465)`
- `npx tsc --noEmit`：exit 0，0 error
- 无 TODO/FIXME，无装饰性分隔注释
