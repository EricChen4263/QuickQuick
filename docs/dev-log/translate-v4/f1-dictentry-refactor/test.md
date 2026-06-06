---
id: TV4-F1-S01-test
type: test_report
level: 小功能
parent: TV4-F1
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV4-F1-A01]
---

# TV4-F1 测试报告（动态证伪）· TranslateResponse 枚举重构（Plain|Dict + DictEntry）

> tester 动态证伪（含一次 maxTurns 续跑）。本小功能为 TV4 地基重构，触及全部 19 源 + DTO + 前端类型，重点验不回归。tester 无 Write，编排器据其结论落盘。变异经 cp 备份还原（禁 git checkout）。

## 一、命中校验（RTK 完整路径取原始输出，防假绿）
3 冻结测试真命中（lib 内联 + tests/translate_response_refactor.rs 集成各命中，N≥1）：`translate_response_plain_variant_roundtrip`、`existing_providers_return_plain_no_regression`、`dict_entry_serializes_with_type_tag`。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | LingvaProvider parse 返回 Dict 而非 Plain | existing_providers_return_plain_no_regression | 如期红（plain_text helper 在 Dict 分支 panic）|
| B | serde tag "kind"→"type" | dict_entry_serializes_with_type_tag + plain_variant_roundtrip | 如期红（两测 tag 断言失败）|
| C | DictEntry 字段 phonetic→pronunciation | dict_entry_serializes_with_type_tag | 如期红（E0609 字段名锚定）|
| D | Plain 变体 serde rename "plaintext"（破坏 tag 值） | translate_response_plain_variant_roundtrip | 如期红（json["kind"]≠"plain"）|

A–D 全红，无恒真/旁路。序列化断言锚定具体串/字段名（非弱断言）。

## 三、边界 / 不回归（核心）
- **既有 19 源不回归**：全量 cargo test 主库 `246 passed; 0 failed`（含 Lingva/GoogleFree/Baidu 等全部既有 provider 测试），全 32 套件 495 passed 0 failed。
- **前端 Plain 渲染不回归**：pnpm test `465 passed`（52 文件，含 TranslatePage/trans-popover/MiniTranslate Plain 路径）。
- **serde 往返**：Plain→`{"kind":"plain","translated":"glacier"}`、Dict→`{"kind":"dict","entry":{phonetic/definitions/examples/audio/inflections}}`，前端可判别联合对齐。
- **安全**：grep providers.rs `eprintln|println|log::|dbg!` 零匹配。

## 四、debug×3 + release + pnpm×3 + clippy + tsc
`cargo test` debug 连跑 3× 均 `495 passed; 0 failed`（无 flaky）+ `cargo test --release` `495 passed`；`pnpm test` 连跑 3× 均 `465 passed`；`cargo clippy --all-targets -- -D warnings` exit 0 No issues；`npx tsc --noEmit` 0 error。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致（11 M 后端+前端文件 + 无关未跟踪 + 新增 tests/translate_response_refactor.rs），变异 A–D 经 cp 全还原，无 git checkout。

## 门禁结论：**通过（放行）**
