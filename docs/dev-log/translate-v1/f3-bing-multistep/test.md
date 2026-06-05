---
id: TV1-F3-S01-test
type: test_report
level: 小功能
parent: TV1-F3
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F3-A01]
evidence:
  - src-tauri/src/translate/mod.rs
  - src-tauri/src/translate/providers.rs
  - docs/dev-log/translate-v1/f3-bing-multistep/artifacts/cargo-test-run1.log
---

# TV1-F3-S01 测试报告（动态证伪）· 多步架构 + Bing 源

> tester 动态证伪（含一次 maxTurns 续跑）。tester 无 Write，本报告由编排器据其返回结论落盘。架构敏感改造，重点验证既有源零回归。

## 一、命中校验（杀假绿）
8 个 Bing 测试真实命中（N=8，非空匹配）：
`bing_two_step_translate_with_mock_executor`、`bing_parse_response_extracts_translation_text`、`bing_translate_token_step_failure_returns_error_not_panic`、`bing_translate_empty_token_returns_auth_error`、`bing_translate_step_failure_returns_error`、`bing_is_keyless_and_built_without_credentials`、`registry_contains_bing_keyless`、`map_lang_for_bing_uses_zh_hans_and_hant` 均 `... ok`。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | Bing parse 取错字段（text→to） | bing_parse / two_step | 如期红 |
| B | 跳过 token 步（固定 token） | token_step_failure / empty_token / two_step | 3 测试如期红（两步真在跑） |
| **C（架构关键）** | 改坏 `TranslateProvider::translate` 默认实现（返回固定值跳过 parse） | **既有单步源 lingva 的 ipc_translate_*_direction** | **如期红**——证明既有源确实经由新默认 translate 路径且路径语义正确，架构不回归成立 |
| D | bing needs_key false→true | is_keyless / registry_contains_bing_keyless | 如期红 |

四档全红，含架构关键变异 C 击中既有源测试。

## 三、边界探测
- Bing 异常（RoutingFakeExecutor 注入，不 panic）：token 步失败→Network 且不发翻译步、空 token→Auth、翻译步失败→RateLimit 透传、空/缺/非法 JSON→ParseError。
- **既有 7 源零回归**：tests/providers.rs 28 passed；免key 4 源（lingva/google_free/yandex/transmart）+ 需key 3 源（baidu/deepl_free/google）parse/build/capability 全绿。
- providers.rs/mod.rs 无 eprintln，无 token/text/译文泄露。

## 四、抗 flaky
全量 `cargo test` 连跑 4 次（含开工 1 次）均 0 failed（429 passed），无 flaky。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致，4 次变异经 cp 还原，未用 git checkout/restore。

## 门禁结论：**通过（放行）**
架构扩展不回归（变异 C 坐实既有源经新路径）、Bing 两步正确、异常全映射、连跑无 flaky、工作区干净。
