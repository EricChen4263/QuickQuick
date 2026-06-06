---
id: TV2-F1-S01-test
type: test_report
level: 小功能
parent: TV2-F1
created: 2026-06-06T00:00:00Z
status: 通过
commit: 29bae0c
acceptance_ids: [TV2-F1-A01, TV2-F5-A01]
---

# TV2-F1 测试报告（动态证伪）· 百度专业 + 有道（keyed）

> tester 动态证伪（含两次 maxTurns 续跑）。tester 无 Write，本报告由编排器据其结论落盘。

## 一、命中校验（杀假绿）
8/8 命中（各 1 passed）：`baidu_field_sign_and_build`、`baidu_field_parse`、`youdao_sign_v3_and_build`、`youdao_parse`、`credential_schema_for_v2_keyed_sources`、`build_provider_baidu_field_missing_required_fields_returns_err`、`build_provider_youdao_missing_required_fields_returns_err`、`static_registry_lists_ten_providers`。
（acceptance ref `build_provider_missing_required_field_errors` 为聚合名，落地为两源各一函数，均命中。）

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | baidu_field 签名去掉 field | baidu_field_sign_and_build | 如期红 |
| B | youdao_sign 去掉 curtime | youdao_sign_v3_and_build | 如期红 |
| C | youdao_truncate 阈值改恒返全文 | youdao_sign_v3_and_build（21字符截断断言） | 如期红 |
| D | baidu_field secret_key is_secret true→false | credential_schema_for_v2_keyed_sources | 如期红 |
| E | baidu_field build_provider 缺字段不报错 | build_provider_baidu_field_missing_required_fields_returns_err | 如期红 |

五项全红，签名确定性/凭据/校验测试均有判别力。

## 三、边界探测
- youdao truncate 边界：恰 20（全文）/21（前10+len+后10），中文按 chars 不按字节、不 panic。
- parse 异常：baidu_field error_code→Auth/非法 JSON→ParseError；youdao errorCode!=0→Auth/非法 JSON→ParseError，全 is_err 不 panic。
- 签名不泄密：providers.rs grep eprintln/println/dbg/log **0 匹配**；缺字段错误消息断言不含 secret 值。
- 既有 8 源未破坏：static_registry 全绿；基础 `baidu_sign` 仍 4 参未变。

## 四、debug + release 双绿
`cargo test` 连跑 3× + `cargo test --release` 连跑 3× 均 0 failed；clippy `-D warnings` exit 0（干净代码；变异 C 残留的 doc-lazy-continuation 还原后消失）。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致，变异全经 cp 还原，未用 git checkout/restore。

## 门禁结论：**通过（放行）**
