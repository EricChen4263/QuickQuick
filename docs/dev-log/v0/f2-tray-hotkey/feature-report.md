---
id: V0-F2-report
type: feature_report
level: 大功能
parent: V0
children: [V0-F2-S02-code, V0-F2-S02-test, V0-F2-S02-review, V0-F2-S03-code, V0-F2-S03-test, V0-F2-S03-review]
created: 2026-05-31T10:30:00Z
status: 通过
commit: 088216c
acceptance_ids: [V0-F2-A01, V0-F2-A02, V0-F2-A03, V0-F2-A04, V0-F2-A05]
evidence: []
author: 编排（聚合）
---

# 大功能验收报告 · V0-F2 托盘 + 全局热键 + 预热窗口

## 引用的小功能（children）

| 小功能 | 编码 | 测试 | 审查 | 状态 |
|---|---|---|---|---|
| V0-F2-S02 全局热键配置与冲突检测 | [code](s02-hotkey/coding.md) | [test](s02-hotkey/test.md) | [review](s02-hotkey/review.md) | 通过（reviewer 打回 1I→复审通过） |
| V0-F2-S03 预热窗口+托盘+全局热键 | [code](s03-prewarm-window/coding.md) | [test](s03-prewarm-window/test.md) | [review](s03-prewarm-window/review.md) | 通过（reviewer 打回 4I→复审通过） |

> 注：S04（自启动）虽留痕在本目录 `s04-autostart/`，但其验收项 V0-F1-A05 归属 F1，已计入 F1 报告，不在本报告重复。

## 大功能级验收项对照

| 验收项 | 结果 | 证据 |
|---|---|---|
| V0-F2-A01 默认热键 V/T + 可改键持久化 | pass | s02/test.md（hotkey_defaults_and_rebind pass，默认 CmdOrCtrl+Shift+V/T） |
| V0-F2-A02 冲突检测拒绝保存（"已被占用"不崩溃） | pass | s02/test.md（hotkey_conflict_rejected pass，配置不变） |
| V0-F2-A03 预热窗口按 V/T 路由（路由单测） | pass | s03/test.md（windowRoute 4 测试 pass；runner 经 change_log CL-V0-001 校正） |
| V0-F2-A04 托盘常驻+热键唤起活动屏上中+瞬开体感 | 未决(manual) | 功能已实现（conf 自动建托盘 + global-shortcut + window_pos 定位 + 失焦即隐）；GUI 体感 headless 无法取证，并入 docs/dev-log/pending-manual.yaml（含 H-01 托盘时序、H-02 高DPI 定位观察项） |
| V0-F2-A05 图标资源（png/ico/icns 多尺寸） | pass | s03/test.md（ls icons exit 0，icon.icns/icon.ico/多 png 齐） |

## 状态汇总

V0-F2 两个小功能（S02、S03）均 done。大功能级 4 个 objective 验收项（A01/A02/A03/A05）全部 pass；A04 为 manual_confirm，功能已实现、证据归 pending-manual，不参与 done 判定、不阻塞。无熔断阻塞。大功能 **通过**。
