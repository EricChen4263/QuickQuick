---
id: v4-retro
type: retrospective
level: 版本
parent: V4
children: []
created: 2026-06-01T02:30:00Z
status: 已归档
commit: 956e07e
author: orchestrator
promoted:
  - F1-tester-flaky-multirun
  - F2-orderby-determinism
  - F3-coder-deliverable-order
---

# 版本复盘 · V4（主窗口 UI 落地）

> 范围：把 V0–V3 的「后端业务逻辑」「前端纯逻辑」两座孤岛，经 IPC 桥 + 启动数据管道 + React 三页 UI 接成可用主窗口，并修菜单栏双图标、落设计语言。11 个小功能、3 个大功能、producer 三轮裁决。
> 本文件作留痕；可机检的经验已即时晋升为全局机制（见 `promoted`），供下一版启动门禁核对。

## 一、本轮台账（逐条带处置标签）

### 1. 真实缺陷（producer 独立重跑抓出，下层漏判）

| 现象 | 根因 | 处置标签 | 晋升去向 |
|---|---|---|---|
| **翻译历史倒序「假绿」**：`ipc_translate_list_history_returns_entries_in_desc_order` 实际 5passed/1failed，但 S02 tester 报"6 passed 全过" | `list_translate_history` 的 `ORDER BY created_utc DESC` 缺确定性兜底；同毫秒并列时顺序不定，tester 单跑一次恰好命中正确序 → 假绿放行 | `[晋升机制]` | **F2**：全局 `~/.claude/rules/code-general.md` 加"排序查询 ORDER BY 必须确定性（并列加 rowid/主键兜底）"。本项修法 `, rowid DESC` |
| **并发 flaky**：`ipc_clipboard_toggle_favorite_puts_item_first` 全量并发跑约 1/4 FAILED，单独跑稳定 | 同一类根因——`list_items_full`/`list_ordered` 的 `ORDER BY ...last_modified_utc DESC` 缺兜底；并发负载下同测试内两次 ingest 易落同毫秒 → 并列 → 排序不定 | `[晋升机制]` | **F1+F2**：F2 同上（排序确定性）；F1（见下：tester 抗 flaky 多跑）。修法两查询均加 `, rowid DESC` |
| **clippy dead_code 漏网**：`tests/boot_pipeline.rs` 的 `FakeClipboardBackend::set_text` 从未调用，`cargo test`/`build` 不报、`clippy -D warnings` 才拦 | tester/coder 自检只跑 `cargo test`/`cargo check`，未跑 `clippy -D warnings`；该项是 A-QUALITY 门禁但小功能阶段没纳入复跑 | `[仅观察]` | 已被 producer 独立重跑 A-QUALITY 抓出；coder 提交前自检清单已含 clippy 类项，强化"涉及 -D warnings 的项小功能阶段也跑一次"即可，暂不新增机制 |

### 2. 流程摩擦

| 摩擦点 | 表现 | 根因 | 处置标签 | 晋升去向 |
|---|---|---|---|---|
| **tester 全量只跑一次放行 flaky** | S02/S04 tester 全量 `cargo test` 各只跑 1 次报全绿，漏掉并发 flaky，被 producer 多次重跑揪出 | 命中校验默认"跑一次过即算过"，未对并发/排序/共享资源类做抗 flaky 多跑 | `[晋升机制]` | **F1**：全局 `~/.claude/agents/tester.md` 命中校验加"全量套件/涉排序时间戳并列项必须连跑 ≥3 次（理想5），每次全绿才过，记录连跑结论" |
| **子 agent 反复在 coding.md / 接入处截断** | 多个 coder 在"组件+测试做完"后，于写 coding.md 或接入 App.tsx 处被截断（撞 maxTurns 或异常早停），留痕缺失/接入未做，需补派续完 | 重 UI 任务回合消耗大；coder 习惯把"接入"和"留痕"放最后当收尾，恰是被截断的位置 | `[晋升机制]` | **F3**：全局 `~/.claude/agents/coder.md` 防截断加"多交付项任务测试一过立即按序落地：①完成接入/wiring ②写 coding.md 骨架 ③才润色；接入与留痕都是交付物不是收尾" |
| **窗口形态产品决策夹在实现里** | §9.3 主窗口（侧栏三页）与原 400×600 无边框预热弹窗冲突，S06 顺带把窗口改成 960×640 带边框主窗口 | 单一 window 同时承担"弹窗"与"主窗口"两种设计意图，V4 未拆分 | `[待用户确认]` | 不机制化；已在 feature-report 与交付说明显式标注供用户拍板（§8 选中即译浮窗作为独立窗口/未来项）|

## 二、晋升分流（按通用性）

| 通用性 | 条目 | 去向 |
|---|---|---|
| **通用**（任何项目都成立）| F2 排序确定性兜底、F1 tester 抗 flaky 多跑、F3 coder 交付序 | 全局 `~/.claude/rules/code-general.md`、`agents/tester.md`、`agents/coder.md`——**均已落地** |
| **印证既有机制** | producer 独立重跑、不信下层报告，三轮各抓真实阻塞（假绿+flaky） | 无需新机制——正是 goal-dev-workflow "裁决必须独立 producer、不信报告" 的价值实证 |
| **项目特定/待确认** | 窗口形态 400×600 弹窗→960×640 主窗口；§8 浮窗未来项 | 项目本地，已交付说明标注待用户拍板 |

## 三、晋升回路状态

本轮全部 `[晋升机制]` 项**已落地**（见 `promoted`）：
- F1 `tester.md` 命中校验 +"抗 flaky 多跑"
- F2 `rules/code-general.md` +"排序查询 ORDER BY 确定性兜底"
- F3 `coder.md` 防截断 +"多交付项落地序：接入→留痕→润色"

`[仅观察]`（clippy 漏网）暂不机制化，已由 producer 独立跑 A-QUALITY 覆盖。

## 四、沉淀的可复用原则
- **排序/取数查询必须确定性**：`ORDER BY` 主键之外必带稳定兜底（rowid/主键/插入序）；同值并列在并发负载下退化为 flaky 假绿——这是本版两个独立阻塞的同一根因。
- **抗 flaky 靠多跑**：并发/排序/时间戳/共享资源类，单跑一次的绿不可信，须连跑 ≥3 次；tester 的"命中校验"对全量套件尤其要多跑。
- **接入与留痕是交付物**：多交付项任务里"挂进主程序""写 coding.md"绝不能留到最后当收尾——那正是被截断丢失的位置；测试一过就先落地。
- **独立裁决不可省**：本版三轮裁决，每轮都靠 producer 独立重跑揪出下层（tester）漏判的真实问题——AI 自评自过的风险被机制实证拦下。

## 五、本版累积的 pending-manual（供用户批量回看）
- V4-F2-A10 三页视觉还原 / V4-F3-A13 动效材质手感（审美）
- V4-F1-A02-H01 翻译真实网络 / V4-F1-A04-H01 真机 keychain 开库+arboard 捕获+轮询+命令往返+热键持久化 / V4-F3-A11-H01 真机单图标目视（运行期）
- 均需真机 `make dev` 运行确认；不阻塞 V4 done。
