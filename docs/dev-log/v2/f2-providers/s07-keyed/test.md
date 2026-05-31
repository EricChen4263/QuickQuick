---
id: V2-F2-S07-test
type: test_report
level: 小功能
parent: V2-F2
created: 2026-05-31T01:27:19Z
status: 通过
commit: WIP
acceptance_ids: [V2-F2-A10, V2-F2-A11]
author: tester
---

# 测试报告：V2-F2-S07 Keyed Providers

## 运行命令

**裸跑，不带任何 --features 参数。**

```bash
# A10：三家 provider 纯函数 + 签名用例
cargo test --manifest-path src-tauri/Cargo.toml providers_keyed > /tmp/T7b.log 2>&1
# exit: 0

# A11：quota 明确提示用例
cargo test --manifest-path src-tauri/Cargo.toml quota_explicit > /tmp/T7q2.log 2>&1
# exit: 0  （过滤关键词为 quota_explicit，原指令 quota_prompt 无匹配用例，实际用例名含 quota_explicit_no_silent_switch）

# providers 集成测试（全文件）
cargo test --manifest-path src-tauri/Cargo.toml --test providers > /tmp/T7p.log 2>&1
# exit: 0

# 全量测试
cargo test --manifest-path src-tauri/Cargo.toml > /tmp/T7all.log 2>&1
# exit: 0

# Clippy 静态检查（-D warnings 零容忍）
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings > /tmp/T7c.log 2>&1
# exit: 0
```

## 结果汇总

| 检查项 | exit | 结论 |
|---|---|---|
| A10 providers_keyed 专项 | 0 | 通过（18 用例） |
| A11 quota_explicit 专项 | 0 | 通过（5 用例） |
| providers 集成测试（--test providers） | 0 | 通过（32 用例） |
| 全量测试 | 0 | 通过（154 用例） |
| Clippy（-D warnings） | 0 | 零警告 |

## A10：providers_keyed_* 用例明细（Baidu / DeepL / Google）

验收标准：capability + build_request + parse_response + 错误码（auth / quota / rate_limit / unsupported）全覆盖；百度签名纯函数可确定性验证。

| 用例名 | 结果 |
|---|---|
| providers_keyed_baidu_capability | ok |
| providers_keyed_baidu_build_request_sign | ok |
| providers_keyed_baidu_sign_pure_function_deterministic | ok |
| providers_keyed_baidu_parse_response_success | ok |
| providers_keyed_baidu_parse_response_quota | ok |
| providers_keyed_baidu_parse_response_rate_limit | ok |
| providers_keyed_baidu_parse_response_unsupported | ok |
| providers_keyed_deepl_capability | ok |
| providers_keyed_deepl_build_request_auth_header | ok |
| providers_keyed_deepl_parse_response_success | ok |
| providers_keyed_deepl_parse_response_quota | ok |
| providers_keyed_deepl_parse_response_auth_error | ok |
| providers_keyed_google_capability | ok |
| providers_keyed_google_build_request_key_and_body | ok |
| providers_keyed_google_parse_response_success | ok |
| providers_keyed_google_parse_response_quota | ok |
| providers_keyed_google_parse_response_auth_error | ok |
| providers_keyed_google_parse_response_code_as_string | ok |

A10 结果：**18/18 通过**（百度 7、DeepL 5、Google 6）

## A11：quota_explicit_no_silent_switch_* 用例明细

验收标准：配额耗尽/认证失败时返回 `NeedKey` 而非静默切换，网络错误返回 `None`，返回类型无自动切换字段。

| 用例名 | 结果 |
|---|---|
| quota_explicit_no_silent_switch_auth_error_returns_need_key | ok |
| quota_explicit_no_silent_switch_keyed_provider_quota_returns_need_key | ok |
| quota_explicit_no_silent_switch_mymemory_quota_returns_need_email | ok |
| quota_explicit_no_silent_switch_network_error_returns_none | ok |
| quota_explicit_no_silent_switch_no_auto_switch_in_return_type | ok |

A11 结果：**5/5 通过**

备注：原指令过滤词 `quota_prompt` 在当前代码库无匹配用例；实际 A11 用例名含 `quota_explicit_no_silent_switch`，使用 `quota_explicit` 可正确匹配，5 用例全通过。

## 全量测试套件明细

| 测试套件 | 用例数 | 通过 | 失败 |
|---|---|---|---|
| 套件 1（单元） | 18 | 18 | 0 |
| 套件 2 | 0 | 0 | 0 |
| 套件 3 | 3 | 3 | 0 |
| 套件 4 | 10 | 10 | 0 |
| 套件 5 | 6 | 6 | 0 |
| 套件 6 | 3 | 3 | 0 |
| 套件 7 | 2 | 2 | 0 |
| 套件 8 | 4 | 4 | 0 |
| 套件 9 | 5 | 5 | 0 |
| providers 集成（S07 主目标） | 32 | 32 | 0 |
| 套件 11 | 9 | 9 | 0 |
| 套件 12 | 61 | 61 | 0 |
| 套件 13 | 1 | 1 | 0 |
| **合计** | **154** | **154** | **0** |

## 覆盖缺口

无。A10 要求的三个 provider 均覆盖 capability / build_request / parse_response / 错误码四类，百度额外覆盖签名纯函数确定性；A11 要求的五个场景均覆盖（auth 失败、keyed 配额、mymemory 配额、网络错误、返回类型无自动切换字段）。无未测试改动。

## 结论

全部验收条件满足：

- A10（V2-F2-A10）：providers_keyed_* 18 用例全绿（较方案预期多 1 个签名纯函数用例）。
- A11（V2-F2-A11）：quota_explicit_no_silent_switch_* 5 用例全绿。
- 全量 154 用例 0 失败。
- Clippy 零警告。

**允许进入下一任务。**
