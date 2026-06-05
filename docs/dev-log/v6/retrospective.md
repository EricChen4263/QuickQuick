---
id: V6-retro
type: retrospective
level: 版本
parent: V6
children: []
created: 2026-06-05T00:00:00Z
status: 已归档
commit: 75d377c
author: orchestrator
promoted: [V6-RETRO-FRICTION-1]
---

# 版本复盘 · V6 自动更新闭环

> 只记本版真实发生的事，每条可溯到具体小功能/打回/证据。

## 一、本版台账（逐条，带处置标签）

### 1. 打回归类

| 小功能 | 打回次数 | 根因归类 | 处置标签 | 晋升去向 |
|---|---|---|---|---|
| 全部 5 个小功能（F1-S01/S02/S03、F2-S01/S02） | 0 | — | — | — |

本版无任何小功能被打回熔断。tester 动态证伪（命中校验 + 变异 sanity）与 reviewer 均一次通过门禁。原因可归到开版前 `hints.md` 已沉淀「下载可测性：把 should_check / payload 构造抽纯函数单测，真实下载/重启隔离薄封装归 manual_confirm」「重启权限 core:default 足够、无需 plugin-process」等关键约定，coder 据此规避了本版最大的两个不确定点（不可测 I/O、capability 权限），未走弯路。

### 2. 复现坑

| 现象 | 出现处 | 根因 | 处置标签 | 晋升去向 |
|---|---|---|---|---|
| 设计明确要求的「既有过时注释订正」在 coding 阶段未随新功能一起完成 | F1-S02（update.rs:34 `check_for_updates` doc、lib.rs:347 `spawn_update_watcher` doc），由 S02 reviewer 标 I-01/I-02、延到 S03 同文件编辑时才补 | coder 聚焦"新增功能"，对设计文档里"落地时顺带订正既有代码"的伴随要求覆盖不全 | [仅观察] | 暂不机制化；若后续版本再现「设计要求的伴随订正被漏」则转 [晋升机制]，去向 coder 契约「把设计文档的伴随订正项纳入交付清单逐条核销」。本版已闭环（S03 补齐 + reviewer grep 复核无残留），ipc-client.ts 的同类过时注释也在 F2-S02 顺带订正。 |

### 3. 流程摩擦

| 摩擦点 | 表现 | 处置标签 | 晋升去向 |
|---|---|---|---|
| RTK 代理压缩测试输出，命中校验拿不到原始证据行 | producer 裁决与 tester 命中校验需要原始 `test <名> ... ok` / `Tests N passed` 行确认 N≥1 防假绿，但本机 cargo/pnpm 经 RTK hook 改写后输出被压成摘要，看不到逐测试名命中——producer 改用 `rtk proxy <cmd>` 绕过代理取原始输出方完成命中校验 | **[晋升机制]** | 项目 `docs/dev-log/hints.md` Procedure 段（项目特定：RTK 是本机配置，非通用）—— **本版已落地 @75d377c**，见 promoted |
| code-reviewer agent 一次 API ConnectionRefused 中断（F2-S02 首轮），review.md 未落盘 | 27 次工具调用后在 Write 前断连，需重新派发一轮 reviewer 才补全 review.md | [一次性] | 环境抖动，不晋升；重派后正常完成 |
| 函数行数硬规则越界被 reviewer 拦截并修正 | F2-S02 `UpdateInstallAction` 51 行越「函数≤50」硬规则，reviewer（置信度85）标 Important，coder 提取 `InstallFeedback` 子组件降至 37 行 | [仅观察]（正向） | 不晋升——这正是 reviewer 门禁按设计生效、硬规则被机制拦住的证据，无需额外机制 |

## 二、晋升分流

| 通用性 | 条目 | 晋升去向 |
|---|---|---|
| 项目特定（RTK 为本机/本项目配置） | V6-RETRO-FRICTION-1：命中校验/裁决取原始测试输出须 `rtk proxy` 绕过代理 | 项目 `docs/dev-log/hints.md`（不进全局 code-standards） |

## 三、晋升回路（落地记录）

- **V6-RETRO-FRICTION-1（[晋升机制]）已即时落地 @75d377c**：写入项目 `docs/dev-log/hints.md` Procedure 段——「命中校验/版本裁决需原始 `test … ok` / `Tests N passed` 行时，本机 cargo/pnpm 经 RTK 代理会压缩输出，须用 `rtk proxy <cmd>` 绕过取原始证据」。已回填本文件 `promoted: [V6-RETRO-FRICTION-1]`。
- 本版为「完成自动更新方案」目标的终版，无后续版本承接；故 [晋升机制] 项不留待"下一版启动门禁"，在本版复盘时即落地，避免沉淀经验丢失。
- [仅观察] 项（设计伴随订正漏项）暂存，复发再转 [晋升机制]。
