---
id: V2-F1-S04-test
type: test_report
level: 小功能
parent: V2-F1
created: 2026-05-31T00:35:07Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A05]
author: tester
---

# 测试报告：V2-F1-S04 Credential Schema + Keychain 路由（I-1/I-2/I-3 修复后）

## 1. 执行命令与结果

| # | 命令 | exit | 通过数 | 结论 |
|---|------|------|--------|------|
| 1 | `cargo test --manifest-path src-tauri/Cargo.toml credential` | **0** | 12 | 通过 |
| 2 | `cargo test --manifest-path src-tauri/Cargo.toml --test schema` | **0** | 8 | 通过 |
| 3 | `cargo test --manifest-path src-tauri/Cargo.toml --test translate` | **0** | 55 | 通过 |
| 4 | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | **0** | — | 零警告 |

## 2. 验收用例映射表（V2-F1-A05）

### 集成测试：credential schema + keychain 路由

| 测试用例 | 验证内容 | 结果 |
|---------|---------|------|
| `credential_schema_keychain_baidu_has_app_id_and_secret_key` | Baidu schema 含 `app_id` 与 `secret_key` 两字段 | **通过** |
| `credential_schema_keychain_google_has_api_key` | Google schema 含 `api_key` 字段 | **通过** |
| `credential_schema_keychain_deepl_has_auth_key` | DeepL schema 含 `auth_key` 字段 | **通过** |
| `credential_schema_keychain_mymemory_has_optional_email` | MyMemory schema 含可选 `email` 字段 | **通过** |
| `credential_schema_keychain_unknown_provider_returns_err` | 未知 provider 返回 `Err`，不降级（I-1 修复验证） | **通过** |
| `credential_schema_keychain_unknown_field_returns_err_and_does_not_write_db` | 未知字段返回 `Err` 且不写 DB（I-2 修复验证） | **通过** |
| `credential_schema_keychain_secret_routes_to_keychain_non_secret_routes_to_db` | secret 字段路由 keychain，非 secret 字段路由 DB（I-3 修复验证） | **通过** |
| `credential_schema_keychain_load_returns_saved_values` | mock store save 后 load 返回相同值 | **通过** |

### 单元测试（`translate::credential::tests`）

| 测试用例 | 验证内容 | 结果 |
|---------|---------|------|
| `credential_schema_unknown_provider_returns_empty` | 未知 provider schema 返回空 | **通过** |
| `credential_schema_baidu_fields_count_is_two` | Baidu schema 字段数量恰好为 2 | **通过** |
| `keychain_account_format_is_correct` | keychain 账户键格式正确 | **通过** |
| `save_and_load_routes_correctly_with_mock_store` | mock store 路由读写一致性 | **通过** |

**A05 合计：12 / 12 通过。**

## 3. 负向路径覆盖确认

| 负向场景 | 对应测试用例 | 预期行为 | 实际结果 |
|---------|------------|---------|---------|
| 未知 provider | `credential_schema_keychain_unknown_provider_returns_err` | 返回 `Err`，不降级 | 通过 |
| 未知字段 | `credential_schema_keychain_unknown_field_returns_err_and_does_not_write_db` | 返回 `Err`，DB 无写入 | 通过 |

两条负向用例均明确断言错误路径不静默降级，覆盖 I-1 与 I-2 修复意图。

## 4. 回归套件全量

| 套件 | 通过 | 失败 | 跳过 |
|------|------|------|------|
| `cargo test credential`（含单元 + 集成 credential 用例） | 12 | 0 | 0 |
| `cargo test --test schema`（含 provider_config 表断言） | 8 | 0 | 0 |
| `cargo test --test translate`（全量） | 55 | 0 | 0 |
| `clippy --all-targets -D warnings` | — | 0 警告 | — |

**总计：75 个用例全绿，clippy 零警告。**

### schema 回归：provider_config 表断言

`schema_preembed_provider_config_table_exists_with_required_columns` 在 `--test schema` 套件中通过，确认 S03 引入的 `provider_config` 表结构未被 S04 改动破坏。

## 5. Mock 隔离确认

`credential_schema_keychain_*` 及 `save_and_load_routes_correctly_with_mock_store` 均采用内存 mock store，测试全程不访问系统钥匙串（macOS Keychain），无弹窗、无真实写入。

## 6. 覆盖缺口

无缺口。

- A05 核心语义（schema 字段定义、secret/非-secret 路由规则、mock store 读写一致性）已由 8 条集成用例全量覆盖。
- 四家 provider（DeepL / Baidu / Google / MyMemory）各有专项字段断言，字段数量及必填/可选均覆盖。
- 负向路径（未知 provider、未知字段不降级）已有专项测试，不存在静默失败风险。
- 回归套件（translate × 55 + schema × 8）确认无副作用破坏已有功能。

## 7. 结论

**门禁：放行。**

A05 通过（12 / 12），translate 回归通过（55 / 55），schema 回归通过（8 / 8，含 provider_config 断言），clippy 零警告。I-1 / I-2 / I-3 三项修复验收通过。V2-F1-S04 Credential Schema + Keychain 路由可进入下一阶段。
