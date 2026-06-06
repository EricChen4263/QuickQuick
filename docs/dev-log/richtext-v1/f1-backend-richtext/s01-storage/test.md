---
id: RT1-F1-S01-test
type: test_report
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: b9a0cb2
acceptance_ids: [RT1-F1-A01]
evidence:
  - src-tauri/src/db.rs (tests)
  - src-tauri/tests/richtext_storage.rs
author: tester
---

# 测试报告 · RT1-F1-S01 存储层支持 html

## 运行的测试命令
```
rtk proxy cargo test            # 绕 RTK 代理取原始逐测试输出（hints V6）
cargo test --lib db::tests      # 命中校验
cargo test --test richtext_storage
# 变异 sanity：cp 备份 db.rs → 改 → 跑 → 从备份还原（禁 git checkout，hints F3）
```

## 结果
**通过**（动态证伪：命中校验无假绿 + 3 变异全 RED + 连跑 5 次无 flaky + 边界探测正常）

## 用例清单 + 结果
| 用例 | 结果 | 对应验收项 |
|---|---|---|
| ingest_richtext_roundtrip_persists_html_and_kind | pass | RT1-F1-A01 |
| html_column_migration_idempotent_on_existing_db | pass | RT1-F1-A01 |
| dedup_by_plaintext_unchanged | pass | RT1-F1-A01 |
| ingest_backfills_html_and_upgrades_kind_on_hit | pass | RT1-F1-A01 |
| fresh_db_persists_richtext_roundtrip_through_encrypted_store（集成） | pass | RT1-F1-A01 |
| plaintext_dedup_unchanged_on_encrypted_store（集成） | pass | RT1-F1-A01 |

## 变异 sanity（杀恒真/旁路，每个变异后从 cp 备份还原）
| 变异 | 预期 | 实测 |
|---|---|---|
| 注释补写 `UPDATE ... html_content` 块 | backfill 测试 RED | FAILED：kind left "text" vs right "richtext" ✓ |
| `kind` 恒为 `"text"`（忽略 html） | roundtrip 测试 RED | FAILED：kind 断言不成立 ✓ |
| `migrate_html_column` 守卫改 `if true`（永跳过 ALTER） | 迁移测试 RED | FAILED：迁移后应有列不成立 ✓ |

## 覆盖率
6 用例覆盖 A01 全部分支：迁移幂等 / roundtrip / 纯文本去重不变 / 命中补写升级。边界探测覆盖：空串 html（kind=richtext，符合 is_some 语义）、同 text 多次不同 html（补写只发生一次、不覆盖已有）、已有列库不重复加。reviewer 标注的"旧行已有 html 不覆盖"由边界探测间接覆盖。

## 失败项详情（与本项无关，附注）
- `traffic_light_position_returns_centered_coords` — 预存在失败（`left 15.0` vs `right 12.0`），最后改动在历史 commit 4a4766f，与 db.rs / 富文本变更无关。**不计入本项判定**，但会卡版本级 RT1-A-TEST「全量绿」——由编排器另起处理。

## 结论
RT1-F1-A01 PASS，允许进入下一步。git status 开工/结束逐行一致（cp 还原无残留）。
