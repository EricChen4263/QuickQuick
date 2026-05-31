---
id: V3-F3-report
type: feature_report
level: 大功能
parent: V3
children: [V3-F3-S07-code, V3-F3-S07-test, V3-F3-S07-review, V3-F3-S08-code, V3-F3-S08-test, V3-F3-S08-review, V3-F3-S09-code, V3-F3-S09-test, V3-F3-S09-review]
created: 2026-05-31T20:30:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F3-A08, V3-F3-A09, V3-F3-A10, V3-F3-A11, V3-F3-A12, V3-F3-A13]
evidence: []
author: 编排（聚合）
---

# 大功能验收报告 · V3-F3 主窗口 UI + 改键 + onboarding

## 引用的小功能（children）
| 小功能 | 编码 | 测试 | 审查 | 状态 |
|---|---|---|---|---|
| V3-F3-S07 主窗口三栏壳路由 | [code](s07-shell/coding.md) | [test](s07-shell/test.md) | [review](s07-shell/review.md) | 通过（打回2I测试质量→复审通过） |
| V3-F3-S08 设置改键UI+子项栏 | [code](s08-settings/coding.md) | [test](s08-settings/test.md) | [review](s08-settings/review.md) | 通过（打回2I测试质量→复审通过） |
| V3-F3-S09 Accessibility引导降级 | [code](s09-onboarding/coding.md) | [test](s09-onboarding/test.md) | [review](s09-onboarding/review.md) | 通过（通过+polish 2建议） |

## 大功能级验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| V3-F3-A08 主窗口左栏一级仅三入口历史二级 | pass | s07（main_window_nav 13 测试） |
| V3-F3-A09 改键实时校验冲突拒绝 | pass | s08（rebind_ui_conflict，"已被占用"拒绝） |
| V3-F3-A10 设置六子项+App排除名单 | pass | s08（settings_sections_and_exclude_ui 11 测试） |
| V3-F3-A11 Accessibility引导+优雅降级 | pass | s09（accessibility_onboarding_degrade，未授权 WriteBackOnly 不发粘贴不崩） |
| V3-F3-A12 主窗口三页视觉还原 | 未决(manual) | pending-manual.yaml；UI 三页视觉需运行确认 |
| V3-F3-A13 主窗口/弹窗动效材质手感 | 未决(manual) | pending-manual.yaml（CL-V3-001 补的人工确认点） |

## 状态汇总
V3-F3 三小功能（S07-S09）均 done。4 个 objective 验收项（A08/A09/A10/A11）全 pass；A12（视觉还原）/A13（动效材质）为 manual_confirm，UI/动效证据入 pending-manual，不参与 done、不阻塞。无熔断。大功能 **通过**。
