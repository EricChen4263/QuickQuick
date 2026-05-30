---
id: V1-F1-report
type: feature_report
level: 大功能
parent: V1
children: [V1-F1-S01-code, V1-F1-S01-test, V1-F1-S01-review, V1-F1-S02-code, V1-F1-S02-test, V1-F1-S02-review, V1-F1-S03-code, V1-F1-S03-test, V1-F1-S03-review]
created: 2026-05-31T15:10:00Z
status: 通过
commit: WIP
acceptance_ids: [V1-F1-A01, V1-F1-A02, V1-F1-A03, V1-F1-A04, V1-F1-A05, V1-F1-A06, V1-F1-A07, V1-F1-A08]
evidence: []
author: 编排（聚合）
---

# 大功能验收报告 · V1-F1 剪贴板捕获引擎

## 引用的小功能（children）
| 小功能 | 编码 | 测试 | 审查 | 状态 |
|---|---|---|---|---|
| V1-F1-S01 捕获核心 | [code](s01-capture-core/coding.md) | [test](s01-capture-core/test.md) | [review](s01-capture-core/review.md) | 通过（打回1I→复审通过） |
| V1-F1-S02 去重+置顶刷新 | [code](s02-dedup/coding.md) | [test](s02-dedup/test.md) | [review](s02-dedup/review.md) | 通过（打回1C+2I→复审通过） |
| V1-F1-S03 隐私门控 | [code](s03-privacy/coding.md) | [test](s03-privacy/test.md) | [review](s03-privacy/review.md) | 通过（打回2I格式→复审通过） |

## 大功能级验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| V1-F1-A01 双字段(纯文本+富文本)同存 | pass | s01（capture_dual_field） |
| V1-F1-A02 ~500ms 轮询一递增即捕获 | pass | s01（poll_changecount_triggers_capture + reset 防御） |
| V1-F1-A03 防自污染私有标记跳过 | pass | s01（self_write_marker_skipped） |
| V1-F1-A04 去重+置顶刷新(文本哈希判重) | pass | s02（dedup_and_bump，FNV-1a 稳定哈希） |
| V1-F1-A05 置顶刷新显式改库不新建 | pass | s02（bump_no_new_record） |
| V1-F1-A06 concealed/transient 跳过不猜内容 | pass | s03（concealed_skipped + concealed_no_heuristic 反证） |
| V1-F1-A07 App 排除名单不记录 | pass | s03（app_exclude_list） |
| V1-F1-A08 托盘暂停不捕获 | pass | s03（pause_stops_capture） |

## 状态汇总
V1-F1 三个小功能（S01/S02/S03）均 done。8 个 objective 验收项全 pass。ClipboardBackend 抽象 + poll_once_with_policy 隐私门控 + db.rs 去重/置顶/收藏一体。无熔断阻塞。大功能 **通过**。
