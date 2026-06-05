---
id: V6-F1-report
type: feature_report
level: 大功能
parent: V6
children: [V6-F1-S01-code, V6-F1-S01-test, V6-F1-S01-review, V6-F1-S02-code, V6-F1-S02-test, V6-F1-S02-review, V6-F1-S03-code, V6-F1-S03-test, V6-F1-S03-review]
created: 2026-06-05T00:00:00Z
status: 通过
commit: 0db9178
acceptance_ids: [V6-F1-A06, V6-F1-A07]
evidence:
  - src-tauri/src/ipc/update.rs
  - src-tauri/src/lib.rs
  - src-tauri/tests/update_watcher.rs
author: 编排（聚合）
---

# 大功能验收报告 · V6-F1 后端自动更新引擎

后端自动更新引擎闭环：后台 watcher 在首检延迟（8s）后定时轮询（6h），受 `auto_update` 开关控制；检测到新版即静默下载安装、完成后 emit `update://ready{version}` 并去重；新增 `restart_app` 命令供前端触发核心重启。检查/下载的可测逻辑抽为纯函数单测，不可测的真实下载/重启隔离到薄封装层、归真机 manual_confirm（A12）。

## 引用的小功能（children）

| 小功能 | 编码 | 测试 | 审查 | 状态 |
|---|---|---|---|---|
| V6-F1-S01 watcher-gate（should_check 纯函数 + watcher 任务 + 时序常量） | [code](s01-watcher-gate/coding.md) | [test](s01-watcher-gate/test.md) | [review](s01-watcher-gate/review.md) | 通过（32c2806） |
| V6-F1-S02 download-install（下载安装薄封装 + 就绪事件 + 手动命令） | [code](s02-download-install/coding.md) | [test](s02-download-install/test.md) | [review](s02-download-install/review.md) | 通过（0db9178） |
| V6-F1-S03 restart-command（restart_app 命令 + 注册 + 注释订正） | [code](s03-restart-command/coding.md) | [test](s03-restart-command/test.md) | [review](s03-restart-command/review.md) | 通过（0db9178） |

## 小功能级验收项对照（归属 F1）

| 验收项 | 结果 | 证据 |
|---|---|---|
| V6-F1-A01 enabled 时 should_check=true | pass | s01-watcher-gate/test.md（命中 `update_watcher_should_check_when_enabled`） |
| V6-F1-A02 disabled 时 should_check=false | pass | s01-watcher-gate/test.md（命中 + 变异②证判别力） |
| V6-F1-A03 就绪 payload 由版本号构造、事件名 update://ready | pass | s02-download-install/test.md（命中 `update_ready_payload_carries_version` + 双变异变红） |
| V6-F1-A04 一次就绪后去重 | pass | s01-watcher-gate/test.md（命中 `update_watcher_dedupes_after_ready` + 变异①证判别力） |
| V6-F1-A05 restart_app 已注册并调用核心重启 API | pass | s03-restart-command/review.md + test.md（grep 双断言 exit 0 + 删注册/改名变异均破坏） |

## 大功能级验收项对照

| 验收项 | 结果 | 证据 |
|---|---|---|
| V6-F1-A06 改动 Rust 文件 clippy -D warnings 无新增告警 | pass | s02/s03 test.md（`cargo clippy --all-targets -- -D warnings` exit 0，No issues found） |
| V6-F1-A07 验签机制保持启用、未旁路签名 | pass | s02/s03 review.md（`grep "pubkey"` 命中且 `! grep dangerous` exit 0） |

## 状态汇总

**通过。** F1 下三个小功能全部三联齐、客观门禁（命中校验 + 变异 sanity + clippy + grep 断言）全绿，commit hash 已回填真值（S01=32c2806，S02/S03=0db9178，无残留 PENDING）。

- S02 reviewer 曾标出两处过时注释（I-01 `check_for_updates` doc、I-02 `spawn_update_watcher` doc，均设计 §一明确要求修正），因 S03 编辑同两文件，已在 S03 一并订正并由 S03 reviewer grep 复核无残留——闭环无遗留。
- 真实下载安装与进程重启（`download_and_install` / `app.restart()`）按设计 §七不可单测，隔离在薄封装层，归 V6-F2 真机端到端 manual_confirm（A12，不阻塞本大功能 done）。
- 无熔断、无阻塞项。
