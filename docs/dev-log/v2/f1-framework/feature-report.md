---
id: V2-F1-report
type: feature_report
level: 大功能
parent: V2
children: [V2-F1-S01-code, V2-F1-S01-test, V2-F1-S01-review, V2-F1-S02-code, V2-F1-S02-test, V2-F1-S02-review, V2-F1-S03-code, V2-F1-S03-test, V2-F1-S03-review, V2-F1-S04-code, V2-F1-S04-test, V2-F1-S04-review, V2-F1-S05-code, V2-F1-S05-test, V2-F1-S05-review]
created: 2026-05-31T18:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A01, V2-F1-A02, V2-F1-A03, V2-F1-A04, V2-F1-A05, V2-F1-A06, V2-F1-A07, V2-F1-A08]
evidence: []
author: 编排（聚合）
---

# 大功能验收报告 · V2-F1 provider 可插拔框架

## 引用的小功能（children）
| 小功能 | 编码 | 测试 | 审查 | 状态 |
|---|---|---|---|---|
| V2-F1-S01 框架 trait+注册表 | [code](s01-trait/coding.md) | [test](s01-trait/test.md) | [review](s01-trait/review.md) | 通过（首轮直接通过） |
| V2-F1-S02 语言归一 | [code](s02-lang/coding.md) | [test](s02-lang/test.md) | [review](s02-lang/review.md) | 通过（打回3I→复审通过） |
| V2-F1-S03 错误/降级/超时取消 | [code](s03-error/coding.md) | [test](s03-error/test.md) | [review](s03-error/review.md) | 通过（打回2I+1次要→复审通过） |
| V2-F1-S04 凭据 schema | [code](s04-credential/coding.md) | [test](s04-credential/test.md) | [review](s04-credential/review.md) | 通过（打回3I安全→复审通过） |
| V2-F1-S05 翻译缓存 | [code](s05-cache/coding.md) | [test](s05-cache/test.md) | [review](s05-cache/review.md) | 通过（打回2I→复审通过） |

## 大功能级验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| V2-F1-A01 薄 provider 三件契约 | pass | s01（provider_contract，横切下沉） |
| V2-F1-A02 语言归一(检测/BCP-47/映射) | pass | s02（lang_normalize_and_direction） |
| V2-F1-A03 统一错误枚举 | pass | s03（error_enum_mapping） |
| V2-F1-A04 同源退避不跨源 failover | pass | s03（same_source_retry_no_cross_failover，sleep_fn 真实退避） |
| V2-F1-A05 凭据 schema secret→keychain | pass | s04（credential_schema_keychain，未知字段报错不降级） |
| V2-F1-A06 缓存键含 provider+LRU | pass | s05（cache_key_includes_provider_lru，换源必 miss） |
| V2-F1-A07 超时/取消在途 | pass | s03（timeout_and_cancel_inflight） |
| V2-F1-A08 静态注册表 4 家 | pass | s01（static_registry_lists_four） |

## 状态汇总
V2-F1 五个小功能（S01-S05）均 done。8 个 objective 验收项全 pass。薄 provider + 厚框架（横切：语言归一/错误降级/凭据/缓存/超时取消下沉）。无熔断。大功能 **通过**。
