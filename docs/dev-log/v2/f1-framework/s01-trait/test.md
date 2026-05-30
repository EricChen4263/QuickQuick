---
id: V2-F1-S01-test
type: test_report
level: 小功能
parent: V2-F1
created: 2026-05-30T23:56:13Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A01, V2-F1-A08]
author: tester
---

# 测试报告：V2-F1-S01 翻译 provider 可插拔框架骨架

## 1. 执行命令与结果

| # | 命令 | exit | 结论 |
|---|------|------|------|
| 1 | `cargo test --manifest-path src-tauri/Cargo.toml provider_contract` | 0 | 通过（5 passed） |
| 2 | `cargo test --manifest-path src-tauri/Cargo.toml registry` | 0 | 通过（7+1 passed） |
| 3 | `cargo test --manifest-path src-tauri/Cargo.toml --test translate` | 0 | 通过（12 passed） |
| 4 | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | 0 | 零警告 |

## 2. 验收用例映射表

### V2-F1-A01：provider_contract* 套件

| 验收 ID | assertion 摘要 | 测试用例 | runner | 结果 |
|---------|---------------|---------|--------|------|
| V2-F1-A01 | `capability()` 返回字段正确（id/name/needs_key 与实现一致） | `provider_contract_capability_returns_correct_fields` | `tests/translate.rs` | **通过** |
| V2-F1-A01 | `build_request()` 产出可断言的请求描述符 | `provider_contract_build_request_produces_assertable_descriptor` | `tests/translate.rs` | **通过** |
| V2-F1-A01 | `parse_response()` 在 JSON 非法时返回 Err | `provider_contract_parse_response_returns_error_on_invalid_json` | `tests/translate.rs` | **通过** |
| V2-F1-A01 | `parse_response()` 在字段缺失时返回 Err | `provider_contract_parse_response_returns_error_on_missing_field` | `tests/translate.rs` | **通过** |
| V2-F1-A01 | `parse_response()` 正常提取翻译文本 | `provider_contract_parse_response_extracts_translated_text` | `tests/translate.rs` | **通过** |

5 / 5 用例全部通过。

### V2-F1-A08：static_registry* 套件

| 验收 ID | assertion 摘要 | 测试用例 | runner | 结果 |
|---------|---------------|---------|--------|------|
| V2-F1-A08 | 注册表恰好含 4 家 provider | `static_registry_lists_four_providers` | `tests/translate.rs` | **通过** |
| V2-F1-A08 | 注册表含 Google | `static_registry_contains_google` | `tests/translate.rs` | **通过** |
| V2-F1-A08 | 注册表含 DeepL | `static_registry_contains_deepl` | `tests/translate.rs` | **通过** |
| V2-F1-A08 | 注册表含 Baidu | `static_registry_contains_baidu` | `tests/translate.rs` | **通过** |
| V2-F1-A08 | 注册表含 MyMemory | `static_registry_contains_mymemory` | `tests/translate.rs` | **通过** |
| V2-F1-A08 | 需 key 的 provider 均标记 needs_key=true | `static_registry_keyed_providers_need_key` | `tests/translate.rs` | **通过** |
| V2-F1-A08 | MyMemory 不需要 key（needs_key=false） | `static_registry_mymemory_does_not_need_key` | `tests/translate.rs` | **通过** |
| V2-F1-A08 | `registry()` 多次调用结果幂等 | `translate::providers::tests::registry_is_idempotent` | `src/lib.rs`（单元测试） | **通过** |

8 / 8 用例全部通过（7 来自集成套件 + 1 来自单元测试）。

## 3. translate 集成套件全量

`cargo test --test translate` 命令跑整个 `tests/translate.rs`，不做关键词过滤：

| 套件 | 通过 | 失败 | 跳过 |
|------|------|------|------|
| `tests/translate.rs`（全量 12 个用例） | 12 | 0 | 0 |
| `src/lib.rs`（registry 单元测试） | 1 | 0 | 0 |
| clippy（--all-targets） | — | 0 警告 | — |

## 4. 覆盖缺口

无缺口。

- A01 的三个 trait 方法（`capability`、`build_request`、`parse_response`）均有专项用例覆盖；`parse_response` 同时覆盖正常路径与两种错误路径（JSON 非法、字段缺失）。
- A08 的 4 家 provider 各有 contains 用例；数量边界、key 标志的正反两面均已覆盖；幂等性由单元测试守护。
- clippy 零警告，无需额外静态检查。

## 5. 结论

**门禁：放行。**

A01 通过（5 用例），A08 通过（8 用例），共 2/2 验收条目、13 个测试用例全绿；clippy 零警告。V2-F1-S01 翻译 provider 可插拔框架骨架可进入下一任务（Phase 6 审查 / 下一 Story）。
