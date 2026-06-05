---
id: V6-F2-report
type: feature_report
level: 大功能
parent: V6
children: [V6-F2-S01-code, V6-F2-S01-test, V6-F2-S01-review, V6-F2-S02-code, V6-F2-S02-test, V6-F2-S02-review]
created: 2026-06-05T00:00:00Z
status: 通过
commit: 8646585
acceptance_ids: [V6-F2-A08, V6-F2-A09, V6-F2-A10]
evidence:
  - src/components/UpdateBanner.tsx
  - src/panels/settings/GeneralPanel.tsx
  - src/ipc/ipc-client.ts
  - src/App.tsx
author: 编排（聚合）
---

# 大功能验收报告 · V6-F2 前端更新提示与手动入口

前端补齐更新闭环的用户侧：① `UpdateBanner` 自包含监听后端 `update://ready` 事件，渲染含版本号的非打扰提示条，点「重启更新」调 `restartApp`（对应后端 S03 `restart_app` 命令）；② `GeneralPanel` 手动检查发现新版后升级为可操作——出现「下载并安装」入口触发 `downloadAndInstallUpdate`。IPC 侧新增 `restartApp` / `downloadAndInstallUpdate` 两个封装，并订正 `checkForUpdates` 过时注释。

## 引用的小功能（children）

| 小功能 | 编码 | 测试 | 审查 | 状态 |
|---|---|---|---|---|
| V6-F2-S01 ready-banner（UpdateBanner + App 挂载 + restartApp IPC） | [code](s01-ready-banner/coding.md) | [test](s01-ready-banner/test.md) | [review](s01-ready-banner/review.md) | 通过（8646585） |
| V6-F2-S02 manual-install（GeneralPanel 安装入口 + downloadAndInstallUpdate IPC） | [code](s02-manual-install/coding.md) | [test](s02-manual-install/test.md) | [review](s02-manual-install/review.md) | 通过（8646585） |

## 小功能级验收项对照（归属 F2）

| 验收项 | 结果 | 证据 |
|---|---|---|
| V6-F2-A08 收到 update://ready 渲染含版本号提示条、点重启更新调 restart_app | pass | s01-ready-banner/test.md（命中 `update_banner_shows_on_ready_and_restart_invokes_command` + 双变异变红：去 restartApp 调用 / 去版本号渲染均如期红） |
| V6-F2-A09 手动检查发现新版后出现「下载并安装」、点击触发 downloadAndInstallUpdate | pass | s02-manual-install/test.md（命中 `general_panel_offers_install_after_update_found` + 双变异变红：去调用 / 不渲染按钮均如期红） |

## 大功能级 / 版本级关联验收项

| 验收项 | 结果 | 证据 |
|---|---|---|
| V6-F2-A10（版本级）前后端新增测试纳入套件且全绿 | pass | 全量 `pnpm test` 460 passed（52 文件）；Rust `cargo test update` 7 passed——由 producer 在版本裁决时独立复跑确认 |

## 状态汇总

**通过。** F2 下两个小功能全部三联齐、客观门禁（A08/A09 命中校验 + 双变异 sanity + 全量 460 测试无回归 + tsc --noEmit 0 error）全绿，commit hash 已回填 8646585（无残留 PENDING）。

- S02 reviewer 标出的 Important I01（`UpdateInstallAction` 51 行越「函数≤50」硬规则）已由 coder 提取 `InstallFeedback` 子组件修正至 37 行，全量测试零回归——闭环无遗留。
- 真机端到端（真实从 GitHub 拉 `latest.json` → 静默下载 → 提示条 → 重启完成更新）按设计 §七不可在 headless 复现，归 V6-F2-A12 manual_confirm / 待采证（不阻塞本大功能 done，挂全局未决清单）。
- 无熔断、无阻塞项。
