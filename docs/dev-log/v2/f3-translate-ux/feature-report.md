---
id: V2-F3-report
type: feature_report
level: 大功能
parent: V2
children: [V2-F3-S08-code, V2-F3-S08-test, V2-F3-S08-review, V2-F3-S09-code, V2-F3-S09-test, V2-F3-S09-review]
created: 2026-05-31T18:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V2-F3-A12, V2-F3-A13, V2-F3-A14, V2-F3-A15, V2-F3-A16, V2-F3-A17]
evidence: []
author: 编排（聚合）
---

# 大功能验收报告 · V2-F3 触发与呈现 UX

## 引用的小功能（children）
| 小功能 | 编码 | 测试 | 审查 | 状态 |
|---|---|---|---|---|
| V2-F3-S08 选中即译触发纪律 | [code](s08-select-icon/coding.md) | [test](s08-select-icon/test.md) | [review](s08-select-icon/review.md) | 通过（打回1I→复审通过） |
| V2-F3-S09 方向/翻译历史/译文操作 | [code](s09-panel/coding.md) | [test](s09-panel/test.md) | [review](s09-panel/review.md) | 通过（首轮直接通过） |

## 大功能级验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| V2-F3-A12 选中冒图标点击/热键才译 | pass | s08（select_translate_icon_trigger，text_selected→show_icon 非 translate） |
| V2-F3-A13 智能双向方向 | pass | s09（smart_direction） |
| V2-F3-A14 翻译历史分开存储+一键翻译 | pass | s09（translate_history_separate，两表互不混入） |
| V2-F3-A15 译文操作集 | pass | s09（translate_actions，copy/speak/切目标语/换源/存历史） |
| V2-F3-A16 呈现形态(浮窗vs固定面板) | 未决(manual) | pending-manual.yaml；UI 呈现需运行确认 |
| V2-F3-A17 选中即译浮窗淡入动效手感 | 未决(manual) | pending-manual.yaml（CL-V2-001 补的人工确认点）；动效审美需运行确认 |

## 状态汇总
V2-F3 两个小功能（S08/S09）均 done。4 个 objective 验收项（A12/A13/A14/A15）全 pass；A16/A17 为 manual_confirm，UI 呈现/动效证据入 pending-manual，不参与 done、不阻塞。无熔断。大功能 **通过**。
