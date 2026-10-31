---
status: done
category: research
subject: 翻译交互对齐方向调研与方案（待确认）
summary: 确认翻译应分层对齐：交互学 Bob（闭源，仅参考交互）、源清单学 pot（GPL-3.0，已完成）、词典视图学 Easydict（GPL-3.0，fast-follow）。核实当前翻译浮窗读的是剪贴板最新一条，不是选中文字，差距在取词通道。产出 docs/design/translate-interaction-alignment.md（Markdown，未生成 HTML），4 项决策待用户确认，未改动任何代码。
evidence:
  commits: []
  commands:
    - "Grep 翻译相关关键词（全仓）"
    - "grep -n 'pick_latest_translate_text_impl' src-tauri/src/popover.rs"
    - "grep 'AXSelectedText|划词|text_selected' src-tauri/src src（无取词实现）"
  urls:
    - "https://github.com/ripperhe/Bob"
    - "https://github.com/tisfeng/Easydict"
    - "https://pot-app.com/"
---

# AI 活动记录 · 翻译交互对齐方案

## 摘要

- 用户问翻译应对齐哪个软件。结论：分层对齐。交互学 Bob，源清单学 pot，词典视图学 Easydict，代码全部原创。
- 核实 Bob 的 README 明确写明「不是开源软件」，因此只能参考交互，不能参考代码。上一轮回答未核实这一点，本次补上。
- 核实现状：`trans-popover` 读剪贴板最新一条（`popover.rs` → `pick_latest_translate_text_impl`），全仓没有 `AXSelectedText` 取词，差距在取词通道。
- 产出方案文档 `docs/design/translate-interaction-alignment.md`（P0 取词 → P1 光标定位 → P2 选中图标（可选）→ P3 固定面板 → P4 词典），列出 D1–D4 四项待确认决策。
- 未修改代码，未生成 HTML（按用户要求）。

## 未决

- D1 Windows 是否本期做取词；D2 选中图标是否要做；D3 是否允许模拟 Cmd+C（推荐否）；D4 浮窗来源标签是否接受。
