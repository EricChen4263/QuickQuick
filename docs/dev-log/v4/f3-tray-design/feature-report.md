---
id: V4-F3-report
type: feature_report
level: 大功能
parent: V4
children: [V4-F3-S10, V4-F3-S11]
created: 2026-06-01T01:25:00Z
status: 已闭合
author: orchestrator
---

# 大功能报告 · V4-F3 托盘去重 + 设计语言落地

## 目标
修复用户报告的「菜单栏两个 icon」缺陷（问题①），并落地设计文档 §9.1 视觉设计语言 token，供 F2 三页 UI 统一取用。

## 小功能闭合清单（三联齐 + tester 动态证伪过 + reviewer 放行）

| 小功能 | 内容 | objective 验收 | 结果 |
|---|---|---|---|
| S10 tray-single | 删 tauri.conf.json 的 app.trayIcon（消除自动建的第二个托盘），tray.rs 带菜单托盘为单一来源；boot_smoke 配置守卫拦回归 | V4-F3-A11 | ✅ tray_single_source 1；整体编译 exit0；变异加回 trayIcon 如期变红。R1 修注释矛盾（reviewer I-1）后闭合 |
| S11 design-tokens | src/theme：BRAND_FJORD_TEAL #3A7CA5 / RADIUS_MD 10px / FONT_STACK / lightTheme·darkTheme / themeToCssVars + theme.css（:root + @media dark + .qq-main 实色 + .qq-popover 毛玻璃）| V4-F3-A12 | ✅ design-tokens 12；3 变异如期变红；theme.css grep 实证防假绿 |

## 关键决策
- **托盘单一来源**：保留代码建的带菜单托盘（tray.rs，含显示/退出菜单 + 左键点击），删配置自动建的；用**配置守卫**（conf 无 /app/trayIcon，加回即测试变红）把"双图标回归"变成可机检 objective 门禁——比单纯运行目视更可靠。
- **明暗双主题纯 CSS**：`@media (prefers-color-scheme: dark)` 覆盖 `:root` 变量，随系统切换无需 JS runtime。主窗实色（.qq-main）、弹窗毛玻璃（.qq-popover + -webkit-backdrop-filter）。
- **TS token + CSS 变量双轨**：`design-tokens.ts` 供 JS/内联取值，`theme.css` 的 `--qq-*` 变量供页面 CSS 取值，两者变量名经 reviewer 逐一核对一致，供 F2 取用不错位。

## 对 F2 的交付（CSS 变量契约）
`--qq-bg`（主窗实色背景）、`--qq-surface`、`--qq-text`、`--qq-text-muted`、`--qq-border`、`--qq-accent`（品牌色随主题调明度）、`--qq-radius-md`（10px）、`--qq-font`（系统字体栈）。页面根用 `.qq-main`，浮层/弹窗用 `.qq-popover`。

## 归 manual（并入 pending-manual.yaml）
- V4-F3-A11-H01：真机菜单栏仅一个图标 + 托盘菜单/左键交互正常。
- V4-F3-A13：主窗口/弹窗动效与材质手感（毛玻璃+实色+<150ms 淡入无弹跳）。
