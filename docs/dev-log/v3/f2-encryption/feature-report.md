---
id: V3-F2-report
type: feature_report
level: 大功能
parent: V3
children: [V3-F2-S04-code, V3-F2-S04-test, V3-F2-S04-review, V3-F2-S05-code, V3-F2-S05-test, V3-F2-S05-review, V3-F2-S06-code, V3-F2-S06-test, V3-F2-S06-review]
created: 2026-05-31T20:30:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F2-A05, V3-F2-A06, V3-F2-A07]
evidence: []
author: 编排（聚合）
---

# 大功能验收报告 · V3-F2 加密落地与失败恢复

## 引用的小功能（children）
| 小功能 | 编码 | 测试 | 审查 | 状态 |
|---|---|---|---|---|
| V3-F2-S04 密钥可访问性 | [code](s04-key/coding.md) | [test](s04-key/test.md) | [review](s04-key/review.md) | 通过（条件通过单行→复核通过） |
| V3-F2-S05 失败分级恢复 | [code](s05-recovery/coding.md) | [test](s05-recovery/test.md) | [review](s05-recovery/review.md) | 通过（打回4I→复审通过） |
| V3-F2-S06 导出导入便携文件 | [code](s06-export/coding.md) | [test](s06-export/test.md) | [review](s06-export/review.md) | 通过（打回2I→复审通过） |

## 大功能级验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| V3-F2-A05 密钥可访问性 AfterFirstUnlock+ThisDeviceOnly | pass | s04（key_accessibility_flags；ThisDeviceOnly不漫游真实OS落地，AfterFirstUnlock精确属性诚实标注+pending V0-F3-A03-H01） |
| V3-F2-A06 失败分级（瞬时不碰库/永久备份不删+确认重建） | pass | s05（encryption_failure_tiered 8 测试） |
| V3-F2-A07 导出/导入便携文件口令保护 | pass | s06（export_import_passphrase，argon2id+AES-256-GCM，错口令验签失败，密钥 Zeroizing） |

## 状态汇总
V3-F2 三小功能（S04-S06）均 done。3 个 objective 验收项全 pass。密钥 ThisDeviceOnly 不漫游真实落地（AfterFirstUnlock 精确属性差距诚实标注入 pending V0-F3-A03-H01）；失败分级永不静默删库；便携文件 argon2id+AES-256-GCM 口令保护、密钥内存清零。安全核心扎实。无熔断。大功能 **通过**。
