---
id: TV2-F3-S01-test
type: test_report
level: 小功能
parent: TV2-F3
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV2-F3-A01, TV2-F5-A01]
---

# TV2-F3 测试报告（动态证伪）· 腾讯 TC3 + 阿里 HMAC（keyed）

> tester 动态证伪（含一次 maxTurns 续跑）。tester 无 Write，编排器据其结论落盘。

## 一、命中校验
10 个目标测试真命中（221 passed，N≥1）：`tencent_tc3_signature_deterministic`、`tencent_build_and_parse`、`alibaba_hmac_signature_deterministic`、`alibaba_build_and_parse`、`credential_schema_for_v2_keyed_sources`、`build_provider_tencent_missing_secret_key_returns_err`、`build_provider_alibaba_missing_secret_returns_err`、`static_registry_lists_fourteen_providers`、`map_lang_for_provider_tencent_uses_zh_and_zh_tw`、`map_lang_for_provider_alibaba_uses_zh_and_zh_tw_lowercase`。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | TC3 派生首层前缀 "TC3"→"WRONG" | tencent_tc3_signature_deterministic | 如期红 |
| B | alibaba HMAC key 后缀 "&" 去掉 | alibaba_hmac_signature_deterministic | 如期红 |
| C | tencent parse TargetText→WrongField | tencent_build_and_parse | 如期红 |
| D | alibaba parse Translated→WrongField | alibaba_build_and_parse | 如期红 |
| E | tencent secret_key is_secret true→false | credential_schema_for_v2_keyed_sources | 如期红 |

五项全红，复杂签名/字段/schema 均有判别力。

## 三、边界探测
- 签名确定性锚定**具体值**（非弱断言）：tencent Authorization 含 Signature `cc913306...ed7decaf`（64 hex）；alibaba Base64 `+uwyBbn3LNXWPJOuNcXCiWB/32k=`；参照向量来自独立 Python 按官方文档手算。
- parse 异常：非法 JSON→ParseError、鉴权→Auth、限流→RateLimit，全 Result 不 panic。
- 密钥不泄密：providers.rs grep eprintln/println/dbg/log **0 匹配**。
- 既有 12 源未破坏（registry 14、221 passed 含既有源）。

## 四、debug + release 双绿
`cargo test` 221 passed（连跑 2×）+ `cargo test --release` 221 passed 0 failed；clippy `-D warnings` exit 0。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致，5 变异经 cp 还原。

## 门禁结论：**通过（放行）**
