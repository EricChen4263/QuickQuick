---
id: TV2-F4-S01-test
type: test_report
level: 小功能
parent: TV2-F4
created: 2026-06-06T00:00:00Z
status: 通过
commit: 29bae0c
acceptance_ids: [TV2-F4-A01, TV2-F5-A01]
---

# TV2-F4 测试报告（动态证伪）· 火山 SigV4（keyed）

> tester 动态证伪（含一次 maxTurns 续跑）。tester 无 Write，编排器据其结论落盘。

## 一、命中校验
7 个测试真命中（各 1 passed）：`volcengine_sigv4_signature_deterministic`、`volcengine_build_and_parse`、`build_provider_volcengine_missing_secret_returns_err`、`build_provider_volcengine_with_all_fields_succeeds`、`registry_contains_volcengine_keyed_official`、`credential_schema_for_v2_keyed_sources`、`static_registry_lists_fifteen_providers`。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | SigV4 credentialScope `/request`→`/req` | volcengine_sigv4_signature_deterministic | 如期红 |
| B | parse Translation→Text | volcengine_build_and_parse | 如期红 |
| C | secret_access_key is_secret true→false | credential_schema_for_v2_keyed_sources | 如期红 |
| D | build_provider 缺字段不报错 | build_provider_volcengine_missing_secret_returns_err | 如期红 |

四项全红。

## 三、边界探测
- 签名确定性锚定**完整 64 位 hex** `dac06f9e...ee332a61`（非弱断言）。
- parse 异常：非法 JSON→ParseError 不 panic。
- 密钥不泄密：providers.rs grep eprintln/println/dbg/log **0 命中**；SigV4 四层中间密钥（k_date/k_region/k_service/k_signing）无打印。
- 既有 14 源未破坏（registry 15）。
- lang 映射：无专门火山 lang 测试（acceptance 未要求 ref），由 build_and_parse 的 en/zh 间接覆盖——非阻塞。

## 四、debug + release 双绿
`cargo test` 连跑 3× 均 226 passed/0 failed + `cargo test --release` 226 passed；clippy `-D warnings` exit 0。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致，4 变异经 cp 还原。

## 门禁结论：**通过（放行）**
