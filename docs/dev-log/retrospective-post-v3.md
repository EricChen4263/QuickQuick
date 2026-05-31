---
id: post-v3-retro
type: retrospective
level: 维护轮次
parent: null
children: []
created: 2026-05-31T22:30:00Z
status: 已归档
commit: 10ec94a
author: orchestrator
promoted:
  - F1-maxturns
  - F2-sendmessage-mitigation
  - F3-tester-restore
  - P1-boot-smoke
---

# 版本复盘 · V3 之后维护轮次（构建流程 + 启动修复）

> 范围：V0–V3 完成后的一轮维护——搭建快速构建流程（Makefile + GitHub Actions 发布 + 双语 README）、本地验证、修复启动期 autostart panic 并补回归守卫。
> 这些条目大多是**工作流工具链自身**的流程摩擦，按"经验→机制"原则即时晋升；本文件作留痕，供下一版启动门禁核对（均已落地，无未决晋升项）。

## 一、本轮台账（逐条带处置标签）

### 2. 复现坑

| 现象 | 出现处 | 根因 | 处置标签 | 晋升去向 |
|---|---|---|---|---|
| 应用**编译通过、启动即 panic**（`PluginInitialization("autostart", invalid type: map, expected unit)`） | `tauri.conf.json` plugins.autostart | conf 写了该插件不接受的 map 配置块（插件配置目标为 unit）；单元测试只测模块逻辑、不过启动反序列化，抓不到 | `[晋升机制]` | **P1**：项目本地——新增 `src-tauri/tests/boot_smoke.rs` 配置反序列化守卫（JSON 合法性 + autostart 须为 unit）；已落地 @10ec94a |
| 回归测试**假绿**两次：①手写 `from_value::<()>` 近似、②mock_builder 真 boot 因 trayIcon→`NotMainThread` 提前返回、被当通过 | boot_smoke.rs 迭代过程 | 测试没真覆盖被测路径；mock boot 在本项目不可行（托盘初始化先于插件初始化） | `[仅观察]` | 已被 tester 变异 sanity（加回坏配置→断言变红）抓出并纠正——印证"动态证伪/变异检查"的必要，无需新机制 |

### 3. 流程摩擦（工作流工具链自身）

| 摩擦点 | 表现 | 根因 | 处置标签 | 晋升去向 |
|---|---|---|---|---|
| 子 agent **反复"截断"** | coder/tester 多次在任务中途停下、返回半句 + 一个无法使用的 `agentId` | 实为撞 `maxTurns` 上限被强停（coder 用 38/33/32 > 上限 30；tester 17 > 15）；TDD/动态证伪回合密集，上限设太低 | `[晋升机制]` | **F1**：全局——`coder maxTurns 30→60`、`tester 15→40` @ `~/.claude/agents` |
| `SendMessage` 续跑通道不可用 | 撞顶 agent 给的 `agentId` 无工具可用，只能重派全新 agent、丢上下文、重复劳动 | SendMessage 是实验性"团队(teammates)"功能，默认未启（需 `--agents`/`--team-name`/`--teammate-mode` 等旗标以 team 模式启动）；普通单会话不挂载 | `[晋升机制]` | **F2**：全局——coder/tester 防截断纪律加"回合预算有限+尽早落痕+超大任务交付最小片段"，`goal-dev-workflow` 编排纪律加"派发切小到预算内" @ `~/.claude` |
| tester 变异还原**冲掉未提交修复** | tester 用 `git checkout -- tauri.conf.json` 还原变异，把同文件上**未提交的 autostart 修复**一起抹回 HEAD，bug 一度复活 | 在带未提交改动的文件上用回-HEAD 命令还原；契约原文恰好写的就是 `git checkout` | `[晋升机制]` | **F3**：全局——tester 还原改为"改前 `cp` 备份→改后从备份复原"，明令禁止 `git checkout`/`git restore`；自证改为开工/结束 `git status` 快照逐行比对 @ `~/.claude/agents/tester.md` |

## 二、晋升分流（按通用性）

| 通用性 | 条目 | 去向 |
|---|---|---|
| **通用**（任何项目都成立） | F1 回合上限、F2 撞顶可续保险、F3 还原纪律 | 全局 `~/.claude/agents/{coder,tester}.md`、`skills/goal-dev-workflow/SKILL.md`——**均已落地** |
| **项目特定**（本技术栈） | P1 boot 配置反序列化守卫；autostart 配置须为 unit | 项目本地 `src-tauri/tests/boot_smoke.rs`——**已落地 @10ec94a** |

## 三、晋升回路状态

本轮全部 `[晋升机制]` 项**已落地**（见 `promoted`），下一版启动门禁无需再处理。`[仅观察]` 项（假绿）暂不机制化，已由现有动态证伪机制覆盖。

**沉淀的可复用原则**：
- 配置类/启动类 bug 编译不报、单测抓不到——**发布前必须真跑一次应用**（boot 冒烟），并为该类 bug 留配置反序列化/启动守卫。
- 在带未提交改动的文件上做变异测试，**还原只能用备份、不能回 HEAD**。
- 给子 agent 派活要**量入为出**：任务切到回合预算内、尽早落痕，撞顶也留可续状态。
