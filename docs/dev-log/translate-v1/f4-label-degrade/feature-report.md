---
id: TV1-F4-report
type: feature_report
level: 大功能
parent: TV1
children: [TV1-F4-S01-code, TV1-F4-S01-test, TV1-F4-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F4-A01, TV1-F4-M01]
---

# TV1-F4 大功能验收报告：非官方源标注 + 失败降级提示

## 范围
非官方免 key 源在翻译源选择器显示「⚠ 非官方」标注；当前源为非官方且翻译失败时追加可区分的降级提示。

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV1-F4-S01 | ProviderCapability.is_unofficial + IPC 透出 + DirBar 标注 + TranslateWorkspace 降级提示（顺带修 mod.rs:65 过时注释） | 通过 | coding.md / test.md / review.md |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV1-F4-A01（非官方标注 + 降级提示，官方源无） | **pass** | label-degrade.test.tsx 3 用例（含 nonofficial_source_label_and_degrade_hint）命中 + 变异 C/D 红 + Rust 能力位单测命中 + 变异 A/B 红 |
| TV1-F4-M01（真机目测标注/降级/默认开箱） | 待采证（manual/real_device） | pending-manual |

## 成果
- is_unofficial 分类：5 免key源（lingva/google_free/yandex/transmart/bing）=true、3 官方源（baidu/deepl_free/google）=false，与设计文档§二一致。
- 透传链 Rust ProviderCapability→ProviderDto(camelCase)→TS Provider.isUnofficial 无断口。
- 标注零侵入 Select；降级提示从既有 providers+selectedProviderId 派生、具名常量。
- 顺带订正 F3 携带的 mod.rs:65 过时注释。

## 门禁
tester 动态证伪通过（Rust+前端命中 + 变异 A/B/C/D 全红 + 双向边界 + 既有用例未污染 + 连跑无 flaky）、code-reviewer APPROVE（无 Critical/Important）、cargo clippy/tsc 干净、未抄 pot。

## 结论：**通过**（objective 全 pass；M01 待真机采证）。
