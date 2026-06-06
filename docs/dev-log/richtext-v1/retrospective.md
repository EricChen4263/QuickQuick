---
id: RT1-retro
type: retrospective
level: 版本
parent: RT1
children: []
created: 2026-06-07T00:00:00Z
status: 已归档
commit: 0222931
author: orchestrator
promoted: []
---

# 版本复盘 · RT1 剪贴板富文本（HTML）保真

> 只记本版真实发生的事。每条带处置标签，[晋升机制] 项喂下一版启动前置门禁。

## 一、本版台账

### 1. 打回归类
| 小功能 | 打回次数 | 根因归类 | 处置标签 | 晋升去向 |
|---|---|---|---|---|
| RT1-F2-S02 | 1 | ① 改名/改调后遗留过时注释（ClipboardPage.tsx:278 仍写 writeToClipboard）— 规范违反（TV1-RETRO-1 类复发）② 测试 mock 用 `as ReturnType<typeof vi.fn>` ≈ any — 规范违反（禁 any） | [仅观察] | 二者均已有机制覆盖（TV1-RETRO-1 在 hints；禁 any 在 frontend 规范），reviewer 当场抓出并打回、修复后复审 APPROVE。防线有效，暂不新增机制 |

### 2. 复现坑
| 现象 | 出现处 | 根因 | 处置标签 | 晋升去向 |
|---|---|---|---|---|
| coder 自报"全量套件 0 failed"实为误报（S03 声称全绿，实树 traffic_light 集成测试 FAIL） | RT1-F1-S03 coder | coder 未真跑全量 / 误读，把"我的测试过 + 没破坏"当成"全绿"，漏报一个预存在失败（TV3-RETRO-1「交付前必实跑全量」类复发） | [仅观察] | 全局机制已存在（coder 提交前自检清单 + tester 命中校验）；本次 tester 权威重跑 traffic_light 当场抓出虚假陈述、S04 coder 被告知后即如实报告。防线有效（tester 独立重跑兜住），暂不新增 |
| 过时注释/旧引用残留（同 §1.①） | RT1-F2-S02 | 改调后旧名注释未同批清 | [仅观察] | TV1-RETRO-1 已在 hints，reviewer 抓出 |

### 3. 流程摩擦
| 摩擦点 | 表现 | 处置标签 | 晋升去向 |
|---|---|---|---|
| 预存在失败中途暴露、归属不清 | 开版未跑基线，`tests/traffic_light.rs::traffic_light_position_returns_centered_coords` 在 S01 tester 时才暴露 FAIL（实为版本外 commit eff3f36 改窗口几何 12→15/38→44px 漏改的 stale 集成测试）；耗费多轮辨认"是不是我引入的" | **[晋升机制]** | **RT1-RETRO-1**：通用编排经验——**版本启动第一步先跑一次全量测试基线，记录"开版即绿/已知预存在失败清单"**，使版本内任何失败可立即归因（本版 vs 预存在），预存在失败当场决定"先修基线再开版 or 标记隔离"。去向：goal-dev-workflow 规范 §版本启动（+ 可即时落项目 hints.md）。待批准全局落地 |
| 留痕 commit 占位不统一 | 某 review.md 被 reviewer 写成 `commit: WIP` 而非约定的 `PENDING`，回填脚本未匹配到、编排器 grep 补正 | [一次性] | 编排器已 grep 兜底补正；个例，暂不机制化（若 reviewer 反复用非约定占位，转 [晋升机制]：reviewer 契约明确占位用 PENDING） |

## 二、晋升分流
| 通用性 | 条目 | 去向 |
|---|---|---|
| 通用 | RT1-RETRO-1 开版基线测试快照 | goal-dev-workflow 规范 §版本启动（待批准全局落地）；可即时落项目 hints.md |

## 三、晋升回路（下一版启动前执行）
1. 下一版启动须读本文件 `[晋升机制]` 未落地项：**RT1-RETRO-1**（开版基线测试快照）——按通用性落地（全局 goal-dev 规范需用户批准；项目本地可即时落 hints.md），落地后回填 `promoted: [RT1-RETRO-1]`。
2. [仅观察] 项（coder 误报全绿 / 过时注释 / as 断言）：均为已有机制覆盖且被防线当场抓住，继续观察；若复发频次升高再强化。
3. [一次性] 项（commit 占位 WIP）跳过。

## 附：本版亮点（非问题，留作正向经验）
- **arboard 3.6.1 三平台原生 HTML 读写**的核实，把"跨平台富文本"从预想的"手撸 NSPasteboard/CF_HTML/Wayland"降为零新依赖零 cfg 分支——开工前对依赖能力边界的事实核实（读 crate 源码）避免了大量无用工作。
- **安全证伪有真实判别力**：RT1-A-SEC 的"去清洗"变异令安全测试如期 RED，证明 DOMPurify 清洗非恒真假绿（非空真实恶意 payload，四类全覆盖含 iframe）。
