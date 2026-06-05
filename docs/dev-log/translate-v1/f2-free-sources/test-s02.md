---
id: TV1-F2-S02-test
type: test_report
level: 小功能
parent: TV1-F2
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F2-A01]
evidence:
  - src-tauri/src/translate/providers.rs
  - docs/dev-log/translate-v1/f2-free-sources/artifacts/cargo-test-s02-run1.log
---

# TV1-F2-S02 测试报告（动态证伪）· Yandex + Transmart 免key源

> tester 动态证伪（含一次 maxTurns 续跑）。tester 无 Write，本报告由编排器据其返回结论落盘。

## 一、命中校验（杀假绿）
8 个新测试真实命中（N=8，非空匹配/skip），providers 套件 178 passed：
`yandex_build_request_endpoint_and_body`、`yandex_parse_extracts_text_array`、`transmart_build_request_endpoint_and_json_body`、`transmart_parse_concatenates_auto_translation`、`map_lang_for_provider_yandex_zh_not_split_traditional`、`map_lang_for_provider_transmart_distinguishes_traditional`、`static_registry_lists_seven_providers`、`static_registry_keyed_providers_need_key` 均 `... ok`。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | Yandex parse `text`→`code` | yandex_parse_extracts_text_array | 如期红 |
| B | Transmart parse `auto_translation`→错字段 | transmart_parse_concatenates_auto_translation | 如期红 |
| C | yandex needs_key false→true | static_registry_keyed_providers_need_key | 如期红 |
| D | build_request 去掉 `srv=android` | yandex_build_request_endpoint_and_body | 如期红 |

四处全红，测试有真实判别力。

## 三、边界探测
- Yandex 异常：`{"code":405}`→Auth、空 text[]→ParseError、非法 JSON→ParseError；多段拼接正确。
- Transmart 异常：`ret_code!=succ`→ServerError、全空/缺 auto_translation→ParseError、非法 JSON→ParseError；含空串多段拼接 `["","冰川",""]`→"冰川"。
- 既有源完整：registry 7 源 id+needs_key 全核对（lingva/google_free/yandex/transmart 免key；baidu/deepl_free/google 需key），均未被破坏。
- 安全 TV1-A-SEC：providers/lang/credential 无 eprintln/println，无译文泄露。
- lang 映射：Yandex zh 不分繁简、Transmart 区分 zh/zh-TW。

## 四、抗 flaky
全量 `cargo test` 连跑 3 次均 lib 178 passed / 外部 67 passed / 0 failed，无 flaky。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致，变异经 cp 还原，未用 git checkout/restore。

## 门禁结论：**通过（放行）**
