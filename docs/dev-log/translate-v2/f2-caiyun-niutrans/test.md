---
id: TV2-F2-S01-test
type: test_report
level: 小功能
parent: TV2-F2
created: 2026-06-06T00:00:00Z
status: 通过
commit: 29bae0c
acceptance_ids: [TV2-F2-A01, TV2-F5-A01]
---

# TV2-F2 测试报告（动态证伪）· 彩云 + 小牛（keyed）

> tester 动态证伪（含一次 maxTurns 续跑）。tester 无 Write，编排器据其结论落盘。

## 一、命中校验
8/8 真命中（各 1 passed）：`caiyun_build_and_parse`、`niutrans_build_and_parse`、`credential_schema_for_v2_keyed_sources`、`build_provider_caiyun_missing_token_returns_err`、`build_provider_niutrans_missing_apikey_returns_err`、`static_registry_lists_twelve_providers`、`map_lang_for_provider_caiyun_only_zh_en_ja`、`map_lang_for_provider_niutrans_uses_zh_cht_codes`。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | caiyun parse target→result | caiyun_build_and_parse | 如期红 |
| B | caiyun 鉴权头 "token "→"bearer " | caiyun_build_and_parse | 如期红 |
| C | niutrans parse tgt_text→tgt_result | niutrans_build_and_parse | 如期红 |
| D | caiyun token is_secret true→false | credential_schema_for_v2_keyed_sources | 如期红 |

四项全红。

## 三、边界探测
- caiyun target 数组/字符串两形态正确取译文；无 target/非法 JSON→Err/ParseError 不 panic。
- niutrans error_code 字符串/数字兼容；非法 JSON→ParseError。
- 签名不泄密：providers.rs grep eprintln/println 含 token/apikey **0 行**。
- 既有 10 源未破坏（debug 全量 209 passed）。

## 四、debug + release 双绿
`cargo test` + `cargo test --release` 均 lib 209 passed + 各集成套件 0 failed；clippy `-D warnings` exit 0（干净代码）。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致，4 变异经 cp 还原。

## 门禁结论：**通过（放行）**
