---
id: V1-F3-S06-test
type: test_report
level: 小功能
parent: V1-F3
created: 2026-05-30T23:33:39Z
status: 通过
commit: WIP
acceptance_ids: [V1-F3-A15, V1-F3-A16, V1-F3-A17, V1-F3-A18]
author: tester
---

# 测试报告：V1-F3-S06 回写粘贴

## 1. 执行命令与结果

| # | 命令 | exit | 结论 |
|---|------|------|------|
| 1 | `cargo test --manifest-path src-tauri/Cargo.toml --test paste` | 0 | 通过 |
| 2 | `pnpm test paste-mode` | 0 | 通过 |
| 3 | `cargo test --manifest-path src-tauri/Cargo.toml`（全量） | 0 | 通过 |
| 4 | `pnpm test`（全量） | 0 | 通过 |
| 5 | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | 0 | 零警告 |
| 6 | `pnpm exec tsc --noEmit` | 0 | 无错误 |

## 2. 验收用例映射表

| 验收 ID | assertion 摘要 | 测试用例 | runner | 结果 |
|---------|---------------|---------|--------|------|
| V1-F3-A15 | 写回后轮询 changeCount 反映我方写入再发粘贴；超时保护 | `paste_timing_paste_waits_changecount`<br>`paste_timing_timeout_when_count_never_increases` | `cargo test … --test paste` | **通过** |
| V1-F3-A16 | Enter 触发写回+粘贴；修饰键组合仅写回不粘贴 | `paste-mode.test.ts`（2 tests） | `pnpm test paste-mode` | **通过** |
| V1-F3-A17 | 唤起→记录前台 app→隐面板→激活原 app→等前台→模拟粘贴路径逻辑 | `focus_restore_path_sequence_matches_spec` | `cargo test … --test paste` | **通过** |
| V1-F3-A18 | 粘贴后剪贴板留下被选条目 X | `paste_leaves_selected_item_on_clipboard` | `cargo test … --test paste` | **通过** |

4 / 4 验收条目全部通过。

## 3. 全量回归

| 套件 | 通过 | 失败 | 跳过 |
|------|------|------|------|
| Rust（全量，7 个测试二进制） | 28 | 0 | 0 |
| 前端 Vitest（全量） | 33 | 0 | 0 |
| clippy | — | 0 警告 | — |
| tsc | — | 0 错误 | — |

## 4. 覆盖缺口

无缺口。A15 含正常路径（changeCount 提升）与超时降级两条分支；A16 同时覆盖 Enter 粘贴与修饰键仅复制两种模式；A17 覆盖焦点恢复完整路径序列；A18 覆盖粘贴后剪贴板归属。

## 5. 结论

**门禁：放行。**

A15 通过（2 用例）、A16 通过（2 用例）、A17 通过（1 用例）、A18 通过（1 用例），共 4/4 验收条目；全 Rust 套件绿、全前端套件绿、clippy 零警告、tsc 无错。S06 回写粘贴可进入下一任务。
